import type { Settings } from "./types";

export const DEFAULT_ACCENT = "#2f5fd0";

/** UI-side defaults, used before the first snapshot arrives and by the mock backend. */
export function defaultSettings(): Settings {
  return {
    core: {
      mode: "proxy",
      socksPort: 11808,
      httpPort: 11809,
      metricsPort: 11813,
      allowLan: false,
      sniffing: true,
      logLevel: "warning",
      accessLog: true,
      tun: { name: "aether0", mtu: 1500, ipv6: false },
      dns: { remote: "https://1.1.1.1/dns-query", direct: "77.88.8.8", tunDirect: false },
      routing: {
        preset: "smart",
        region: "ru",
        bypassLan: true,
        blockAds: true,
        domainStrategy: "IPIfNonMatch",
        rules: [],
      },
    },
    theme: "system",
    accent: DEFAULT_ACCENT,
    language: "en",
    launchAtLogin: false,
    startMinimized: false,
    autoConnect: false,
    setSystemProxy: true,
    closeToTray: true,
    userAgent: null,
  };
}
