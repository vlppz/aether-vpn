// Side-effectful actions: backend calls, event wiring, optimistic updates and toasts.
import { toast } from "sonner";
import { api, copyText, errorMessage } from "../lib/api";
import { on, type Unlisten } from "../lib/events";
import type { ImportResult, LogLine, RoutingPreset, Settings, Snapshot, Subscription, TunnelMode } from "../lib/types";
import { getT } from "../i18n";
import { useApp, type AppState } from ".";
import { appendLogs, mergeLatency, pushSample } from "./reducers";
import { isWindowActive } from "../lib/windowActive";

const set = useApp.setState;
const get = useApp.getState;

/** Runs a backend call and turns failures into an error toast. */
export async function attempt<T>(fn: () => Promise<T>, title?: string): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    toast.error(title ?? getT()("toast.error"), { description: errorMessage(e) });
    return undefined;
  }
}

// ------------------------------------------------------------------ settings (optimistic + debounced)

let saveTimer: ReturnType<typeof setTimeout> | undefined;
let savesInFlight = 0;

function settingsPending(): boolean {
  return saveTimer !== undefined || savesInFlight > 0;
}

async function flushSettings() {
  saveTimer = undefined;
  savesInFlight++;
  try {
    await api.saveSettings(get().settings);
  } catch (e) {
    toast.error(getT()("toast.settingsFailed"), { description: errorMessage(e) });
  } finally {
    savesInFlight--;
  }
}

/** Mutate a draft copy of the settings; persisted via save_settings after a short debounce. */
export function updateSettings(mutate: (draft: Settings) => void, delay = 400): void {
  const draft = structuredClone(get().settings);
  mutate(draft);
  set({ settings: draft });
  clearTimeout(saveTimer);
  saveTimer = setTimeout(flushSettings, delay);
}

export function setPreset(preset: RoutingPreset) {
  updateSettings((s) => {
    s.core.routing.preset = preset;
  }, 0);
}

export function toggleTheme() {
  const dark = document.documentElement.dataset.theme === "dark";
  updateSettings((s) => {
    s.theme = dark ? "light" : "dark";
  }, 0);
}

// ------------------------------------------------------------------ snapshot & events

function applySnapshot(snap: Snapshot) {
  const patch: Partial<AppState> = {
    servers: snap.servers,
    subscriptions: snap.subscriptions,
    selectedId: snap.selectedId,
    status: snap.status,
    platform: snap.platform,
    appVersion: snap.appVersion,
    coreVersion: snap.coreVersion,
  };
  // don't clobber edits the user is still typing / that are mid-save
  if (!settingsPending()) patch.settings = snap.settings;
  set(patch);
}

let logBuffer: LogLine[] = [];
let logFlush: ReturnType<typeof setTimeout> | undefined;

function queueLog(line: LogLine) {
  logBuffer.push(line);
  // nobody is reading while the window is in the background: batch harder
  logFlush ??= setTimeout(() => {
    const lines = logBuffer;
    logBuffer = [];
    logFlush = undefined;
    set((s) => ({ logs: appendLogs(s.logs, lines) }));
  }, isWindowActive() ? 120 : 1000);
}

let unlisteners: Unlisten[] = [];

async function subscribe() {
  unlisteners.forEach((u) => u());
  unlisteners = await Promise.all([
    on("snapshot", applySnapshot),
    on("status", (status) => {
      const prev = get().status.state;
      const leaving = prev === "connected" && status.state !== "connected";
      const fresh = status.state === "connecting" && prev !== "connected";
      set(leaving || fresh ? { status, traffic: [], lastTick: null } : { status });
    }),
    on("traffic", (tick) => set((s) => ({ traffic: pushSample(s.traffic, tick, Date.now()), lastTick: tick }))),
    on("log", queueLog),
    on("latency", (upd) =>
      set((s) => {
        const pinging = { ...s.pinging };
        delete pinging[upd.id];
        return { servers: mergeLatency(s.servers, upd), pinging };
      }),
    ),
    on("notice", (n) => {
      if (n.kind === "success") toast.success(n.message);
      else if (n.kind === "error") toast.error(n.message);
      else toast(n.message);
    }),
  ]);
}

export async function bootstrap(): Promise<void> {
  try {
    await subscribe();
    const [snap, logs] = await Promise.all([api.getSnapshot(), api.getLogs().catch(() => [] as LogLine[])]);
    applySnapshot(snap);
    set({ settings: snap.settings, logs: appendLogs([], logs), ready: true, bootError: null });
  } catch (e) {
    set({ bootError: errorMessage(e) });
  }
}

// ------------------------------------------------------------------ connection

export async function toggleConnection() {
  const t = getT();
  const { status, selectedId } = get();
  if (!selectedId && status.state !== "connected") {
    set({ pickerOpen: true });
    return;
  }
  await attempt(() => api.toggleConnection(), t("toast.connectFailed"));
}

export async function connect() {
  await attempt(() => api.connect(), getT()("toast.connectFailed"));
}

export async function disconnect() {
  await attempt(() => api.disconnect());
}

export async function selectServer(id: string, andConnect = false) {
  set({ selectedId: id });
  await attempt(() => api.selectServer(id));
  const state = get().status.state;
  if (andConnect && state !== "connected" && state !== "connecting") await connect();
}

export async function setMode(mode: TunnelMode) {
  const prev = get().settings.core.mode;
  if (prev === mode) return;
  set((s) => ({ settings: { ...s.settings, core: { ...s.settings.core, mode } } }));
  const ok = await attempt(() => api.setMode(mode).then(() => true));
  if (!ok) set((s) => ({ settings: { ...s.settings, core: { ...s.settings.core, mode: prev } } }));
}

export async function testRealDelay(): Promise<number | undefined> {
  return attempt(() => api.testRealDelay());
}

// ------------------------------------------------------------------ servers

let pingSafety: ReturnType<typeof setTimeout> | undefined;

export async function pingAll() {
  const pinging: Record<string, true> = {};
  for (const s of get().servers) pinging[s.id] = true;
  set({ pinging });
  clearTimeout(pingSafety);
  // results stream as "latency" events; clear stragglers if the backend never reports them
  pingSafety = setTimeout(() => set({ pinging: {} }), 15_000);
  const ok = await attempt(() => api.pingAll().then(() => true));
  if (!ok) set({ pinging: {} });
}

export async function pingServer(id: string) {
  set((s) => ({ pinging: { ...s.pinging, [id]: true } }));
  const latency = await attempt(() => api.pingServer(id));
  set((s) => {
    const pinging = { ...s.pinging };
    delete pinging[id];
    return latency === undefined ? { pinging } : { pinging, servers: mergeLatency(s.servers, { id, latency }) };
  });
}

export async function selectFastest() {
  const t = getT();
  const id = await attempt(() => api.selectFastest());
  if (!id) return;
  set({ selectedId: id });
  const srv = get().servers.find((s) => s.id === id);
  if (srv) toast.success(t("toast.fastest", { name: srv.name, ms: srv.latency ?? "?" }));
}

export async function renameServer(id: string, name: string) {
  const trimmed = name.trim();
  if (!trimmed) return;
  set((s) => ({ servers: s.servers.map((x) => (x.id === id ? { ...x, name: trimmed } : x)) }));
  await attempt(() => api.renameServer(id, trimmed));
}

export async function removeServer(id: string) {
  const t = getT();
  const srv = get().servers.find((s) => s.id === id);
  set((s) => ({ servers: s.servers.filter((x) => x.id !== id) }));
  const ok = await attempt(() => api.removeServer(id).then(() => true));
  if (ok && srv) toast(t("toast.serverDeleted", { name: srv.name }));
}

export async function copyServerLink(id: string) {
  const t = getT();
  const link = await attempt(() => api.exportLink(id));
  if (!link) return;
  const ok = await attempt(() => copyText(link).then(() => true));
  if (ok) toast.success(t("toast.copied"));
}

// ------------------------------------------------------------------ import

export function reportImport(res: ImportResult) {
  const t = getT();
  const parts: string[] = [];
  if (res.added) parts.push(t("toast.addedServers", { n: res.added }));
  if (res.subscriptionsAdded) parts.push(t("toast.addedSubs", { n: res.subscriptionsAdded }));
  const dup = res.duplicates ? t("toast.duplicates", { n: res.duplicates }) : "";
  const description = res.errors.length ? res.errors.slice(0, 3).join("\n") : undefined;
  if (parts.length) {
    toast.success([parts.join(" + "), dup].filter(Boolean).join(" · "), { description });
  } else if (res.duplicates) {
    toast(t("toast.allDuplicates"), { description: dup });
  } else {
    toast.error(t("toast.noLinks"), { description });
  }
}

export async function importText(text: string): Promise<boolean> {
  if (!text.trim()) return false;
  const res = await attempt(() => api.importText(text));
  if (!res) return false;
  reportImport(res);
  return res.added + res.subscriptionsAdded > 0;
}

export async function importClipboard(): Promise<boolean> {
  const res = await attempt(() => api.importClipboard());
  if (!res) return false;
  reportImport(res);
  return res.added + res.subscriptionsAdded > 0;
}

// ------------------------------------------------------------------ subscriptions

function markRefreshing(id: string, on: boolean) {
  set((s) => {
    const refreshing = { ...s.refreshing };
    if (on) refreshing[id] = true;
    else delete refreshing[id];
    return { refreshing };
  });
}

export async function refreshSubscription(id: string) {
  const t = getT();
  markRefreshing(id, true);
  const sub = await attempt(() => api.refreshSubscription(id));
  markRefreshing(id, false);
  if (sub) {
    set((s) => ({ subscriptions: s.subscriptions.map((x) => (x.id === id ? sub : x)) }));
    toast.success(t("toast.subRefreshed", { name: sub.name }));
  }
}

export async function refreshAllSubscriptions() {
  const t = getT();
  const ids = get().subscriptions.map((s) => s.id);
  ids.forEach((id) => markRefreshing(id, true));
  const ok = await attempt(() => api.refreshAllSubscriptions().then(() => true));
  ids.forEach((id) => markRefreshing(id, false));
  if (ok) toast.success(t("toast.subsRefreshed"));
}

export async function addSubscription(url: string, name?: string): Promise<boolean> {
  const t = getT();
  const sub = await attempt(() => api.addSubscription(url, name));
  if (!sub) return false;
  toast.success(t("toast.subAdded", { name: sub.name }));
  return true;
}

export async function saveSubscription(sub: Subscription): Promise<boolean> {
  const t = getT();
  set((s) => ({ subscriptions: s.subscriptions.map((x) => (x.id === sub.id ? sub : x)) }));
  const ok = await attempt(() => api.updateSubscription(sub).then(() => true));
  if (ok) toast.success(t("toast.subSaved"));
  return !!ok;
}

export async function removeSubscription(id: string) {
  const t = getT();
  set((s) => ({
    subscriptions: s.subscriptions.filter((x) => x.id !== id),
    servers: s.servers.filter((x) => x.subscriptionId !== id),
  }));
  const ok = await attempt(() => api.removeSubscription(id).then(() => true));
  if (ok) toast(t("toast.subDeleted"));
}

// ------------------------------------------------------------------ logs

export async function clearLogs() {
  set({ logs: [] });
  await attempt(() => api.clearLogs());
}
