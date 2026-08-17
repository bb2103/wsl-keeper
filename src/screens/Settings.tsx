import { useEffect, useRef, useState } from "react";
import { Export, FolderOpen, HardDrives, Plus, Trash } from "@phosphor-icons/react";
import { keeper } from "../api";
import type {
  AppConfig,
  AppPaths,
  DiskInfo,
  DistroInfo,
  KeeperStatus,
} from "../api/types";
import { useI18n } from "../i18n";
import { errorMessage, formatBytes } from "../lib/format";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Label } from "../components/ui/label";
import { NativeSelect } from "../components/ui/select";
import { Switch } from "../components/ui/switch";

interface Props {
  config: AppConfig;
  status: KeeperStatus | null;
  busy: boolean;
  setError: (error: string | null) => void;
  onChange: (config: AppConfig) => void;
  onSaved: () => Promise<void>;
}

export default function Settings({
  config,
  status,
  busy,
  setError,
  onChange,
  onSaved,
}: Props) {
  const { t } = useI18n();
  const [distros, setDistros] = useState<DistroInfo[]>([]);
  const [disks, setDisks] = useState<DiskInfo[]>([]);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [loadingInventory, setLoadingInventory] = useState(true);
  const saveTimer = useRef(0);
  const lastSaved = useRef("");
  const configRef = useRef(config);
  configRef.current = config;

  useEffect(() => {
    lastSaved.current = JSON.stringify(config);
    let cancelled = false;
    void (async () => {
      const [nextDistros, nextDisks, nextPaths] = await Promise.all([
        keeper.wsl.list().catch(() => [] as DistroInfo[]),
        keeper.disks.list().catch(() => [] as DiskInfo[]),
        keeper.app.paths().catch(() => null),
      ]);
      if (cancelled) return;
      setDistros(nextDistros);
      setDisks(nextDisks);
      setPaths(nextPaths);
      setLoadingInventory(false);
    })();
    return () => {
      cancelled = true;
      window.clearTimeout(saveTimer.current);
    };
    // Prime lastSaved from the first config snapshot only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!distros.length) return;
    const current = configRef.current;
    if (distros.some((d) => d.name === current.distro)) return;
    const next = distros.find((d) => d.isDefault)?.name ?? distros[0].name;
    persist({ ...current, distro: next });
  }, [distros]);

  function persist(next: AppConfig) {
    onChange(next);
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      void (async () => {
        const encoded = JSON.stringify(next);
        if (encoded === lastSaved.current) return;
        try {
          await keeper.config.save(next);
          lastSaved.current = encoded;
          setError(null);
        } catch (e) {
          setError(errorMessage(e));
        }
      })();
    }, 280);
  }

  function patch(partial: Partial<AppConfig>) {
    persist({ ...configRef.current, ...partial });
  }

  function addDisk(disk: DiskInfo) {
    const current = configRef.current;
    const partition = disk.partitions[0]?.number ?? 1;
    const exists = current.diskRules.some(
      (rule) => rule.diskNumber === disk.number && rule.partition === partition,
    );
    if (exists) return;
    patch({
      diskRules: [
        ...current.diskRules,
        {
          id: crypto.randomUUID(),
          diskNumber: disk.number,
          friendlyName: disk.friendlyName || t("settings.diskFallback", { n: disk.number }),
          partition,
          fsType: "ext4",
          mountName: `disk${disk.number}-${partition}`,
          enabled: true,
        },
      ],
    });
  }

  function updateRule(id: string, partial: Partial<AppConfig["diskRules"][number]>) {
    const current = configRef.current;
    patch({
      diskRules: current.diskRules.map((rule) =>
        rule.id === id ? { ...rule, ...partial } : rule,
      ),
    });
  }

  return (
    <main className="page settings">
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.general")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="field">
            <Label htmlFor="locale">{t("settings.language")}</Label>
            <NativeSelect
              id="locale"
              value={config.locale || "system"}
              onChange={(e) => patch({ locale: e.target.value })}
            >
              <option value="system">{t("settings.localeSystem")}</option>
              <option value="en">English</option>
              <option value="zh">简体中文</option>
            </NativeSelect>
            <p className="hint">{t("settings.languageHint")}</p>
          </div>
          <div className="field">
            <Label htmlFor="theme">{t("theme.label")}</Label>
            <NativeSelect
              id="theme"
              value={config.theme || "system"}
              onChange={(e) => patch({ theme: e.target.value })}
            >
              <option value="system">{t("theme.system")}</option>
              <option value="light">{t("theme.light")}</option>
              <option value="dark">{t("theme.dark")}</option>
            </NativeSelect>
            <p className="hint">{t("theme.hint")}</p>
          </div>
          <div className="row">
            <div>
              <strong>{t("settings.autostart")}</strong>
              <p>{t("settings.autostartHint")}</p>
            </div>
            <Switch
              checked={config.autostart}
              onCheckedChange={(checked) => patch({ autostart: checked })}
            />
          </div>
          <div className="row">
            <div>
              <strong>{t("settings.startInTray")}</strong>
              <p>{t("settings.startInTrayHint")}</p>
            </div>
            <Switch
              checked={config.startMinimized}
              onCheckedChange={(checked) => patch({ startMinimized: checked })}
            />
          </div>
          <div className="field">
            <Label htmlFor="log-level">{t("settings.logLevel")}</Label>
            <NativeSelect
              id="log-level"
              value={config.logLevel}
              onChange={(e) => patch({ logLevel: e.target.value })}
            >
              <option value="INFO">INFO</option>
              <option value="WARN">WARN</option>
              <option value="ERROR">ERROR</option>
              <option value="DEBUG">DEBUG</option>
            </NativeSelect>
          </div>
          <div className="btn-row">
            <Button
              variant="secondary"
              onClick={() => void keeper.config.exportToFile().catch((e) => setError(errorMessage(e)))}
            >
              <Export size={16} />
              {t("settings.exportConfig")}
            </Button>
            <Button
              variant="secondary"
              onClick={() =>
                void keeper.config
                  .importFromFile()
                  .then(async (ok) => {
                    if (!ok) return;
                    lastSaved.current = "";
                    await onSaved();
                  })
                  .catch((e) => setError(errorMessage(e)))
              }
            >
              {t("settings.importConfig")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.wsl")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="field">
            <Label htmlFor="distro">{t("settings.distroKeepRunning")}</Label>
            <NativeSelect
              id="distro"
              value={config.distro}
              onChange={(e) => patch({ distro: e.target.value })}
              disabled={loadingInventory && distros.length === 0}
            >
              {!config.distro && <option value="">{t("settings.detectingDistros")}</option>}
              {config.distro && !distros.some((d) => d.name === config.distro) && (
                <option value={config.distro}>{config.distro}</option>
              )}
              {distros.map((d) => (
                <option key={d.name} value={d.name}>
                  {d.name} ({d.state}, WSL {d.version}
                  {d.isDefault ? `, ${t("settings.distroDefault")}` : ""})
                </option>
              ))}
            </NativeSelect>
          </div>
          <div className="field">
            <Label htmlFor="init-command">{t("settings.initCommand")}</Label>
            <Input
              id="init-command"
              value={config.initCommand ?? ""}
              placeholder="sudo service docker start"
              onChange={(e) =>
                patch({ initCommand: e.target.value.trim() ? e.target.value : null })
              }
            />
          </div>
          {status && !status.mountSupported && status.wslAvailable && (
            <p className="hint warn">{t("settings.notWsl2")}</p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.disks")}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="hint">{t("settings.disksHint")}</p>
          <div className="disk-picker">
            {loadingInventory && disks.length === 0 && (
              <p className="hint">{t("settings.readingDisks")}</p>
            )}
            {!loadingInventory && disks.length === 0 && (
              <p className="hint">{t("settings.noDisks")}</p>
            )}
            {disks.map((disk) => (
              <div key={disk.number} className="picker-row">
                <div>
                  <strong>
                    {disk.friendlyName || t("settings.diskFallback", { n: disk.number })}
                  </strong>
                  <p>
                    PHYSICALDRIVE{disk.number} · {formatBytes(disk.size, t)} ·{" "}
                    {disk.partitionStyle}
                  </p>
                </div>
                <Button size="sm" variant="secondary" onClick={() => addDisk(disk)}>
                  <Plus size={14} />
                  {t("settings.guard")}
                </Button>
              </div>
            ))}
          </div>

          {config.diskRules.length > 0 && (
            <div className="rule-list">
              {config.diskRules.map((rule) => {
                const disk = disks.find((d) => d.number === rule.diskNumber);
                return (
                  <div key={rule.id} className="rule">
                    <header>
                      <HardDrives size={16} />
                      <strong>{rule.friendlyName}</strong>
                      <label className="mini">
                        <Switch
                          checked={rule.enabled}
                          onCheckedChange={(checked) =>
                            updateRule(rule.id, { enabled: checked })
                          }
                        />
                        {t("settings.autoMount")}
                      </label>
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={t("settings.removeRule")}
                        onClick={() =>
                          patch({
                            diskRules: config.diskRules.filter((r) => r.id !== rule.id),
                          })
                        }
                      >
                        <Trash size={16} />
                      </Button>
                    </header>
                    <div className="rule-grid">
                      <div className="field">
                        <Label>{t("settings.partition")}</Label>
                        <NativeSelect
                          value={rule.partition}
                          onChange={(e) =>
                            updateRule(rule.id, { partition: Number(e.target.value) })
                          }
                        >
                          {(disk?.partitions.length
                            ? disk.partitions
                            : [
                                {
                                  number: rule.partition,
                                  size: 0,
                                  partitionType: "Unknown",
                                  driveLetter: null,
                                },
                              ]
                          ).map((p) => (
                            <option key={p.number} value={p.number}>
                              {p.number}
                              {p.size ? ` · ${formatBytes(p.size, t)}` : ""}
                              {p.partitionType ? ` · ${p.partitionType}` : ""}
                            </option>
                          ))}
                        </NativeSelect>
                      </div>
                      <div className="field">
                        <Label>{t("settings.filesystem")}</Label>
                        <NativeSelect
                          value={rule.fsType}
                          onChange={(e) => updateRule(rule.id, { fsType: e.target.value })}
                        >
                          <option value="ext4">ext4</option>
                          <option value="xfs">xfs</option>
                          <option value="btrfs">btrfs</option>
                        </NativeSelect>
                      </div>
                      <div className="field">
                        <Label>{t("settings.wslName")}</Label>
                        <Input
                          value={rule.mountName}
                          onChange={(e) =>
                            updateRule(rule.id, { mountName: e.target.value })
                          }
                        />
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.maintenance")}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="path">{paths?.config}</p>
          <div className="btn-row">
            <Button variant="secondary" onClick={() => void keeper.app.openLogs()}>
              <FolderOpen size={16} />
              {t("settings.openLogs")}
            </Button>
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => void keeper.guardian.resetWsl().then(onSaved)}
            >
              {t("settings.resetWslCircuit")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </main>
  );
}
