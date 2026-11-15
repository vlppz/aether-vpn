import type {
  ImportResult,
  LogLine,
  Settings,
  Snapshot,
  Subscription,
  TunnelMode,
} from "./types";

/** Every backend command with its argument object and return type. */
export interface CommandMap {
  get_snapshot: { args: void; ret: Snapshot };
  import_text: { args: { text: string }; ret: ImportResult };
  import_clipboard: { args: void; ret: ImportResult };
  add_subscription: { args: { url: string; name?: string }; ret: Subscription };
  refresh_subscription: { args: { id: string }; ret: Subscription };
  refresh_all_subscriptions: { args: void; ret: void };
  update_subscription: { args: { subscription: Subscription }; ret: void };
  remove_subscription: { args: { id: string }; ret: void };
  remove_server: { args: { id: string }; ret: void };
  rename_server: { args: { id: string; name: string }; ret: void };
  select_server: { args: { id: string }; ret: void };
  connect: { args: void; ret: void };
  disconnect: { args: void; ret: void };
  toggle_connection: { args: void; ret: void };
  set_mode: { args: { mode: TunnelMode }; ret: void };
  save_settings: { args: { settings: Settings }; ret: void };
  ping_all: { args: void; ret: void };
  ping_server: { args: { id: string }; ret: number };
  test_real_delay: { args: void; ret: number };
  select_fastest: { args: void; ret: string };
  get_logs: { args: void; ret: LogLine[] };
  clear_logs: { args: void; ret: void };
  export_link: { args: { id: string }; ret: string };
  server_qr: { args: { id: string }; ret: string };
  uninstall_vpn_service: { args: void; ret: void };
  quit_app: { args: void; ret: void };
}

export type Command = keyof CommandMap;
export type ArgsOf<K extends Command> = CommandMap[K]["args"];
export type RetOf<K extends Command> = CommandMap[K]["ret"];
