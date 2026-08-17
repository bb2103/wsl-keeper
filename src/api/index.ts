import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppConfig,
  AppPaths,
  DiskInfo,
  DistroInfo,
  KeeperStatus,
} from "./types";

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

export const keeper = {
  config: {
    get: () => call<AppConfig>("config_get"),
    save: (config: AppConfig) => call<void>("config_save", { config }),
    importJson: (content: string) => call<void>("config_import", { content }),
    exportJson: () => call<string>("config_export"),
    async exportToFile(): Promise<boolean> {
      const content = await keeper.config.exportJson();
      const path = await save({
        defaultPath: "wsl-keeper-config.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return false;
      await call<void>("app_write_file", { path, content });
      return true;
    },
    async importFromFile(): Promise<boolean> {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected || Array.isArray(selected)) return false;
      const content = await call<string>("app_read_file", { path: selected });
      await keeper.config.importJson(content);
      return true;
    },
  },

  status: {
    get: () => call<KeeperStatus>("status_get"),
    subscribe(onStatus: (status: KeeperStatus) => void): Promise<UnlistenFn> {
      return listen<KeeperStatus>("status", (event) => onStatus(event.payload));
    },
  },

  guardian: {
    pause: (minutes: number | null) =>
      call<void>("guardian_pause", { minutes }),
    resume: () => call<void>("guardian_resume"),
    checkNow: () => call<void>("guardian_check"),
    resetWsl: () => call<void>("guardian_reset_wsl"),
    resetDisk: (ruleId: string) =>
      call<void>("guardian_reset_disk", { ruleId }),
  },

  wsl: {
    list: () => call<DistroInfo[]>("wsl_list"),
  },

  disks: {
    list: () => call<DiskInfo[]>("disk_list"),
  },

  app: {
    paths: () => call<AppPaths>("app_paths"),
    openLogs: () => call<void>("app_open_logs"),
    onNavigate(onPage: (page: string) => void): Promise<UnlistenFn> {
      return listen<string>("navigate", (event) => onPage(event.payload));
    },
  },
};

export type { AppConfig, AppPaths, DiskInfo, DistroInfo, KeeperStatus };
