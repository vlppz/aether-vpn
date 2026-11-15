import type { ConnectionState } from "../../lib/types";
import { cn } from "../../lib/cn";

const color: Record<ConnectionState, string> = {
  connected: "bg-ok",
  connecting: "bg-warn",
  disconnecting: "bg-warn",
  disconnected: "bg-ink-3/60",
  error: "bg-danger",
};

/** 6px state dot; pulses only while a transition is in progress. */
export function StatusDot({ state, className }: { state: ConnectionState; className?: string }) {
  const busy = state === "connecting" || state === "disconnecting";
  return <span aria-hidden className={cn("inline-block size-1.5 shrink-0 rounded-full", color[state], busy && "animate-[breathe_1s_ease-in-out_infinite]", className)} />;
}
