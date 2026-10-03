export type Mode = "bidirectional" | "upload" | "download";
export interface SyncPair {
  id: string;
  name: string;
  localPath: string;
  remotePath: string;
  deviceUid?: string | null;
  mode: Mode;
  intervalMinutes: number;
  enabled: boolean;
  lastRun: number | null;
  propagateDeletions: boolean;
}
export interface Activity {
  id: string;
  timestamp: number;
  kind: string;
  message: string;
  pairId: string | null;
}
export interface Config {
  locale: import("~/composables/useI18n").Locale;
  pairs: SyncPair[];
  autoUpdate: boolean;
  paused: boolean;
  closeToTray: boolean;
  computerRegistrations: Record<string, ComputerRegistration>;
  lastUpdateCheck: number | null;
  events: Activity[];
  accountId: string | null;
  accountEmail: string | null;
}
export interface Runtime {
  connected: boolean;
  busy: boolean;
  operation: string;
  currentPair: string | null;
  currentFile: string | null;
  cliVersion: string;
  error: string | null;
}
export interface DriveState {
  config: Config;
  runtime: Runtime;
  dataPath: string;
  autostart: boolean;
  trayAvailable: boolean;
  computer: ComputerRegistration | null;
  queue: OperationQueue;
}
export interface ComputerRegistration {
  name: string;
  deviceUid: string | null;
}
export interface Operation {
  lane: "transfer" | "interactive" | "background" | "exclusive";
  id: string;
  kind: string;
  title: string;
  detail: string;
  pairId: string | null;
  automatic: boolean;
  status:
    "queued" | "running" | "cancelling" | "completed" | "failed" | "cancelled";
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  error: string | null;
  canCancelRunning: boolean;
}
export interface OperationQueue {
  paused: boolean;
  items: Operation[];
}
export interface RemoteEntry {
  name: string;
  path: string;
  directory: boolean;
  revision: string;
  uid: string;
  kind: string;
  mediaType: string;
  nativeDocument: boolean;
  photoCount: number;
  modified: string | null;
  size: number;
}

export interface LibraryView {
  entries: RemoteEntry[] | null;
  revision: number;
  updatedAt: number | null;
  refreshing: boolean;
  partial: boolean;
  foldersDone: number;
  foldersPending: number;
  error: string | null;
}
