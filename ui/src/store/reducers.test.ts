import { describe, expect, it } from "vitest";
import type { LogLine, ServerView, Subscription } from "../lib/types";
import { appendLogs, fastestServer, filterServers, groupServers, MANUAL_GROUP, mergeLatency, pushSample, sortServers } from "./reducers";

const srv = (id: string, latency: number | null, extra: Partial<ServerView> = {}): ServerView => ({
  id,
  name: id,
  protocol: "VLESS",
  transport: "tcp",
  security: "reality",
  address: `${id}.example`,
  port: 443,
  country: "DE",
  latency,
  ...extra,
});

describe("pushSample (traffic ring buffer)", () => {
  it("appends without mutating", () => {
    const a = pushSample([], { up: 1, down: 2, totalUp: 1, totalDown: 2 }, 10);
    const b = pushSample(a, { up: 3, down: 4, totalUp: 4, totalDown: 6 }, 20);
    expect(a).toHaveLength(1);
    expect(b).toEqual([
      { t: 10, up: 1, down: 2 },
      { t: 20, up: 3, down: 4 },
    ]);
  });
  it("keeps only the newest `cap` samples", () => {
    let buf = pushSample([], { up: 0, down: 0, totalUp: 0, totalDown: 0 }, 0, 3);
    for (let i = 1; i < 10; i++) buf = pushSample(buf, { up: i, down: i, totalUp: 0, totalDown: 0 }, i, 3);
    expect(buf.map((s) => s.t)).toEqual([7, 8, 9]);
  });
  it("clamps negative speeds", () => {
    expect(pushSample([], { up: -5, down: -1, totalUp: 0, totalDown: 0 }, 0)[0]).toEqual({ t: 0, up: 0, down: 0 });
  });
});

describe("mergeLatency", () => {
  const list = [srv("a", null), srv("b", 50)];
  it("updates the matching server immutably", () => {
    const next = mergeLatency(list, { id: "a", latency: 120 });
    expect(next).not.toBe(list);
    expect(next[0].latency).toBe(120);
    expect(list[0].latency).toBeNull();
    expect(next[1]).toBe(list[1]);
  });
  it("returns the same array when nothing changes", () => {
    expect(mergeLatency(list, { id: "zzz", latency: 1 })).toBe(list);
    expect(mergeLatency(list, { id: "b", latency: 50 })).toBe(list);
  });
});

describe("appendLogs", () => {
  const line = (i: number): LogLine => ({ ts: i, level: "info", message: String(i) });
  it("caps the buffer", () => {
    const logs = Array.from({ length: 8 }, (_, i) => line(i));
    const next = appendLogs(logs, [line(8), line(9), line(10)], 10);
    expect(next).toHaveLength(10);
    expect(next[0].ts).toBe(1);
    expect(next[9].ts).toBe(10);
  });
  it("handles batches larger than the cap", () => {
    const next = appendLogs([line(0)], Array.from({ length: 20 }, (_, i) => line(i + 1)), 5);
    expect(next.map((l) => l.ts)).toEqual([16, 17, 18, 19, 20]);
  });
  it("returns the input when nothing is appended", () => {
    const logs = [line(1)];
    expect(appendLogs(logs, [])).toBe(logs);
  });
});

describe("sorting & grouping", () => {
  const list = [srv("slow", 300), srv("none", null), srv("fail", -1), srv("fast", 20), srv("Mid", 90)];
  it("sorts by latency: measured, failed, untested", () => {
    expect(sortServers(list, "latency").map((s) => s.id)).toEqual(["fast", "Mid", "slow", "fail", "none"]);
  });
  it("sorts by name case-insensitively and keeps default order", () => {
    expect(sortServers(list, "name").map((s) => s.id)).toEqual(["fail", "fast", "Mid", "none", "slow"]);
    expect(sortServers(list, "default")).toEqual(list);
  });
  it("finds the fastest reachable server", () => {
    expect(fastestServer(list)?.id).toBe("fast");
    expect(fastestServer([srv("x", -1), srv("y", null)])).toBeNull();
  });
  it("groups by subscription with manual last", () => {
    const subs: Subscription[] = [
      { id: "s1", name: "One", url: "https://a", autoUpdate: true, updateIntervalHours: 1 },
      { id: "s2", name: "Two", url: "https://b", autoUpdate: true, updateIntervalHours: 1 },
    ];
    const groups = groupServers([srv("m", 1), srv("b", 1, { subscriptionId: "s2" }), srv("a", 1, { subscriptionId: "s1" }), srv("orphan", 1, { subscriptionId: "gone" })], subs, "Manual");
    expect(groups.map((g) => g.id)).toEqual(["s1", "s2", MANUAL_GROUP]);
    expect(groups[2].servers.map((s) => s.id)).toEqual(["m", "orphan"]);
  });
  it("filters by name, country name and protocol", () => {
    const servers = [srv("Frankfurt", 1), srv("Tokyo", 1, { country: "JP", protocol: "Hysteria2" })];
    const name = (c: string | null) => (c === "JP" ? "Japan" : "Germany");
    expect(filterServers(servers, "japan", name).map((s) => s.id)).toEqual(["Tokyo"]);
    expect(filterServers(servers, "hy2", name).map((s) => s.id)).toEqual(["Tokyo"]);
    expect(filterServers(servers, "", name)).toHaveLength(2);
  });
});
