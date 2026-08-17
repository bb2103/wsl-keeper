import { useCallback, useEffect, useState } from "react";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { keeper } from "../api";
import { defaultConfig, type AppConfig, type KeeperStatus } from "../api/types";
import { errorMessage } from "./format";

export type Page = "dashboard" | "settings";

export function useKeeper() {
  const [page, setPage] = useState<Page>("dashboard");
  const [config, setConfig] = useState<AppConfig>(defaultConfig());
  const [status, setStatus] = useState<KeeperStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async (opts?: { config?: boolean }) => {
    try {
      if (opts?.config === false) {
        setStatus(await keeper.status.get());
        return;
      }
      const [nextConfig, nextStatus] = await Promise.all([
        keeper.config.get(),
        keeper.status.get(),
      ]);
      setConfig(nextConfig);
      setStatus(nextStatus);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    void (async () => {
      try {
        const granted = await isPermissionGranted();
        if (!granted) await requestPermission();
      } catch {
        // Notifications are optional.
      }
    })();

    const timer = window.setInterval(() => void refresh({ config: false }), 4000);
    let unlistenStatus: (() => void) | undefined;
    let unlistenNav: (() => void) | undefined;

    void keeper.status.subscribe((next) => {
      setStatus(next);
      setConfig((current) =>
        !current.distro && next.distro ? { ...current, distro: next.distro } : current,
      );
    }).then((fn) => {
      unlistenStatus = fn;
    });
    void keeper.app.onNavigate((next) => {
      setPage(next === "settings" ? "settings" : "dashboard");
    }).then((fn) => {
      unlistenNav = fn;
    });

    return () => {
      window.clearInterval(timer);
      unlistenStatus?.();
      unlistenNav?.();
    };
  }, [refresh]);

  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      setError(null);
      try {
        await action();
        await refresh();
      } catch (e) {
        setError(errorMessage(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  return {
    page,
    setPage,
    config,
    setConfig,
    status,
    error,
    setError,
    busy,
    setBusy,
    refresh,
    run,
  };
}
