import { create } from "zustand";
import { defaultSettings } from "../lib/defaults";
import type { ConnectionStatus, LogLine, Platform, ServerView, Settings, Subscription, TrafficTick } from "../lib/types";
import type { TrafficSample } from "./reducers";

export type Page = "home" | "servers" | "subscriptions" | "routing" | "logs" | "settings";
export const PAGES: Page[] = ["home", "servers", "subscriptions", "routing", "logs", "settings"];

export interface AppState {
  ready: boolean;
  bootError: string | null;

  // mirrored from the backend snapshot
  servers: ServerView[];
  subscriptions: Subscription[];
  selectedId: string | null;
  settings: Settings;
  platform: Platform;
  appVersion: string;
  coreVersion: string | null;

  status: ConnectionStatus;
  traffic: TrafficSample[];
  lastTick: TrafficTick | null;
  logs: LogLine[];
  /** server ids with a ping in flight */
  pinging: Record<string, true>;
  /** subscription ids being refreshed */
  refreshing: Record<string, true>;

  // UI-only
  page: Page;
  paletteOpen: boolean;
  importOpen: boolean;
  pickerOpen: boolean;
}

function guessPlatform(): Platform {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Mac/i.test(ua)) return "macos";
  if (/Win/i.test(ua)) return "windows";
  return "linux";
}

export const useApp = create<AppState>()(() => ({
  ready: false,
  bootError: null,
  servers: [],
  subscriptions: [],
  selectedId: null,
  settings: defaultSettings(),
  platform: guessPlatform(),
  appVersion: "",
  coreVersion: null,
  status: { state: "disconnected", mode: "proxy" },
  traffic: [],
  lastTick: null,
  logs: [],
  pinging: {},
  refreshing: {},
  page: "home",
  paletteOpen: false,
  importOpen: false,
  pickerOpen: false,
}));

export const setUi = (patch: Partial<Pick<AppState, "page" | "paletteOpen" | "importOpen" | "pickerOpen">>) =>
  useApp.setState(patch);

export const selectSelectedServer = (s: AppState): ServerView | undefined =>
  s.servers.find((x) => x.id === s.selectedId);

export function isDark(theme: Settings["theme"]): boolean {
  if (theme === "system") return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches;
  return theme === "dark";
}
