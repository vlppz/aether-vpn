use super::*;
use crate::model::{Network, Security};

fn round_trip(link: &str) -> Server {
    let a = parse_link(link).unwrap_or_else(|e| panic!("parse {link}: {e}"));
    let exported = to_link(&a).unwrap();
    let b = parse_link(&exported).unwrap_or_else(|e| panic!("reparse {exported}: {e}"));
    assert_eq!(a.outbound, b.outbound, "round trip changed outbound via {exported}");
    assert_eq!(a.name, b.name);
    a
}

#[test]
fn vless_reality_vision() {
    let s = round_trip(
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@203.0.113.7:443?encryption=none&flow=xtls-rprx-vision\
         &security=reality&sni=www.microsoft.com&fp=chrome&pbk=SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc\
         &sid=6ba85179e30d4fc2&spx=%2F&type=tcp&headerType=none#%F0%9F%87%A9%F0%9F%87%AA%20Frankfurt",
    );
    assert_eq!(s.name, "🇩🇪 Frankfurt");
    assert_eq!(s.country().as_deref(), Some("DE"));
    let Outbound::Vless { flow, stream, port, .. } = &s.outbound else { panic!() };
    assert_eq!(flow.as_deref(), Some("xtls-rprx-vision"));
    assert_eq!(*port, 443);
    let Security::Reality(r) = &stream.security else { panic!() };
    assert_eq!(r.short_id.as_deref(), Some("6ba85179e30d4fc2"));
    assert_eq!(r.spider_x.as_deref(), Some("/"));
    assert!(matches!(stream.network, Network::Tcp { header_type: None, .. }));
}

#[test]
fn vless_ws_tls_ipv6() {
    let s = round_trip(
        "vless://uuid-1@[2001:db8::1]:8443?type=ws&path=%2Fws%3Fed%3D2048&host=cdn.example.com\
         &security=tls&sni=cdn.example.com&alpn=h2,http/1.1&fp=firefox#ws",
    );
    let Outbound::Vless { address, stream, .. } = &s.outbound else { panic!() };
    assert_eq!(address, "2001:db8::1");
    assert_eq!(
        stream.network,
        Network::Ws { path: "/ws?ed=2048".into(), host: Some("cdn.example.com".into()) }
    );
    let Security::Tls(tls) = &stream.security else { panic!() };
    assert_eq!(tls.alpn, vec!["h2", "http/1.1"]);
}

#[test]
fn vless_xhttp_with_extra_and_grpc() {
    let s = round_trip(
        "vless://id@example.com:443?type=xhttp&path=%2Fx&mode=stream-one&security=tls\
         &extra=%7B%22xPaddingBytes%22%3A%22100-1000%22%7D#x",
    );
    let Outbound::Vless { stream, .. } = &s.outbound else { panic!() };
    let Network::Xhttp { mode, extra, .. } = &stream.network else { panic!() };
    assert_eq!(mode.as_deref(), Some("stream-one"));
    assert_eq!(extra.as_ref().unwrap()["xPaddingBytes"], "100-1000");

    let g = round_trip("vless://id@example.com:443?type=grpc&serviceName=svc&mode=multi&security=tls#g");
    let Outbound::Vless { stream, .. } = &g.outbound else { panic!() };
    assert_eq!(
        stream.network,
        Network::Grpc { service_name: "svc".into(), authority: None, multi_mode: true }
    );
}

#[test]
fn vless_rejects_bad_input() {
    assert!(parse_link("vless://@host:443").is_err());
    assert!(parse_link("vless://id@host").is_err());
    assert!(parse_link("vless://id@host:99999").is_err());
    assert!(parse_link("vless://id@host:443?security=reality").is_err());
    assert!(parse_link("vless://id@host:443?type=quic").is_err());
    assert!(matches!(parse_link("wireguard://x@h:1"), Err(Error::UnsupportedScheme(_))));
}

#[test]
fn vmess_v2rayn() {
    let json = r#"{"v":"2","ps":"🇳🇱 NL vmess","add":"nl.example.com","port":443,"id":"a3482e88-686a-4a58-8126-99c9df64b7bf","aid":"0","scy":"auto","net":"ws","type":"none","host":"nl.example.com","path":"/vm","tls":"tls","sni":"nl.example.com","fp":"chrome"}"#;
    let link = format!("vmess://{}", encode_base64(json));
    let s = round_trip(&link);
    assert_eq!(s.name, "🇳🇱 NL vmess");
    let Outbound::Vmess { port, stream, .. } = &s.outbound else { panic!() };
    assert_eq!(*port, 443);
    assert!(matches!(stream.security, Security::Tls(_)));
}

#[test]
fn trojan_defaults_to_tls() {
    let s = round_trip("trojan://p%40ss@t.example.com:443?sni=t.example.com#Trojan");
    let Outbound::Trojan { password, stream, .. } = &s.outbound else { panic!() };
    assert_eq!(password, "p@ss");
    assert!(matches!(stream.security, Security::Tls(_)));
}

#[test]
fn shadowsocks_variants() {
    // SIP002 base64 userinfo
    let s = round_trip("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@198.51.100.1:8388#SS");
    let Outbound::Shadowsocks { method, password, .. } = &s.outbound else { panic!() };
    assert_eq!((method.as_str(), password.as_str()), ("aes-256-gcm", "password"));

    // 2022 cipher with percent-encoded plain userinfo
    let s = round_trip("ss://2022-blake3-aes-128-gcm:YctPZ6U7xPPcU%2Bgp3u%2B0tx%2FtRizJN9K8y%2BuKlW2qjlI%3D@h:443#2022");
    let Outbound::Shadowsocks { password, .. } = &s.outbound else { panic!() };
    assert_eq!(password, "YctPZ6U7xPPcU+gp3u+0tx/tRizJN9K8y+uKlW2qjlI=");

    // legacy fully-encoded form: base64("chacha20-ietf-poly1305:pw@1.2.3.4:8000")
    let legacy = format!("ss://{}#Legacy", encode_base64("chacha20-ietf-poly1305:pw@1.2.3.4:8000"));
    let s = parse_link(&legacy).unwrap();
    assert_eq!(s.outbound.endpoint(), Some(("1.2.3.4".into(), 8000)));

    assert!(parse_link("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@h:1?plugin=obfs-local#x").is_err());
}

#[test]
fn socks_forms() {
    let s = round_trip("socks://dXNlcjpwYXNz@127.0.0.1:1080#local");
    let Outbound::Socks { username, password, .. } = &s.outbound else { panic!() };
    assert_eq!(username.as_deref(), Some("user"));
    assert_eq!(password.as_deref(), Some("pass"));
    let s = parse_link("socks5://127.0.0.1:1080").unwrap();
    assert_eq!(s.name, "127.0.0.1:1080");
}

#[test]
fn hysteria2_with_obfs_and_hopping() {
    let s = round_trip(
        "hysteria2://letmein@hy.example.com:443,20000-30000/?sni=hy.example.com&obfs=salamander\
         &obfs-password=gawrgura&insecure=1#Hy2",
    );
    let Outbound::Hysteria2 { auth, obfs_password, hop_ports, tls, .. } = &s.outbound else { panic!() };
    assert_eq!(auth, "letmein");
    assert_eq!(obfs_password.as_deref(), Some("gawrgura"));
    assert_eq!(hop_ports.as_deref(), Some("443,20000-30000"));
    assert!(tls.allow_insecure);
    assert!(parse_link("hy2://pw@h:443").is_ok());
}

#[test]
fn parse_many_mixed_and_base64() {
    let text = "garbage line\nvless://id@a.com:443?security=none#A\r\nhttps://example.com\n\
                trojan://pw@b.com:443#B\nvless://broken";
    let batch = parse_many(text);
    assert_eq!(batch.servers.len(), 2);
    assert_eq!(batch.errors.len(), 1);

    let encoded = encode_base64("vless://id@a.com:443#A\nss://YWVzLTI1Ni1nY206cA@c.com:1#C\n");
    let batch = parse_many(&encoded);
    assert_eq!(batch.servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["A", "C"]);
}

#[test]
fn lenient_base64() {
    assert_eq!(decode_base64("aGVsbG8").as_deref(), Some("hello"));
    assert_eq!(decode_base64("aGVsbG8=").as_deref(), Some("hello"));
    assert_eq!(decode_base64("aGVs\nbG8=").as_deref(), Some("hello"));
    assert!(decode_base64("-_8").is_none()); // not valid UTF-8
}
