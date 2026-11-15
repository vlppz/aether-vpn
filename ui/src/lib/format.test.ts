import { describe, expect, it } from "vitest";
import { daysUntil, formatBytes, formatClock, formatDuration, formatRelative, formatSpeed, hostOf, isHttpUrl, latencyTone, splitBytes, splitSpeed } from "./format";

describe("formatBytes / splitBytes", () => {
  it("handles zero, negatives and non-finite input", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });
  it("keeps bytes as integers", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });
  it("uses binary units with one decimal below 100", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(45 * 1024 ** 3)).toBe("45.0 GB");
    expect(formatBytes(200 * 1024 ** 3)).toBe("200 GB");
    expect(formatBytes(3 * 1024 ** 4)).toBe("3.0 TB");
  });
  it("promotes values that would round to 1024", () => {
    expect(formatBytes(1024 * 1024 - 1)).toBe("1.0 MB");
  });
  it("splits value and unit", () => {
    expect(splitBytes(2.5 * 1024 * 1024)).toEqual({ value: "2.5", unit: "MB" });
  });
});

describe("formatSpeed", () => {
  it("appends /s", () => {
    expect(formatSpeed(0)).toBe("0 B/s");
    expect(formatSpeed(512)).toBe("512 B/s");
    expect(formatSpeed(12.34 * 1024 * 1024)).toBe("12.3 MB/s");
    expect(splitSpeed(150 * 1024)).toEqual({ value: "150", unit: "KB/s" });
  });
});

describe("formatDuration", () => {
  it("formats HH:MM:SS", () => {
    expect(formatDuration(0)).toBe("00:00:00");
    expect(formatDuration(999)).toBe("00:00:00");
    expect(formatDuration(61_000)).toBe("00:01:01");
    expect(formatDuration(3_723_000)).toBe("01:02:03");
  });
  it("clamps negatives and lets hours exceed 99", () => {
    expect(formatDuration(-5000)).toBe("00:00:00");
    expect(formatDuration(100 * 3600 * 1000)).toBe("100:00:00");
  });
});

describe("formatClock", () => {
  it("pads milliseconds", () => {
    const d = new Date(2026, 0, 2, 3, 4, 5, 6);
    expect(formatClock(d.getTime())).toBe("03:04:05.006");
  });
});

describe("formatRelative", () => {
  const now = Date.UTC(2026, 9, 20, 12, 0, 0);
  it("says now for tiny differences", () => {
    expect(formatRelative(now - 10_000, now, "en")).toBe("now");
  });
  it("picks sensible units in English", () => {
    expect(formatRelative(now - 5 * 60_000, now, "en")).toBe("5 minutes ago");
    expect(formatRelative(now - 2 * 3_600_000, now, "en")).toBe("2 hours ago");
    expect(formatRelative(now - 86_400_000, now, "en")).toBe("yesterday");
    expect(formatRelative(now + 12 * 86_400_000, now, "en")).toBe("in 12 days");
    expect(formatRelative(now - 90 * 86_400_000, now, "en")).toBe("3 months ago");
    expect(formatRelative(now + 400 * 86_400_000, now, "en")).toBe("next year");
  });
  it("localises to Russian", () => {
    expect(formatRelative(now + 12 * 86_400_000, now, "ru")).toBe("через 12 дней");
    expect(formatRelative(now - 2 * 3_600_000, now, "ru")).toBe("2 часа назад");
  });
});

describe("daysUntil", () => {
  it("returns fractional days, negative when past", () => {
    const now = 1_000_000_000_000;
    expect(daysUntil(now / 1000 + 86400 * 2, now)).toBeCloseTo(2);
    expect(daysUntil(now / 1000 - 43200, now)).toBeCloseTo(-0.5);
  });
});

describe("latencyTone", () => {
  it("grades latency", () => {
    expect(latencyTone(null)).toBe("none");
    expect(latencyTone(-1)).toBe("fail");
    expect(latencyTone(42)).toBe("good");
    expect(latencyTone(149)).toBe("good");
    expect(latencyTone(150)).toBe("warn");
    expect(latencyTone(399)).toBe("warn");
    expect(latencyTone(400)).toBe("bad");
  });
});

describe("url helpers", () => {
  it("extracts hosts and validates http(s)", () => {
    expect(hostOf("https://sub.example.com:8443/a?b=1")).toBe("sub.example.com:8443");
    expect(hostOf("not a url")).toBe("not a url");
    expect(isHttpUrl("https://x.y/z")).toBe(true);
    expect(isHttpUrl(" http://x.y ")).toBe(true);
    expect(isHttpUrl("vless://abc@x:443")).toBe(false);
    expect(isHttpUrl("")).toBe(false);
  });
});
