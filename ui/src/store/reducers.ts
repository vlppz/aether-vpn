// Pure state helpers — kept free of React/zustand so they are unit-testable.
import type { LatencyUpdate, LogLine, ServerView, Subscription, TrafficTick } from "../lib/types";
import { fuzzyMatchAll } from "../lib/fuzzy";

export const TRAFFIC_CAP = 90;
export const LOG_CAP = 5000;

export interface TrafficSample {
  /** unix ms when the tick was received */
  t: number;
  up: number;
  down: number;
}

/** Appends a sample, keeping at most `cap` newest entries (ring-buffer semantics). */
export function pushSample(buf: readonly TrafficSample[], tick: TrafficTick, t: number, cap = TRAFFIC_CAP): TrafficSample[] {
  const next = buf.length >= cap ? buf.slice(buf.length - cap + 1) : buf.slice();
  next.push({ t, up: Math.max(0, tick.up), down: Math.max(0, tick.down) });
  return next;
}

/** Applies one latency result. Returns the same array when nothing changed. */
export function mergeLatency(servers: ServerView[], upd: LatencyUpdate): ServerView[] {
  const i = servers.findIndex((s) => s.id === upd.id);
  if (i < 0 || servers[i].latency === upd.latency) return servers;
  const next = servers.slice();
  next[i] = { ...servers[i], latency: upd.latency };
  return next;
}

export function appendLogs(logs: readonly LogLine[], lines: readonly LogLine[], cap = LOG_CAP): LogLine[] {
  if (lines.length === 0) return logs as LogLine[];
  const total = logs.length + lines.length;
  if (total <= cap) return logs.concat(lines);
  if (lines.length >= cap) return lines.slice(lines.length - cap);
  return logs.slice(total - cap).concat(lines);
}

export type ServerSort = "default" | "latency" | "name";

/** latency order: measured ascending, then failed (-1), then untested */
function latencyKey(l: number | null): number {
  if (l === null) return Number.MAX_SAFE_INTEGER;
  if (l < 0) return Number.MAX_SAFE_INTEGER - 1;
  return l;
}

export function sortServers(servers: readonly ServerView[], sort: ServerSort): ServerView[] {
  const list = servers.slice();
  if (sort === "latency") list.sort((a, b) => latencyKey(a.latency) - latencyKey(b.latency));
  else if (sort === "name") list.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }));
  return list;
}

export function fastestServer(servers: readonly ServerView[]): ServerView | null {
  let best: ServerView | null = null;
  for (const s of servers) {
    if (s.latency !== null && s.latency > 0 && (best === null || s.latency < (best.latency ?? Infinity))) best = s;
  }
  return best;
}

export interface ServerGroup {
  /** subscription id, or MANUAL_GROUP */
  id: string;
  name: string;
  subscription?: Subscription;
  servers: ServerView[];
}

export const MANUAL_GROUP = "__manual__";

/** Groups servers by subscription (in subscription order); servers without one go to "manual" last. */
export function groupServers(servers: readonly ServerView[], subscriptions: readonly Subscription[], manualName: string): ServerGroup[] {
  const groups = new Map<string, ServerGroup>();
  for (const sub of subscriptions) groups.set(sub.id, { id: sub.id, name: sub.name, subscription: sub, servers: [] });
  const manual: ServerGroup = { id: MANUAL_GROUP, name: manualName, servers: [] };
  for (const s of servers) {
    const g = s.subscriptionId ? groups.get(s.subscriptionId) : undefined;
    (g ?? manual).servers.push(s);
  }
  const out = [...groups.values()].filter((g) => g.servers.length > 0 || g.subscription);
  if (manual.servers.length) out.push(manual);
  return out;
}

export function filterServers(servers: readonly ServerView[], query: string, countryName: (code: string | null) => string): ServerView[] {
  if (!query.trim()) return servers.slice();
  const scored: { s: ServerView; score: number }[] = [];
  for (const s of servers) {
    const score = fuzzyMatchAll(query, [s.name, s.country ?? "", countryName(s.country), s.protocol, s.transport, s.security, s.address ?? ""]);
    if (score !== null) scored.push({ s, score });
  }
  return scored.map((x) => x.s);
}
