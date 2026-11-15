import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./env";
import type { ArgsOf, Command, RetOf } from "./commands";
import type { Settings, Subscription, TunnelMode } from "./types";

type CallArgs<K extends Command> = ArgsOf<K> extends void ? [] : [ArgsOf<K>];

async function call<K extends Command>(cmd: K, ...args: CallArgs<K>): Promise<RetOf<K>> {
  if (isTauri) {
    return invoke<RetOf<K>>(cmd, args[0] as Record<string, unknown> | undefined);
  }
  // Loaded lazily so the mock never ships in the Tauri code path's critical chunk.
  const { mockInvoke } = await import("./mock");
  return mockInvoke(cmd, args[0] as ArgsOf<K>);
}

export const api = {
  getSnapshot: () => call("get_snapshot"),
  importText: (text: string) => call("import_text", { text }),
  importClipboard: () => call("import_clipboard"),
  addSubscription: (url: string, name?: string) => call("add_subscription", name ? { url, name } : { url }),
  refreshSubscription: (id: string) => call("refresh_subscription", { id }),
  refreshAllSubscriptions: () => call("refresh_all_subscriptions"),
  updateSubscription: (subscription: Subscription) => call("update_subscription", { subscription }),
  removeSubscription: (id: string) => call("remove_subscription", { id }),
  removeServer: (id: string) => call("remove_server", { id }),
  renameServer: (id: string, name: string) => call("rename_server", { id, name }),
  selectServer: (id: string) => call("select_server", { id }),
  connect: () => call("connect"),
  disconnect: () => call("disconnect"),
  toggleConnection: () => call("toggle_connection"),
  setMode: (mode: TunnelMode) => call("set_mode", { mode }),
  saveSettings: (settings: Settings) => call("save_settings", { settings }),
  pingAll: () => call("ping_all"),
  pingServer: (id: string) => call("ping_server", { id }),
  testRealDelay: () => call("test_real_delay"),
  selectFastest: () => call("select_fastest"),
  getLogs: () => call("get_logs"),
  clearLogs: () => call("clear_logs"),
  exportLink: (id: string) => call("export_link", { id }),
  serverQr: (id: string) => call("server_qr", { id }),
  uninstallVpnService: () => call("uninstall_vpn_service"),
  quitApp: () => call("quit_app"),
};

/** Best-effort human message from whatever invoke() rejected with. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export async function openExternal(url: string): Promise<void> {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

export async function copyText(text: string): Promise<void> {
  if (isTauri) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  } else {
    await navigator.clipboard.writeText(text);
  }
}

/** Window controls; no-ops in the browser demo. */
export const windowControls = {
  async minimize() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  },
  async toggleMaximize() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().toggleMaximize();
  },
  async close() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  },
};
