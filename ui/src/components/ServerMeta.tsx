import type { ServerView } from "../lib/types";
import { cn } from "../lib/cn";

/** "VLESS · XHTTP · REALITY" — protocol, non-default transport and security as plain text. */
function serverMetaParts(s: Pick<ServerView, "protocol" | "transport" | "security">): string[] {
  const parts = [s.protocol === "Shadowsocks" ? "SS" : s.protocol];
  if (s.transport && s.transport !== "tcp") parts.push(s.transport);
  if (s.security && s.security !== "none") parts.push(s.security);
  else parts.push("plain");
  return parts;
}

export function ServerMeta({ server, className }: { server: Pick<ServerView, "protocol" | "transport" | "security">; className?: string }) {
  return <span className={cn("truncate text-[10.5px] font-medium tracking-[0.06em] text-ink-3 uppercase", className)}>{serverMetaParts(server).join(" · ")}</span>;
}
