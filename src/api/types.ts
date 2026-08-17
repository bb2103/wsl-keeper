export type OverallKind =
  | "ok"
  | "starting"
  | "paused"
  | "circuit"
  | "stopped"
  | "error";

export interface PauseState {
  until: string;
}

export interface DiskRule {
  id: string;
  diskNumber: number;
  friendlyName: string;
  partition: number;
  fsType: string;
  mountName: string;
  enabled: boolean;
}

export interface AppConfig {
  schemaVersion: number;
  autostart: boolean;
  startMinimized: boolean;
  distro: string;
  guardianEnabled: boolean;
  initCommand: string | null;
  pauseState: PauseState | null;
  diskRules: DiskRule[];
  logLevel: string;
  locale: string;
  theme: string;
}

export interface DistroInfo {
  name: string;
  state: string;
  version: number;
  isDefault: boolean;
}

export interface PartitionInfo {
  number: number;
  size: number;
  partitionType: string;
  driveLetter: string | null;
}

export interface DiskInfo {
  number: number;
  friendlyName: string;
  size: number;
  partitionStyle: string;
  healthStatus: string;
  operationalStatus: string;
  serial: string | null;
  partitions: PartitionInfo[];
}

export interface DiskStatus {
  ruleId: string;
  name: string;
  diskNumber: number;
  partition: number;
  fsType: string;
  mountName: string;
  enabled: boolean;
  mounted: boolean;
  device: string | null;
  lastError: string | null;
  failures: number;
  circuitOpen: boolean;
  nextRetry: string | null;
}

export interface KeeperStatus {
  overall: OverallKind;
  distro: string;
  distroRunning: boolean;
  distroVersion: number | null;
  wslAvailable: boolean;
  mountSupported: boolean;
  paused: boolean;
  pauseUntil: string | null;
  runningSince: string | null;
  lastCheck: string | null;
  lastError: string | null;
  wslFailures: number;
  wslCircuitOpen: boolean;
  mountTaskExists: boolean;
  disks: DiskStatus[];
}

export interface AppPaths {
  config: string;
  logs: string;
}

export function defaultConfig(): AppConfig {
  return {
    schemaVersion: 1,
    autostart: false,
    startMinimized: false,
    distro: "",
    guardianEnabled: false,
    initCommand: null,
    pauseState: null,
    diskRules: [],
    logLevel: "INFO",
    locale: "system",
    theme: "system",
  };
}

export const PAUSE_OPTIONS: {
  key: "pause.15m" | "pause.1h" | "pause.4h" | "pause.24h" | "pause.manual";
  minutes: number | null;
}[] = [
  { key: "pause.15m", minutes: 15 },
  { key: "pause.1h", minutes: 60 },
  { key: "pause.4h", minutes: 240 },
  { key: "pause.24h", minutes: 1440 },
  { key: "pause.manual", minutes: null },
];
