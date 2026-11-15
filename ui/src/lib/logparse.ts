export interface AccessEntry {
  /** client address, e.g. 127.0.0.1:61634 */
  from: string;
  accepted: boolean;
  /** "tcp" | "udp" | "" (for `//host:port` HTTP-inbound destinations) */
  network: string;
  /** destination host:port */
  dest: string;
  inbound: string;
  /** outbound tag ("proxy", "direct", "block", …) */
  outbound: string;
  /** anything after the bracket, e.g. "email: user@x" */
  rest: string;
}

// Accepts both Xray styles:
//   from tcp:127.0.0.1:61634 accepted tcp:www.gstatic.com:443 [socks-in >> direct]
//   from 127.0.0.1:61637 accepted //www.gstatic.com:443 [http-in -> proxy]
// optionally preceded by a "YYYY/MM/DD hh:mm:ss.micro " timestamp.
const ACCESS_RE =
  /^(?:\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}:\d{2}(?:\.\d+)? )?from (?:(?:tcp|udp):)?(\S+) (accepted|rejected) (?:(tcp|udp):|\/\/)?(\S+?)(?: \[([^\]]*?)\s*(?:->|>>)\s*([^\]]*?)\])?(?:\s+(.*))?$/;

export function parseAccess(message: string): AccessEntry | null {
  const m = ACCESS_RE.exec(message.trim());
  if (!m) return null;
  return {
    from: m[1],
    accepted: m[2] === "accepted",
    network: m[3] ?? "",
    dest: m[4],
    inbound: m[5] ?? "",
    outbound: m[6] ?? "",
    rest: m[7] ?? "",
  };
}

export type OutboundTone = "proxy" | "direct" | "block" | "other";

export function outboundTone(tag: string): OutboundTone {
  const t = tag.toLowerCase();
  if (t === "direct" || t.startsWith("direct")) return "direct";
  if (t === "block" || t.startsWith("block") || t === "blackhole" || t === "reject") return "block";
  if (t === "proxy" || t.startsWith("proxy")) return "proxy";
  return "other";
}
