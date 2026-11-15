import { describe, expect, it } from "vitest";
import { outboundTone, parseAccess } from "./logparse";

describe("parseAccess", () => {
  it("parses Xray 26 lines with tcp: prefixes and >>", () => {
    expect(parseAccess("from tcp:127.0.0.1:61634 accepted tcp:www.gstatic.com:443 [socks-in >> direct]")).toEqual({
      from: "127.0.0.1:61634",
      accepted: true,
      network: "tcp",
      dest: "www.gstatic.com:443",
      inbound: "socks-in",
      outbound: "direct",
      rest: "",
    });
  });
  it("parses HTTP inbound // destinations", () => {
    const e = parseAccess("from 127.0.0.1:61637 accepted //www.gstatic.com:443 [http-in >> proxy]");
    expect(e).toMatchObject({ from: "127.0.0.1:61637", network: "", dest: "www.gstatic.com:443", inbound: "http-in", outbound: "proxy" });
  });
  it("accepts -> separators, udp, timestamps and trailing email", () => {
    const e = parseAccess("2026/10/20 10:58:39.544132 from udp:172.19.0.1:5353 accepted udp:1.1.1.1:53 [tun-in -> block] email: a@b");
    expect(e).toMatchObject({ network: "udp", dest: "1.1.1.1:53", inbound: "tun-in", outbound: "block", rest: "email: a@b" });
  });
  it("handles rejected lines without brackets", () => {
    expect(parseAccess("from 127.0.0.1:1 rejected tcp:x.com:80")).toMatchObject({ accepted: false, dest: "x.com:80", outbound: "" });
  });
  it("returns null for other messages", () => {
    expect(parseAccess("[Warning] core: Xray 26.1.13 started")).toBeNull();
  });
});

describe("outboundTone", () => {
  it("classifies tags", () => {
    expect(outboundTone("proxy")).toBe("proxy");
    expect(outboundTone("direct")).toBe("direct");
    expect(outboundTone("block")).toBe("block");
    expect(outboundTone("blackhole")).toBe("block");
    expect(outboundTone("warp")).toBe("other");
  });
});
