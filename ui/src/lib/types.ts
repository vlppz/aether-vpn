// Contract between the Rust backend (src-tauri) and the UI.
// Every type here mirrors a serde struct (camelCase) on the Rust side —
// change both together.

export type Platform = "macos" | "windows" | "linux";
export type TunnelMode = "proxy" | "tun";
export type RoutingPreset = "global" | "smart" | "direct";
export type RuleKind = "domain" | "ip" | "process" | "port";
export type RuleTarget = "proxy" | "direct" | "block";
export type ConnectionState = "disconnected" | "connecting" | "connected" | "disconnecting" | "error";
export type LogLevel = "debug" | "info" | "warning" | "error" | "access" | "app";
export type Theme = "dark" | "light" | "system";
export type Language = "en" | "ru";

export interface ServerView {
  id: string;
  name: string;
  subscriptionId?: string;
  /** "VLESS" | "VMess" | "Trojan" | "Shadowsocks" | "SOCKS" | "Hysteria2" | "JSON" */
  protocol: string;
  /** "tcp" | "ws" | "grpc" | "httpupgrade" | "xhttp" | "kcp" | "quic" | "custom" */
  transport: string;
  /** "none" | "tls" | "reality" | "aead" */
  security: string;
  address: string | null;
  port: number | null;
  /** ISO 3166-1 alpha-2, uppercase */
  country: string | null;
  /** last measured TCP latency in ms; null = never tested; -1 = failed */
  latency: number | null;
}

export interface UserInfo {
  upload: number;
  download: number;
  /** bytes, 0 = unlimited */
  total: number;
  /** unix seconds */
  expire?: number;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  userAgent?: string;
  autoUpdate: boolean;
  updateIntervalHours: number;
  /** unix seconds */
  lastUpdated?: number;
  lastError?: string;
  info?: UserInfo;
  announce?: string;
  supportUrl?: string;
  webPageUrl?: string;
}

export interface Rule {
  id: string;
  enabled: boolean;
  kind: RuleKind;
  values: string[];
  target: RuleTarget;
  note?: string;
}

export interface RoutingSettings {
  preset: RoutingPreset;
  /** two-letter region whose sites go direct in "smart", e.g. "ru" */
  region: string;
  bypassLan: boolean;
  blockAds: boolean;
  domainStrategy: "AsIs" | "IPIfNonMatch" | "IPOnDemand";
  rules: Rule[];
}

export interface CoreSettings {
  mode: TunnelMode;
  socksPort: number;
  httpPort: number;
  metricsPort: number;
  allowLan: boolean;
  sniffing: boolean;
  logLevel: "debug" | "info" | "warning" | "error" | "none";
  accessLog: boolean;
  tun: { name: string; mtu: number; ipv6: boolean };
  /** tunDirect: in TUN mode resolve every domain with `direct`, outside the tunnel */
  dns: { remote: string; direct: string; tunDirect: boolean };
  routing: RoutingSettings;
}

export interface Settings {
  core: CoreSettings;
  theme: Theme;
  /** hex color, e.g. "#7c5cff" */
  accent: string;
  language: Language;
  launchAtLogin: boolean;
  startMinimized: boolean;
  autoConnect: boolean;
  /** set the OS proxy automatically in proxy mode */
  setSystemProxy: boolean;
  closeToTray: boolean;
  /** custom User-Agent for subscription requests; null = default */
  userAgent: string | null;
}

export interface ConnectionStatus {
  state: ConnectionState;
  mode: TunnelMode;
  serverId?: string;
  /** unix ms when the connection became active */
  since?: number;
  error?: string;
  exitIp?: string;
  /** ISO country code of the exit IP */
  exitCountry?: string;
  exitCity?: string;
}

export interface Snapshot {
  servers: ServerView[];
  subscriptions: Subscription[];
  selectedId: string | null;
  status: ConnectionStatus;
  settings: Settings;
  platform: Platform;
  appVersion: string;
  coreVersion: string | null;
}

export interface ImportResult {
  added: number;
  duplicates: number;
  subscriptionsAdded: number;
  errors: string[];
}

/** payload of the "traffic" event, emitted every second while connected */
export interface TrafficTick {
  /** bytes per second */
  up: number;
  down: number;
  /** bytes since connect */
  totalUp: number;
  totalDown: number;
}

/** payload of the "log" event */
export interface LogLine {
  /** unix ms */
  ts: number;
  level: LogLevel;
  message: string;
}

/** payload of the "latency" event */
export interface LatencyUpdate {
  id: string;
  latency: number;
}

/** payload of the "notice" event (backend-originated toasts) */
export interface Notice {
  kind: "success" | "error" | "info";
  message: string;
}

/** names of events emitted by the backend */
export const EVENTS = {
  snapshot: "snapshot", // payload: Snapshot — any change to servers/subs/selection/settings
  status: "status", // payload: ConnectionStatus
  traffic: "traffic", // payload: TrafficTick
  log: "log", // payload: LogLine
  latency: "latency", // payload: LatencyUpdate
  notice: "notice", // payload: Notice
} as const;
