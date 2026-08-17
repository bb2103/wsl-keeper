import { useMemo } from "react";
import {
  ArrowsClockwise,
  CheckCircle,
  CircleNotch,
  GearSix,
  HardDrives,
  Pause,
  Play,
  Warning,
} from "@phosphor-icons/react";
import { keeper } from "../api";
import { PAUSE_OPTIONS, type AppConfig, type KeeperStatus, type OverallKind } from "../api/types";
import { useI18n, type TFunction } from "../i18n";
import { formatClock, formatDuration } from "../lib/format";
import type { ResolvedTheme } from "../lib/theme";
import AnimatedContent from "../components/AnimatedContent";
import BlurText from "../components/BlurText";
import GlareHover from "../components/GlareHover";
import Magnet from "../components/Magnet";
import ShinyText from "../components/ShinyText";
import SpotlightCard from "../components/SpotlightCard";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "../components/ui/dropdown-menu";

interface Props {
  config: AppConfig;
  status: KeeperStatus | null;
  busy: boolean;
  resolvedTheme: ResolvedTheme;
  onOpenSettings: () => void;
  run: (action: () => Promise<unknown>) => Promise<void>;
}

const OVERALL_KEYS = {
  ok: "overall.ok",
  starting: "overall.starting",
  paused: "overall.paused",
  circuit: "overall.circuit",
  stopped: "overall.stopped",
  error: "overall.error",
} as const;

function overallCopy(kind: OverallKind, t: TFunction): string {
  return t(OVERALL_KEYS[kind]);
}

function overallBadge(kind: OverallKind) {
  if (kind === "ok") return "ok" as const;
  if (kind === "paused") return "pause" as const;
  if (kind === "starting") return "warn" as const;
  if (kind === "stopped") return "idle" as const;
  return "danger" as const;
}

export default function Dashboard({
  config,
  status,
  busy,
  resolvedTheme,
  onOpenSettings,
  run,
}: Props) {
  const { t, locale } = useI18n();
  const overall = status?.overall ?? "paused";

  const pauseLabel = useMemo(() => {
    if (!status?.paused) return null;
    if (!status.pauseUntil) return t("pause.untilResume");
    const until = new Date(status.pauseUntil);
    const far = until.getTime() - Date.now() > 1000 * 60 * 60 * 24 * 30;
    return far
      ? t("pause.untilResume")
      : t("pause.until", { time: formatClock(status.pauseUntil, locale) });
  }, [status, t, locale]);

  const distroName = status?.distro || config.distro || t("hero.noDistro");
  const shineColor = resolvedTheme === "light" ? "#1c1f26" : "#ffffff";
  const shineBase = resolvedTheme === "light" ? "#5c6370" : "#9aa1ad";

  return (
    <main className="page" id="snap-main-container">
      <AnimatedContent distance={24} duration={0.5} threshold={0.01}>
        <GlareHover
          width="100%"
          height="auto"
          background="var(--card)"
          borderColor="var(--border)"
          borderRadius="var(--radius)"
          glareColor="#ffffff"
          glareOpacity={resolvedTheme === "light" ? 0.42 : 0.22}
          className={`hero ${overall}`}
        >
          <div className="hero-copy">
            <div className="hero-kicker">
              <Badge variant={overallBadge(overall)}>
                {overall === "ok" && <CheckCircle size={14} weight="fill" />}
                {overall === "starting" && <CircleNotch size={14} className="spin" />}
                {overall === "paused" && <Pause size={14} weight="fill" />}
                {(overall === "circuit" || overall === "error") && (
                  <Warning size={14} weight="fill" />
                )}
                {overall === "stopped" && <HardDrives size={14} />}
                <ShinyText
                  text={overallCopy(overall, t)}
                  color={shineBase}
                  shineColor={shineColor}
                  speed={3}
                />
              </Badge>
            </div>
            <BlurText
              key={distroName}
              text={distroName}
              className="hero-title"
              animateBy="letters"
              delay={40}
              stepDuration={0.22}
            />
            <p className="hero-meta">
              {status?.paused
                ? pauseLabel
                : status?.distroRunning
                  ? t("hero.upFor", { duration: formatDuration(status.runningSince, t) })
                  : status?.wslAvailable
                    ? t("hero.waitingWsl")
                    : t("hero.wslMissing")}
            </p>
            {status?.lastError && !status.paused && (
              <p className="hero-error">{status.lastError}</p>
            )}
          </div>
          <div className="hero-actions">
            {status?.paused ? (
              <Magnet padding={28} magnetStrength={5}>
                <Button
                  disabled={busy}
                  onClick={() => void run(() => keeper.guardian.resume())}
                >
                  <Play size={16} weight="fill" />
                  {t("pause.resume")}
                </Button>
              </Magnet>
            ) : (
              <DropdownMenu>
                <Magnet padding={28} magnetStrength={5}>
                  <DropdownMenuTrigger asChild>
                    <Button variant="secondary" disabled={busy}>
                      <Pause size={16} />
                      {t("pause.action")}
                    </Button>
                  </DropdownMenuTrigger>
                </Magnet>
                <DropdownMenuContent align="end">
                  {PAUSE_OPTIONS.map((option) => (
                    <DropdownMenuItem
                      key={option.key}
                      onSelect={() => {
                        void run(() => keeper.guardian.pause(option.minutes));
                      }}
                    >
                      {t(option.key)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            )}
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => void run(() => keeper.guardian.checkNow())}
            >
              <ArrowsClockwise size={16} />
              {t("action.checkNow")}
            </Button>
            {status?.wslCircuitOpen && (
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() => void run(() => keeper.guardian.resetWsl())}
              >
                {t("action.resetWslRetry")}
              </Button>
            )}
            <Button variant="ghost" onClick={onOpenSettings}>
              <GearSix size={16} />
              {t("action.settings")}
            </Button>
          </div>
        </GlareHover>

        <section className="section">
          <div className="section-head">
            <h2>{t("disks.title")}</h2>
            <span className="disk-meta">
              {t("disks.mountedCount", {
                mounted: status?.disks.filter((d) => d.mounted).length ?? 0,
                total: status?.disks.length ?? 0,
              })}
            </span>
          </div>
          {!status?.disks.length ? (
            <Card className="empty">
              <HardDrives size={28} />
              <p>{t("disks.empty")}</p>
              <Button variant="secondary" onClick={onOpenSettings}>
                {t("disks.addInSettings")}
              </Button>
            </Card>
          ) : (
            <div className="disk-grid">
              {status.disks.map((disk) => {
                const tone = !disk.enabled
                  ? "idle"
                  : disk.circuitOpen
                    ? "circuit"
                    : disk.mounted
                      ? "ok"
                      : "starting";
                return (
                  <SpotlightCard
                    key={disk.ruleId}
                    className={`disk-card ${tone}`}
                    spotlightColor={
                      resolvedTheme === "light"
                        ? "rgba(63, 157, 110, 0.18)"
                        : "rgba(63, 157, 110, 0.22)"
                    }
                  >
                    <header>
                      <span className={`pulse-dot ${tone}`} />
                      <strong>{disk.name || disk.mountName}</strong>
                      <Badge
                        variant={
                          tone === "ok"
                            ? "ok"
                            : tone === "starting"
                              ? "warn"
                              : tone === "circuit"
                                ? "danger"
                                : "idle"
                        }
                        className="ml-auto"
                      >
                        {!disk.enabled
                          ? t("disks.disabled")
                          : disk.circuitOpen
                            ? t("disks.mountFailed")
                            : disk.mounted
                              ? t("disks.mounted")
                              : t("disks.notMounted")}
                      </Badge>
                    </header>
                    <p className="disk-path">/mnt/wsl/{disk.mountName}</p>
                    <p className="disk-meta">
                      PHYSICALDRIVE{disk.diskNumber} · {t("disks.partition", { n: disk.partition })} ·{" "}
                      {disk.fsType}
                    </p>
                    {disk.mounted && disk.device && (
                      <p className="disk-state">{t("disks.mountedAt", { device: disk.device })}</p>
                    )}
                    {disk.lastError && <p className="hero-error">{disk.lastError}</p>}
                    {disk.circuitOpen && (
                      <Button
                        size="sm"
                        variant="secondary"
                        className="mt-3"
                        disabled={busy}
                        onClick={() =>
                          void run(() => keeper.guardian.resetDisk(disk.ruleId))
                        }
                      >
                        {t("disks.retryNow")}
                      </Button>
                    )}
                  </SpotlightCard>
                );
              })}
            </div>
          )}
        </section>

        <footer className="footnote">
          {t("footer.lastCheck", {
            time: formatClock(status?.lastCheck ?? null, locale) || t("footer.pending"),
          })}
          {status && !status.mountSupported && status.wslAvailable
            ? ` · ${t("footer.needsWsl2")}`
            : ""}
        </footer>
      </AnimatedContent>
    </main>
  );
}
