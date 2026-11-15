use super::*;
use crate::links::parse_link;
use crate::routing::RoutingPreset;

/// One link per supported protocol/transport/security combination.
pub(crate) const SAMPLE_LINKS: &[(&str, &str)] = &[
    (
        "vless_reality_vision",
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@203.0.113.7:443?encryption=none&flow=xtls-rprx-vision\
         &security=reality&sni=www.microsoft.com&fp=chrome&pbk=SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc\
         &sid=6ba85179e30d4fc2&spx=%2F&type=tcp#Reality",
    ),
    (
        "vless_xhttp_reality",
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@203.0.113.7:443?type=xhttp&path=%2Fx&mode=stream-one\
         &security=reality&pbk=SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc&sni=a.com\
         &extra=%7B%22xPaddingBytes%22%3A%22100-1000%22%7D#XHTTP",
    ),
    (
        "vless_ws_tls",
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@cdn.example.com:443?type=ws&path=%2Fws&host=cdn.example.com\
         &security=tls&sni=cdn.example.com&alpn=h2,http/1.1&fp=firefox#WS",
    ),
    (
        "vmess_grpc_tls",
        "vmess://eyJ2IjoiMiIsInBzIjoiVk0iLCJhZGQiOiJ2bS5leGFtcGxlLmNvbSIsInBvcnQiOiI0NDMiLCJpZCI6ImI4MzEzODFkLTYzMjQtNGQ1My1hZDRmLThjZGE0OGIzMDgxMSIsImFpZCI6IjAiLCJzY3kiOiJhdXRvIiwibmV0IjoiZ3JwYyIsInR5cGUiOiJtdWx0aSIsImhvc3QiOiIiLCJwYXRoIjoic3ZjIiwidGxzIjoidGxzIiwic25pIjoidm0uZXhhbXBsZS5jb20ifQ==",
    ),
    ("trojan_httpupgrade", "trojan://pw@t.example.com:443?type=httpupgrade&path=%2Fu&host=t.example.com#TR"),
    ("trojan_tcp_http_header", "trojan://pw@t.example.com:80?security=none&type=tcp&headerType=http&host=a.com,b.com&path=%2Fp#H"),
    (
        "ss_2022",
        "ss://2022-blake3-aes-128-gcm:YctPZ6U7xPPcU%2Bgp3u%2B0tx%2FtRizJN9K8y%2BuKlW2qjlI%3D@198.51.100.1:443#SS",
    ),
    ("socks_auth", "socks://dXNlcjpwYXNz@198.51.100.2:1080#SOCKS"),
    (
        "hysteria2_full",
        "hysteria2://letmein@hy.example.com:443/?sni=hy.example.com&obfs=salamander&obfs-password=gawr\
         &mport=20000-30000&up=50&down=200#HY2",
    ),
    ("vless_kcp_seed", "vless://b831381d-6324-4d53-ad4f-8cda48b30811@k.example.com:9000?type=kcp&headerType=wechat-video&seed=s33d#KCP"),
];

fn sample(name: &str) -> Server {
    let link = SAMPLE_LINKS.iter().find(|(n, _)| *n == name).unwrap().1;
    parse_link(link).unwrap()
}

#[test]
fn snapshots_proxy_mode() {
    let settings = CoreSettings::default();
    for (name, link) in SAMPLE_LINKS {
        let server = parse_link(link).unwrap();
        let config = build_xray_config(&server, &settings).unwrap();
        insta::assert_json_snapshot!(format!("proxy_{name}"), config);
    }
}

#[test]
fn tun_mode_adds_tun_inbound_and_dns_hijack() {
    let settings = CoreSettings { mode: TunnelMode::Tun, ..Default::default() };
    let config = build_xray_config(&sample("vless_reality_vision"), &settings).unwrap();
    insta::assert_json_snapshot!("tun_vless_reality_vision", config);

    let inbounds = config["inbounds"].as_array().unwrap();
    assert_eq!(inbounds[0]["protocol"], "tun");
    // only the fields Xray 26 implements; routing is the helper's job
    assert_eq!(inbounds[0]["settings"], json!({ "name": settings.tun.name, "mtu": 1500 }));
    // split prefixes, never a competing default route
    assert!(!TUN_ROUTES_V4.contains(&"0.0.0.0/0"));
    assert!(TUN_ROUTES_V4.contains(&"128.0.0.0/1"));
    let rules = config["routing"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["outboundTag"], TAG_DNS_OUT);
    // IPv6 off: no AAAA answers and raw v6 refused instead of leaking
    assert_eq!(config["dns"]["queryStrategy"], "UseIPv4");
    assert_eq!(rules[1], json!({ "inboundTag": [TAG_TUN_IN], "ip": ["::/0"], "outboundTag": TAG_BLOCK }));

    let v6 = CoreSettings { mode: TunnelMode::Tun, tun: TunSettings { ipv6: true, ..Default::default() }, ..Default::default() };
    let config = build_xray_config(&sample("vless_reality_vision"), &v6).unwrap();
    assert_eq!(config["dns"]["queryStrategy"], "UseIP");
    assert!(!config["routing"]["rules"].as_array().unwrap().iter().any(|r| r["ip"] == json!(["::/0"])));
}

#[test]
fn proxy_is_first_outbound() {
    let config = build_xray_config(&sample("ss_2022"), &CoreSettings::default()).unwrap();
    assert_eq!(config["outbounds"][0]["tag"], "proxy");
}

#[test]
fn global_preset_has_no_direct_dns() {
    let mut settings = CoreSettings::default();
    settings.routing.preset = RoutingPreset::Global;
    let config = build_xray_config(&sample("ss_2022"), &settings).unwrap();
    assert_eq!(config["dns"]["servers"].as_array().unwrap().len(), 1);
}

#[test]
fn tun_direct_dns_resolves_everything_outside_the_tunnel() {
    let mut settings = CoreSettings { mode: TunnelMode::Tun, ..Default::default() };
    settings.dns.tun_direct = true;
    for preset in [RoutingPreset::Smart, RoutingPreset::Global] {
        settings.routing.preset = preset;
        let config = build_xray_config(&sample("ss_2022"), &settings).unwrap();
        assert_eq!(config["dns"]["servers"], json!(["77.88.8.8"]));
        let rules = config["routing"]["rules"].as_array().unwrap();
        assert!(rules.contains(&json!({ "ip": ["77.88.8.8"], "port": "53", "outboundTag": TAG_DIRECT })));
    }

    // proxy mode ignores the switch
    settings.mode = TunnelMode::Proxy;
    let config = build_xray_config(&sample("ss_2022"), &settings).unwrap();
    assert_eq!(config["dns"]["servers"][0], json!(settings.dns.remote));
}

#[test]
fn allow_lan_listens_everywhere() {
    let settings = CoreSettings { allow_lan: true, ..Default::default() };
    let config = build_xray_config(&sample("ss_2022"), &settings).unwrap();
    assert_eq!(config["inbounds"][0]["listen"], "0.0.0.0");
}

#[test]
fn custom_config_keeps_user_outbounds_and_replaces_inbounds() {
    let custom = json!({
        "remarks": "My JSON",
        "inbounds": [{ "port": 1080, "protocol": "socks" }],
        "outbounds": [
            { "tag": "proxy", "protocol": "vless", "settings": { "vnext": [{ "address": "x.com", "port": 443, "users": [{ "id": "b831381d-6324-4d53-ad4f-8cda48b30811", "encryption": "none" }] }] } },
            { "tag": "direct", "protocol": "freedom" }
        ],
        "routing": { "rules": [{ "domain": ["example.com"], "outboundTag": "direct" }] }
    });
    let server = Server::new("custom", Outbound::Custom { config: custom });
    assert_eq!(server.outbound.endpoint(), Some(("x.com".into(), 443)));

    let settings = CoreSettings { mode: TunnelMode::Tun, ..Default::default() };
    let config = build_xray_config(&server, &settings).unwrap();
    assert!(config.get("remarks").is_none());
    assert_eq!(config["dns"]["queryStrategy"], "UseIPv4", "custom DNS must not hand out AAAA in TUN mode");
    assert_eq!(config["inbounds"][0]["tag"], TAG_TUN_IN);
    assert_eq!(config["routing"]["rules"][0]["outboundTag"], TAG_DNS_OUT);
    assert_eq!(config["routing"]["rules"].as_array().unwrap().last().unwrap()["domain"][0], "example.com");
    assert!(config["outbounds"].as_array().unwrap().iter().any(|o| o["tag"] == TAG_DNS_OUT));

    let bad = Server::new("bad", Outbound::Custom { config: json!({ "outbounds": [] }) });
    assert!(build_xray_config(&bad, &settings).is_err());
}

#[test]
fn custom_config_gets_routing_preset_and_user_rules() {
    use crate::routing::{Rule, RuleKind, RuleTarget};
    // shaped like a Remnawave JSON subscription entry
    let custom = json!({
        "outbounds": [
            { "tag": "proxy-1", "protocol": "vless", "settings": { "vnext": [{ "address": "x.com", "port": 443, "users": [] }] } },
            { "tag": "direct", "protocol": "freedom" }
        ],
        "routing": { "rules": [{ "protocol": ["bittorrent"], "outboundTag": "direct" }] }
    });
    let server = Server::new("custom", Outbound::Custom { config: custom });
    let mut s = CoreSettings::default(); // Smart, region ru, ads blocked
    s.routing.rules.push(Rule {
        id: "r".into(),
        enabled: true,
        kind: RuleKind::Domain,
        values: vec!["openai.com".into()],
        target: RuleTarget::Proxy,
        note: None,
    });
    let config = build_xray_config(&server, &s).unwrap();
    let rules = config["routing"]["rules"].as_array().unwrap();

    // user rule first, mapped onto the config's own proxy tag
    let user = rules.iter().position(|r| r["domain"] == json!(["domain:openai.com"])).unwrap();
    assert_eq!(rules[user]["outboundTag"], "proxy-1");
    // Smart preset present, before the provider's own rules
    let ru = rules.iter().position(|r| r["domain"] == json!(["geosite:category-ru"])).unwrap();
    assert_eq!(rules[ru]["outboundTag"], "direct");
    assert!(rules.iter().any(|r| r["ip"] == json!(["geoip:ru"]) && r["outboundTag"] == "direct"));
    let provider = rules.iter().position(|r| r["protocol"] == json!(["bittorrent"])).unwrap();
    assert!(user < ru && ru < provider);
    // the referenced block outbound is added, and geoip matching can resolve
    assert!(config["outbounds"].as_array().unwrap().iter().any(|o| o["tag"] == "block"));
    assert_eq!(config["routing"]["domainStrategy"], "IPIfNonMatch");

    // Global keeps everything on the provider's default route
    s.routing.preset = RoutingPreset::Global;
    let config = build_xray_config(&server, &s).unwrap();
    assert!(!config["routing"]["rules"].as_array().unwrap().iter().any(|r| r["ip"] == json!(["geoip:ru"])));
}

#[test]
fn dns_ip_extraction() {
    assert_eq!(dns_ip("https://1.1.1.1/dns-query"), "1.1.1.1");
    assert_eq!(dns_ip("8.8.8.8"), "8.8.8.8");
    assert_eq!(dns_ip("udp://9.9.9.9:53"), "9.9.9.9");
    assert_eq!(dns_ip("2001:4860:4860::8888"), "2001:4860:4860::8888");
}

/// Runs `xray run -test` against every generated config. Needs the binary:
/// `XRAY_BIN=../../src-tauri/binaries/xray-<triple> cargo test -p aether-core -- --ignored`
#[test]
#[ignore]
fn xray_accepts_generated_configs() {
    let bin = std::env::var("XRAY_BIN").expect("set XRAY_BIN");
    let assets = std::env::var("XRAY_LOCATION_ASSET")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/resources").into());
    let dir = std::env::temp_dir().join(format!("aether-xray-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for preset in [RoutingPreset::Global, RoutingPreset::Smart, RoutingPreset::Direct] {
        for (name, link) in SAMPLE_LINKS {
            let mut settings = CoreSettings::default();
            settings.routing.preset = preset;
            let config = build_xray_config(&parse_link(link).unwrap(), &settings).unwrap();
            let path = dir.join(format!("{name}-{preset:?}.json"));
            std::fs::write(&path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
            let out = std::process::Command::new(&bin)
                .args(["run", "-test", "-c"])
                .arg(&path)
                .env("XRAY_LOCATION_ASSET", &assets)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
            assert!(text.contains("Configuration OK"), "{name} ({preset:?}) rejected:\n{text}");
        }
    }
}
