// In-browser fake backend so `pnpm dev` renders a fully working demo without Tauri.
// Behaviour mirrors what the Rust side is expected to do (see commands.ts).

import type { ArgsOf, Command, CommandMap, RetOf } from "./commands";
import { defaultSettings } from "./defaults";
import { emitMock } from "./events";
import type {
  ConnectionStatus,
  ImportResult,
  LogLevel,
  LogLine,
  Platform,
  ServerView,
  Settings,
  Snapshot,
  Subscription,
  TunnelMode,
} from "./types";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const rand = (min: number, max: number) => min + Math.random() * (max - min);
const randInt = (min: number, max: number) => Math.floor(rand(min, max + 1));
const pick = <T>(arr: readonly T[]): T => arr[Math.floor(Math.random() * arr.length)];
const GB = 1024 ** 3;

let seq = 0;
const uid = (p: string) => `${p}-${(++seq).toString(36)}${Math.random().toString(36).slice(2, 6)}`;
const uuid = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : "8c1f2a4e-3b7d-4e9a-9f10-5d2c6b8a7e41";

interface MockServer extends ServerView {
  city: string;
  /** typical TCP latency used to synthesise pings */
  base: number;
  /** pings fail ~half of the time */
  flaky?: boolean;
  /** connecting to it always fails — demonstrates the error state */
  broken?: boolean;
  link?: string;
}

type Seed = Omit<MockServer, "id" | "subscriptionId" | "latency"> & { latency?: number | null };

function seed(subscriptionId: string | undefined, list: Seed[]): MockServer[] {
  return list.map((s) => ({ ...s, id: uid("srv"), subscriptionId, latency: s.latency ?? null }));
}

// ---------------------------------------------------------------- state

const now = Date.now();
const settings: Settings = defaultSettings();
settings.core.routing.rules = [
  { id: uuid(), enabled: true, kind: "domain", values: ["geosite:category-ads-all", "doubleclick.net"], target: "block", note: "Ads" },
  { id: uuid(), enabled: true, kind: "domain", values: ["openai.com", "chatgpt.com", "claude.ai"], target: "proxy", note: "AI" },
  { id: uuid(), enabled: true, kind: "process", values: ["Telegram", "Discord"], target: "proxy" },
  { id: uuid(), enabled: false, kind: "ip", values: ["192.168.0.0/16", "geoip:private"], target: "direct", note: "Home network" },
];

const subscriptions: Subscription[] = [
  {
    id: "sub-nebula",
    name: "Nebula Premium",
    url: "https://sub.nebula-net.example/api/v1/client/subscribe?token=7f3a9c21e4b84d6f",
    autoUpdate: true,
    updateIntervalHours: 12,
    lastUpdated: Math.floor(now / 1000) - 2 * 3600 - 340,
    info: { upload: 3.2 * GB, download: 41.7 * GB, total: 200 * GB, expire: Math.floor(now / 1000) + 12 * 86400 + 5 * 3600 },
    announce: "🚀 New Hysteria2 nodes in Tokyo & Singapore. Scheduled maintenance Fri 03:00 UTC.",
    supportUrl: "https://t.me/nebula_support",
    webPageUrl: "https://nebula-net.example/account",
  },
  {
    id: "sub-polar",
    name: "Polar Free",
    url: "https://polar.example.net/s/QmF2ZTY0LXRva2Vu",
    userAgent: "Happ/1.0",
    autoUpdate: true,
    updateIntervalHours: 24,
    lastUpdated: Math.floor(now / 1000) - 26 * 3600,
    lastError: "HTTP 503 Service Unavailable (kept previous server list)",
    info: { upload: 0.4 * GB, download: 6.9 * GB, total: 0, expire: Math.floor(now / 1000) + 2 * 86400 + 3 * 3600 },
  },
];

let servers: MockServer[] = [
  ...seed("sub-nebula", [
    { name: "Frankfurt · Reality", protocol: "VLESS", transport: "tcp", security: "reality", address: "fra-1.nebula-net.example", port: 443, country: "DE", city: "Frankfurt", base: 38, latency: 41 },
    { name: "Amsterdam · XHTTP", protocol: "VLESS", transport: "xhttp", security: "reality", address: "ams-2.nebula-net.example", port: 443, country: "NL", city: "Amsterdam", base: 44, latency: 47 },
    { name: "Helsinki · Trojan WS", protocol: "Trojan", transport: "ws", security: "tls", address: "hel.nebula-net.example", port: 8443, country: "FI", city: "Helsinki", base: 52, latency: 55 },
    { name: "Stockholm · Reality", protocol: "VLESS", transport: "grpc", security: "reality", address: "sto.nebula-net.example", port: 443, country: "SE", city: "Stockholm", base: 49, latency: null },
    { name: "London · VMess", protocol: "VMess", transport: "ws", security: "tls", address: "lon.nebula-net.example", port: 443, country: "GB", city: "London", base: 63, latency: 66 },
    { name: "New York · Reality", protocol: "VLESS", transport: "tcp", security: "reality", address: "nyc.nebula-net.example", port: 443, country: "US", city: "New York", base: 112, latency: 118 },
    { name: "Tokyo · Hysteria2", protocol: "Hysteria2", transport: "quic", security: "tls", address: "tyo.nebula-net.example", port: 36712, country: "JP", city: "Tokyo", base: 236, latency: 241 },
    { name: "Singapore · Hysteria2", protocol: "Hysteria2", transport: "quic", security: "tls", address: "sgp.nebula-net.example", port: 36712, country: "SG", city: "Singapore", base: 204, latency: null },
  ]),
  ...seed("sub-polar", [
    { name: "🇹🇷 Istanbul SS", protocol: "Shadowsocks", transport: "tcp", security: "aead", address: "185.244.30.17", port: 8388, country: "TR", city: "Istanbul", base: 86, latency: 92 },
    { name: "🇰🇿 Almaty Reality", protocol: "VLESS", transport: "tcp", security: "reality", address: "kz1.polar.example.net", port: 443, country: "KZ", city: "Almaty", base: 121, latency: 415 },
    { name: "🇩🇪 Nuremberg Trojan", protocol: "Trojan", transport: "tcp", security: "tls", address: "de2.polar.example.net", port: 443, country: "DE", city: "Nuremberg", base: 42, latency: null },
    { name: "🇺🇸 Los Angeles VMess", protocol: "VMess", transport: "tcp", security: "none", address: "203.0.113.9", port: 10086, country: "US", city: "Los Angeles", base: 178, latency: -1, flaky: true, broken: true },
  ]),
  ...seed(undefined, [
    { name: "My VPS · Helsinki", protocol: "VLESS", transport: "xhttp", security: "reality", address: "95.217.44.120", port: 443, country: "FI", city: "Helsinki", base: 47, latency: 51 },
    { name: "Home relay", protocol: "Shadowsocks", transport: "tcp", security: "aead", address: "home.dyn.example", port: 8388, country: null, city: "Home", base: 18, latency: null },
  ]),
];

let selectedId: string | null = servers[0].id;
let status: ConnectionStatus = { state: "disconnected", mode: settings.core.mode };
let logs: LogLine[] = [];
const importedLinks = new Set<string>();

const platform: Platform = /Mac/i.test(navigator.userAgent)
  ? "macos"
  : /Win/i.test(navigator.userAgent)
    ? "windows"
    : "linux";

// ---------------------------------------------------------------- helpers

function view(s: MockServer): ServerView {
  return {
    id: s.id,
    name: s.name,
    subscriptionId: s.subscriptionId,
    protocol: s.protocol,
    transport: s.transport,
    security: s.security,
    address: s.address,
    port: s.port,
    country: s.country,
    latency: s.latency,
  };
}

function snapshot(): Snapshot {
  return {
    servers: servers.map(view),
    subscriptions: structuredClone(subscriptions),
    selectedId,
    status: { ...status },
    settings: structuredClone(settings),
    platform,
    appVersion: "0.1.0-demo",
    coreVersion: "26.1.13",
  };
}

const emitSnapshot = () => emitMock("snapshot", snapshot());
function setStatus(next: ConnectionStatus) {
  status = next;
  emitMock("status", { ...status });
}

function log(level: LogLevel, message: string) {
  const line: LogLine = { ts: Date.now(), level, message };
  logs.push(line);
  if (logs.length > 5000) logs = logs.slice(-5000);
  emitMock("log", line);
}

function selected(): MockServer | undefined {
  return servers.find((s) => s.id === selectedId);
}

function samplePing(s: MockServer): number {
  if (s.flaky && Math.random() < 0.6) return -1;
  return Math.max(4, Math.round(s.base * rand(0.88, 1.22)));
}

// ---------------------------------------------------------------- connection simulation

let connectToken = 0;
let trafficTimer: ReturnType<typeof setInterval> | undefined;
let logTimer: ReturnType<typeof setTimeout> | undefined;
let totalUp = 0;
let totalDown = 0;

function stopSession() {
  clearInterval(trafficTimer);
  clearTimeout(logTimer);
  trafficTimer = undefined;
  logTimer = undefined;
}

type Phase = "idle" | "browse" | "stream" | "burst";
let phase: Phase = "browse";
let phaseLeft = 6;
let curDown = 0;
let curUp = 0;

function nextTraffic() {
  if (--phaseLeft <= 0) {
    const r = Math.random();
    phase = r < 0.22 ? "idle" : r < 0.62 ? "browse" : r < 0.88 ? "stream" : "burst";
    phaseLeft = phase === "burst" ? randInt(3, 7) : randInt(5, 16);
  }
  let target: number;
  switch (phase) {
    case "idle":
      target = rand(1.5, 14) * 1024;
      break;
    case "browse":
      target = Math.random() < 0.35 ? rand(300, 2400) * 1024 : rand(25, 260) * 1024;
      break;
    case "stream":
      target = rand(2.6, 6.4) * 1024 * 1024;
      break;
    case "burst":
      target = rand(9, 26) * 1024 * 1024;
      break;
  }
  curDown = curDown * 0.3 + target * 0.7;
  const upTarget = curDown * rand(0.03, 0.11) + rand(1, 9) * 1024 + (Math.random() < 0.08 ? rand(400, 1800) * 1024 : 0);
  curUp = curUp * 0.4 + upTarget * 0.6;
  const down = Math.round(curDown);
  const up = Math.round(curUp);
  totalDown += down;
  totalUp += up;
  emitMock("traffic", { up, down, totalUp, totalDown });
}

const DOMAINS: { host: string; port: number; kind: "global" | "local" | "ads" }[] = [
  { host: "www.google.com", port: 443, kind: "global" },
  { host: "www.gstatic.com", port: 443, kind: "global" },
  { host: "fonts.googleapis.com", port: 443, kind: "global" },
  { host: "i.ytimg.com", port: 443, kind: "global" },
  { host: "rr3---sn-4g5e6nsz.googlevideo.com", port: 443, kind: "global" },
  { host: "api.github.com", port: 443, kind: "global" },
  { host: "github.githubassets.com", port: 443, kind: "global" },
  { host: "claude.ai", port: 443, kind: "global" },
  { host: "chatgpt.com", port: 443, kind: "global" },
  { host: "discord.com", port: 443, kind: "global" },
  { host: "149.154.167.51", port: 443, kind: "global" },
  { host: "pbs.twimg.com", port: 443, kind: "global" },
  { host: "www.instagram.com", port: 443, kind: "global" },
  { host: "open.spotify.com", port: 443, kind: "global" },
  { host: "1.1.1.1", port: 53, kind: "global" },
  { host: "yandex.ru", port: 443, kind: "local" },
  { host: "mc.yandex.ru", port: 443, kind: "local" },
  { host: "vk.com", port: 443, kind: "local" },
  { host: "sun9-74.userapi.com", port: 443, kind: "local" },
  { host: "www.gosuslugi.ru", port: 443, kind: "local" },
  { host: "online.sberbank.ru", port: 443, kind: "local" },
  { host: "googleads.g.doubleclick.net", port: 443, kind: "ads" },
  { host: "ad.mail.ru", port: 443, kind: "ads" },
  { host: "pagead2.googlesyndication.com", port: 443, kind: "ads" },
];

function accessLine(): string {
  const d = pick(DOMAINS);
  const r = settings.core.routing;
  let out = "proxy";
  if (r.preset === "direct") out = "direct";
  else if (d.kind === "local" && r.preset === "smart") out = "direct";
  if (d.kind === "ads" && r.blockAds) out = "block";
  const tun = status.mode === "tun";
  const inbound = tun ? "tun-in" : Math.random() < 0.7 ? "socks-in" : "http-in";
  const net = d.port === 53 || (d.host.includes("googlevideo") && Math.random() < 0.5) ? "udp" : "tcp";
  const port = randInt(49152, 65000);
  const from = inbound === "http-in" ? `127.0.0.1:${port}` : `${net}:${tun ? "172.19.0.1" : "127.0.0.1"}:${port}`;
  const dest = inbound === "http-in" ? `//${d.host}:${d.port}` : `${net}:${d.host}:${d.port}`;
  const sep = out === "block" && Math.random() < 0.5 ? "->" : ">>";
  return `from ${from} accepted ${dest} [${inbound} ${sep} ${out}]`;
}

function scheduleLog() {
  logTimer = setTimeout(
    () => {
      if (status.state !== "connected") return;
      const r = Math.random();
      if (settings.core.accessLog && r < 0.86) log("access", accessLine());
      else if (r < 0.93)
        log("info", pick([
          "[Info] transport/internet/splithttp: XHTTP is dialing to tcp:fra-1.nebula-net.example:443, mode stream-one",
          "[Info] proxy/vless/outbound: tunneling request to tcp:www.google.com:443 via fra-1.nebula-net.example:443",
          "[Info] app/dns: UDP:1.1.1.1:53 got answer: www.gstatic.com. TypeA -> [142.250.74.67] 18.402ms",
          "[Info] transport/internet/tcp: dialing TCP to tcp:95.217.44.120:443",
        ]));
      else if (r < 0.985)
        log("warning", pick([
          "[Warning] core: failed to handler mux client connection > context canceled",
          "[Warning] app/dispatcher: default route for tcp:10.0.0.12:8009",
          "[Warning] transport/internet/reality: REALITY: processed invalid connection from 127.0.0.1",
        ]));
      else log("error", "[Error] proxy/freedom: failed to open connection to tcp:203.0.113.77:443 > dial tcp 203.0.113.77:443: i/o timeout");
      scheduleLog();
    },
    phase === "burst" || phase === "browse" ? rand(90, 650) : rand(400, 1500),
  );
}

const IPS: Record<string, string> = {
  DE: "185.213.154.72",
  NL: "45.83.220.18",
  FI: "95.217.44.120",
  SE: "185.65.135.244",
  GB: "178.62.33.102",
  US: "104.244.76.13",
  JP: "45.76.214.9",
  SG: "139.180.143.52",
  TR: "185.244.30.17",
  KZ: "2.133.92.41",
};

async function connectSequence(): Promise<void> {
  const srv = selected();
  if (!srv) throw "No server selected";
  const token = ++connectToken;
  stopSession();
  setStatus({ state: "connecting", mode: settings.core.mode, serverId: srv.id });
  log("app", `Connecting to ${srv.name} (${srv.protocol}/${srv.transport}${srv.security !== "none" ? "+" + srv.security : ""}) in ${settings.core.mode.toUpperCase()} mode`);
  log("info", "Xray 26.1.13 (Xray, Penetrates Everything.) 4c2b7e1 (go1.25.4 darwin/arm64)");
  log("info", "[Info] infra/conf/serial: Reading config: &{Name:stdin: Format:json}");
  await sleep(1200);
  if (token !== connectToken) return;
  if (srv.broken) {
    const error = `dial tcp ${srv.address}:${srv.port}: i/o timeout`;
    log("error", `[Error] proxy/vmess/outbound: failed to find an available destination > ${error}`);
    log("app", "Connection failed, core stopped");
    setStatus({ state: "error", mode: settings.core.mode, serverId: srv.id, error });
    throw `Connection failed: ${error}`;
  }
  log("warning", "[Warning] core: Xray 26.1.13 started");
  if (settings.core.mode === "proxy" && settings.setSystemProxy)
    log("app", `System proxy set to 127.0.0.1:${settings.core.httpPort} (HTTP) / ${settings.core.socksPort} (SOCKS)`);
  if (settings.core.mode === "tun") log("app", `TUN interface ${settings.core.tun.name} up, MTU ${settings.core.tun.mtu}`);
  totalUp = 0;
  totalDown = 0;
  setStatus({ state: "connected", mode: settings.core.mode, serverId: srv.id, since: Date.now() });
  log("app", `Connected to ${srv.name}`);
  trafficTimer = setInterval(nextTraffic, 1000);
  scheduleLog();
  setTimeout(() => {
    if (token !== connectToken || status.state !== "connected") return;
    setStatus({
      ...status,
      exitIp: srv.country ? IPS[srv.country] ?? "193.32.127.66" : "93.184.216.34",
      exitCountry: srv.country ?? undefined,
      exitCity: srv.city,
    });
  }, 700);
}

async function disconnectSequence(): Promise<void> {
  if (status.state === "disconnected") return;
  ++connectToken;
  stopSession();
  setStatus({ ...status, state: "disconnecting" });
  await sleep(350);
  log("app", "Core stopped, system proxy restored");
  setStatus({ state: "disconnected", mode: settings.core.mode, serverId: selectedId ?? undefined });
}

// ---------------------------------------------------------------- import

const SCHEME_RE = /^(vless|vmess|trojan|ss|hysteria2|hy2|socks|tuic):\/\//i;
const PROTO: Record<string, string> = { vless: "VLESS", vmess: "VMess", trojan: "Trojan", ss: "Shadowsocks", hysteria2: "Hysteria2", hy2: "Hysteria2", socks: "SOCKS", tuic: "JSON" };
const DEMO_CLIPBOARD = [
  "vless://2f6e1c1a-8b6d-4f2e-9a3c-0b7d5e4f1a2b@waw.example.org:443?type=tcp&security=reality&sni=www.microsoft.com&fp=chrome&pbk=Zx9...#Warsaw%20Reality",
  "hysteria2://s3cret@hk.example.org:443?sni=hk.example.org#Hong%20Kong%20Hy2",
].join("\n");

const fakeServers = (subscriptionId: string, n: number): MockServer[] => {
  const pool: [string, string, number, string][] = [
    ["DE", "Berlin", 40, "VLESS"],
    ["NL", "Rotterdam", 46, "Trojan"],
    ["PL", "Warsaw", 55, "VLESS"],
    ["US", "Dallas", 140, "VMess"],
    ["JP", "Osaka", 250, "Hysteria2"],
    ["FI", "Espoo", 51, "Shadowsocks"],
  ];
  return Array.from({ length: n }, () => {
    const [country, city, base, protocol] = pick(pool);
    return {
      id: uid("srv"),
      subscriptionId,
      name: `${city} ${protocol}`,
      protocol,
      transport: protocol === "Hysteria2" ? "quic" : pick(["tcp", "ws", "grpc", "xhttp"]),
      security: protocol === "Shadowsocks" ? "aead" : pick(["tls", "reality"]),
      address: `${city.toLowerCase()}.example.org`,
      port: 443,
      country,
      city,
      base,
      latency: null,
    };
  });
};

function importLines(text: string): ImportResult {
  const res: ImportResult = { added: 0, duplicates: 0, subscriptionsAdded: 0, errors: [] };
  const lines = text.split(/[\r\n]+/).map((l) => l.trim()).filter(Boolean);
  for (const line of lines) {
    if (importedLinks.has(line)) {
      res.duplicates++;
      continue;
    }
    if (/^https?:\/\//i.test(line)) {
      const id = uid("sub");
      let host = line;
      try {
        host = new URL(line).host;
      } catch {
        /* keep raw */
      }
      subscriptions.push({ id, name: host, url: line, autoUpdate: true, updateIntervalHours: 24, lastUpdated: Math.floor(Date.now() / 1000), info: { upload: 0, download: 0, total: 50 * GB } });
      servers.push(...fakeServers(id, 3));
      importedLinks.add(line);
      res.subscriptionsAdded++;
      continue;
    }
    const m = SCHEME_RE.exec(line);
    if (!m) {
      if (line.length > 3) res.errors.push(`Unsupported line: ${line.length > 48 ? line.slice(0, 48) + "…" : line}`);
      continue;
    }
    const scheme = m[1].toLowerCase();
    let name = "Imported server";
    let host = "example.org";
    let port = 443;
    try {
      const u = new URL(line.replace(/^[a-z0-9]+:/i, "http:"));
      name = decodeURIComponent(u.hash.slice(1)) || u.hostname;
      host = u.hostname;
      port = Number(u.port) || 443;
    } catch {
      /* vmess base64 etc. */
    }
    servers.push({
      id: uid("srv"),
      name,
      protocol: PROTO[scheme] ?? "VLESS",
      transport: /type=(\w+)/.exec(line)?.[1] ?? (scheme.startsWith("hy") ? "quic" : "tcp"),
      security: /security=(\w+)/.exec(line)?.[1] ?? (scheme === "ss" ? "aead" : "tls"),
      address: host,
      port,
      country: null,
      city: name,
      base: randInt(40, 260),
      latency: null,
    });
    importedLinks.add(line);
    res.added++;
  }
  if (res.added || res.subscriptionsAdded) emitSnapshot();
  return res;
}

// ---------------------------------------------------------------- QR / links

function fakeQr(seedText: string): string {
  const n = 29;
  let h = 2166136261;
  for (let i = 0; i < seedText.length; i++) h = Math.imul(h ^ seedText.charCodeAt(i), 16777619);
  const rnd = () => {
    h ^= h << 13;
    h ^= h >>> 17;
    h ^= h << 5;
    return ((h >>> 0) % 1000) / 1000;
  };
  const finder = (x: number, y: number) => {
    const inBox = (cx: number, cy: number) => {
      const dx = x - cx;
      const dy = y - cy;
      if (dx < -1 || dy < -1 || dx > 7 || dy > 7) return null;
      if (dx === -1 || dy === -1 || dx === 7 || dy === 7) return false;
      const ring = Math.min(dx, dy, 6 - dx, 6 - dy);
      return ring !== 1;
    };
    return inBox(0, 0) ?? inBox(n - 7, 0) ?? inBox(0, n - 7);
  };
  let rects = "";
  for (let y = 0; y < n; y++)
    for (let x = 0; x < n; x++) {
      const f = finder(x, y);
      const on = f === null ? (x === 6 || y === 6 ? (x + y) % 2 === 0 : rnd() < 0.5) : f;
      if (on) rects += `<rect x="${x + 2}" y="${y + 2}" width="1" height="1"/>`;
    }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="#fff"/><g fill="#000">${rects}</g></svg>`;
}

function exportLink(s: MockServer): string {
  const name = encodeURIComponent(s.name);
  const hp = `${s.address}:${s.port}`;
  switch (s.protocol) {
    case "Trojan":
      return `trojan://Xk2p9QvL7mR4@${hp}?type=${s.transport}&security=${s.security}&sni=${s.address}#${name}`;
    case "Shadowsocks":
      return `ss://${btoa("2022-blake3-aes-128-gcm:dGhpc2lzYWRlbW9rZXk=")}@${hp}#${name}`;
    case "Hysteria2":
      return `hysteria2://p4ssw0rd@${hp}?sni=${s.address}&obfs=salamander#${name}`;
    case "VMess":
      return `vmess://${btoa(JSON.stringify({ v: "2", ps: s.name, add: s.address, port: String(s.port), id: uuid(), net: s.transport, tls: s.security === "tls" ? "tls" : "" }))}`;
    default:
      return `vless://${uuid()}@${hp}?type=${s.transport}&security=${s.security}&encryption=none&fp=chrome&sni=www.microsoft.com&pbk=SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc&sid=6ba85179e30d4fc2#${name}`;
  }
}

// ---------------------------------------------------------------- commands

type Handlers = { [K in Command]: (args: CommandMap[K]["args"]) => Promise<CommandMap[K]["ret"]> | CommandMap[K]["ret"] };

const handlers: Handlers = {
  get_snapshot: () => snapshot(),
  import_text: async ({ text }) => {
    await sleep(250);
    return importLines(text);
  },
  import_clipboard: async () => {
    let text = "";
    try {
      text = await navigator.clipboard.readText();
    } catch {
      /* permission denied in browsers — fall back to demo links */
    }
    if (!SCHEME_RE.test(text.trim()) && !/^https?:\/\//i.test(text.trim())) text = DEMO_CLIPBOARD;
    await sleep(200);
    return importLines(text);
  },
  add_subscription: async ({ url, name }) => {
    await sleep(900);
    if (!/^https?:\/\//i.test(url)) throw "Subscription URL must start with http:// or https://";
    const id = uid("sub");
    const sub: Subscription = {
      id,
      name: name || new URL(url).host,
      url,
      autoUpdate: true,
      updateIntervalHours: 24,
      lastUpdated: Math.floor(Date.now() / 1000),
      info: { upload: 0.1 * GB, download: 1.3 * GB, total: 100 * GB, expire: Math.floor(Date.now() / 1000) + 30 * 86400 },
    };
    subscriptions.push(sub);
    servers.push(...fakeServers(id, randInt(3, 5)));
    emitSnapshot();
    return structuredClone(sub);
  },
  refresh_subscription: async ({ id }) => {
    await sleep(rand(600, 1300));
    const sub = subscriptions.find((s) => s.id === id);
    if (!sub) throw "Subscription not found";
    sub.lastUpdated = Math.floor(Date.now() / 1000);
    sub.lastError = undefined;
    if (sub.info) sub.info.download += rand(0.05, 0.4) * GB;
    emitSnapshot();
    return structuredClone(sub);
  },
  refresh_all_subscriptions: async () => {
    await sleep(1400);
    for (const s of subscriptions) {
      s.lastUpdated = Math.floor(Date.now() / 1000);
      s.lastError = undefined;
    }
    emitSnapshot();
  },
  update_subscription: ({ subscription }) => {
    const i = subscriptions.findIndex((s) => s.id === subscription.id);
    if (i < 0) throw "Subscription not found";
    subscriptions[i] = structuredClone(subscription);
    emitSnapshot();
  },
  remove_subscription: ({ id }) => {
    const i = subscriptions.findIndex((s) => s.id === id);
    if (i >= 0) subscriptions.splice(i, 1);
    servers = servers.filter((s) => s.subscriptionId !== id);
    if (!selected()) selectedId = servers[0]?.id ?? null;
    emitSnapshot();
  },
  remove_server: ({ id }) => {
    servers = servers.filter((s) => s.id !== id);
    if (selectedId === id) selectedId = servers[0]?.id ?? null;
    emitSnapshot();
  },
  rename_server: ({ id, name }) => {
    const s = servers.find((x) => x.id === id);
    if (s) s.name = name;
    emitSnapshot();
  },
  select_server: async ({ id }) => {
    if (!servers.some((s) => s.id === id)) throw "Server not found";
    selectedId = id;
    emitSnapshot();
    if (status.state === "connected" || status.state === "connecting") await connectSequence();
    else if (status.state === "error") setStatus({ state: "disconnected", mode: settings.core.mode, serverId: id });
  },
  connect: () => connectSequence(),
  disconnect: () => disconnectSequence(),
  toggle_connection: () =>
    status.state === "connected" || status.state === "connecting" ? disconnectSequence() : connectSequence(),
  set_mode: async ({ mode }: { mode: TunnelMode }) => {
    settings.core.mode = mode;
    status = { ...status, mode };
    emitSnapshot();
    if (status.state === "connected") await connectSequence();
    else emitMock("status", { ...status });
  },
  save_settings: ({ settings: next }) => {
    Object.assign(settings, structuredClone(next));
    emitSnapshot();
  },
  ping_all: async () => {
    const jobs = servers.map((s, i) =>
      sleep(120 + i * 70 + rand(0, 420) + (s.flaky ? 900 : 0)).then(() => {
        s.latency = samplePing(s);
        emitMock("latency", { id: s.id, latency: s.latency });
      }),
    );
    await Promise.all(jobs);
  },
  ping_server: async ({ id }) => {
    const s = servers.find((x) => x.id === id);
    if (!s) throw "Server not found";
    await sleep(rand(200, 600));
    s.latency = samplePing(s);
    emitMock("latency", { id, latency: s.latency });
    return s.latency;
  },
  test_real_delay: async () => {
    if (status.state !== "connected") throw "Not connected";
    const s = selected();
    await sleep(rand(300, 900));
    return Math.round((s?.base ?? 80) * rand(2.2, 3.1) + rand(20, 60));
  },
  select_fastest: async () => {
    if (servers.every((s) => s.latency === null)) await handlers.ping_all(undefined);
    const best = servers
      .filter((s) => s.latency !== null && s.latency > 0)
      .sort((a, b) => (a.latency ?? 0) - (b.latency ?? 0))[0];
    if (!best) throw "No reachable servers";
    await handlers.select_server({ id: best.id });
    return best.id;
  },
  get_logs: () => logs.slice(),
  clear_logs: () => {
    logs = [];
  },
  export_link: ({ id }) => {
    const s = servers.find((x) => x.id === id);
    if (!s) throw "Server not found";
    return s.link ?? (s.link = exportLink(s));
  },
  server_qr: async ({ id }) => {
    await sleep(150);
    const link = handlers.export_link({ id }) as string;
    return fakeQr(link);
  },
  uninstall_vpn_service: async () => {
    await sleep(600);
  },
  quit_app: () => {
    emitMock("notice", { kind: "info", message: "Quit is not available in the browser demo" });
  },
};

// boot-time log history and a background "auto-update" notice
log("app", "Aether 0.1.0-demo started (browser mock backend)");
log("info", "Xray 26.1.13 (Xray, Penetrates Everything.) 4c2b7e1 (go1.25.4 darwin/arm64)");
log("app", `Loaded ${servers.length} servers from ${subscriptions.length} subscriptions`);
setTimeout(() => {
  const sub = subscriptions[0];
  if (!sub) return;
  sub.lastUpdated = Math.floor(Date.now() / 1000);
  emitSnapshot();
  emitMock("notice", { kind: "success", message: `Subscription “${sub.name}” auto-updated` });
}, 45_000);

export async function mockInvoke<K extends Command>(cmd: K, args: ArgsOf<K>): Promise<RetOf<K>> {
  const handler = handlers[cmd] as (a: ArgsOf<K>) => Promise<RetOf<K>> | RetOf<K>;
  await sleep(20 + Math.random() * 40);
  return handler(args);
}
