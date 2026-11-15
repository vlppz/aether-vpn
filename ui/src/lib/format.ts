import type { Language } from "./types";

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

export interface Split {
  value: string;
  unit: string;
}

/** Splits a byte count into a display value and a binary (1024) unit. */
export function splitBytes(bytes: number): Split {
  if (!Number.isFinite(bytes) || bytes <= 0) return { value: "0", unit: "B" };
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  if (i === 0) return { value: String(Math.round(v)), unit: "B" };
  // 1023.96 KB would round up to "1024.0" — promote it to the next unit instead
  if (v >= 999.95 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return { value: v >= 100 ? v.toFixed(0) : v.toFixed(1), unit: UNITS[i] };
}

export function formatBytes(bytes: number): string {
  const { value, unit } = splitBytes(bytes);
  return `${value} ${unit}`;
}

export function splitSpeed(bytesPerSecond: number): Split {
  const s = splitBytes(bytesPerSecond);
  return { value: s.value, unit: `${s.unit}/s` };
}

export function formatSpeed(bytesPerSecond: number): string {
  const { value, unit } = splitSpeed(bytesPerSecond);
  return `${value} ${unit}`;
}

const pad2 = (n: number) => String(n).padStart(2, "0");

/** HH:MM:SS (hours keep growing past 99). */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${pad2(h)}:${pad2(m)}:${pad2(s)}`;
}

/** HH:MM:SS.mmm wall-clock time for log lines. */
export function formatClock(ts: number): string {
  const d = new Date(ts);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, "0")}`;
}

const rtfCache = new Map<string, Intl.RelativeTimeFormat>();
function rtf(lang: Language): Intl.RelativeTimeFormat {
  let f = rtfCache.get(lang);
  if (!f) {
    f = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
    rtfCache.set(lang, f);
  }
  return f;
}

/** "3 hours ago" / "in 12 days" / "now" relative to `now` (both unix ms). */
export function formatRelative(ts: number, now: number, lang: Language): string {
  const diff = (ts - now) / 1000;
  const abs = Math.abs(diff);
  const f = rtf(lang);
  if (abs < 45) return f.format(0, "second");
  if (abs < 3600) return f.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return f.format(Math.round(diff / 3600), "hour");
  if (abs < 86400 * 30) return f.format(Math.round(diff / 86400), "day");
  if (abs < 86400 * 365) return f.format(Math.round(diff / (86400 * 30)), "month");
  return f.format(Math.round(diff / (86400 * 365)), "year");
}

/** Fractional days from now until a unix-seconds timestamp (negative when past). */
export function daysUntil(expireSec: number, now: number): number {
  return (expireSec * 1000 - now) / 86_400_000;
}

export type LatencyTone = "good" | "warn" | "bad" | "fail" | "none";

export function latencyTone(latency: number | null | undefined): LatencyTone {
  if (latency === null || latency === undefined) return "none";
  if (latency < 0) return "fail";
  if (latency < 150) return "good";
  if (latency < 400) return "warn";
  return "bad";
}

export function formatNumber(n: number, lang: Language): string {
  return new Intl.NumberFormat(lang).format(n);
}

/** Hostname of a URL, or the input when it is not a URL. */
export function hostOf(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

export function isHttpUrl(text: string): boolean {
  try {
    const u = new URL(text.trim());
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}
