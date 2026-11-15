//! VMess links use the v2rayN format: `vmess://base64(json)`.

use serde_json::{json, Value};

use super::{decode_base64, encode_base64};
use crate::error::{malformed, Result};
use crate::model::{Network, Outbound, Reality, Security, Server, Stream, Tls};

fn field(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub(super) fn parse(link: &str) -> Result<Server> {
    let body = link.split_once("://").map(|x| x.1).unwrap_or_default();
    let body = body.split('#').next().unwrap_or_default();
    let json = decode_base64(body).ok_or_else(|| malformed("vmess payload is not base64"))?;
    let v: Value = serde_json::from_str(&json).map_err(|e| malformed(format!("vmess JSON: {e}")))?;

    let address = field(&v, "add").ok_or_else(|| malformed("vmess without address"))?;
    let port: u16 = field(&v, "port")
        .and_then(|p| p.parse().ok())
        .filter(|p| *p != 0)
        .ok_or_else(|| malformed("vmess without valid port"))?;
    let id = field(&v, "id").ok_or_else(|| malformed("vmess without id"))?;

    let host = field(&v, "host");
    let path = field(&v, "path");
    let header_type = field(&v, "type").filter(|t| t != "none");
    let network = match field(&v, "net").as_deref().unwrap_or("tcp") {
        "tcp" | "raw" => Network::Tcp { header_type, host, path },
        "ws" => Network::Ws { path: path.unwrap_or_else(|| "/".into()), host },
        "grpc" => Network::Grpc {
            service_name: path.unwrap_or_default(),
            authority: host,
            multi_mode: header_type.as_deref() == Some("multi"),
        },
        "httpupgrade" => Network::HttpUpgrade { path: path.unwrap_or_else(|| "/".into()), host },
        "xhttp" | "splithttp" => Network::Xhttp {
            path: path.unwrap_or_else(|| "/".into()),
            host,
            mode: header_type.filter(|m| m != "auto"),
            extra: None,
        },
        "kcp" => Network::Kcp { header_type, seed: path },
        other => return Err(malformed(format!("unsupported vmess transport '{other}'"))),
    };

    let security = match field(&v, "tls").as_deref() {
        Some("tls") => Security::Tls(Tls {
            sni: field(&v, "sni"),
            alpn: field(&v, "alpn")
                .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            fingerprint: field(&v, "fp"),
            allow_insecure: matches!(field(&v, "allowInsecure").as_deref(), Some("1" | "true")),
            ..Default::default()
        }),
        Some("reality") => Security::Reality(Reality {
            sni: field(&v, "sni"),
            fingerprint: field(&v, "fp"),
            public_key: field(&v, "pbk").ok_or_else(|| malformed("reality without pbk"))?,
            short_id: field(&v, "sid"),
            spider_x: field(&v, "spx"),
            mldsa65_verify: None,
        }),
        _ => Security::None,
    };

    let outbound = Outbound::Vmess {
        address: address.clone(),
        port,
        id,
        alter_id: field(&v, "aid").and_then(|a| a.parse().ok()).unwrap_or(0),
        security: field(&v, "scy").unwrap_or_else(|| "auto".into()),
        stream: Stream { network, security, finalmask: None },
    };
    let name = field(&v, "ps").unwrap_or_else(|| format!("{address}:{port}"));
    Ok(Server::new(name, outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Vmess { address, port, id, alter_id, security, stream } = &server.outbound else {
        unreachable!()
    };
    let (net, kind, host, path) = match &stream.network {
        Network::Tcp { header_type, host, path } => ("tcp", header_type.clone(), host.clone(), path.clone()),
        Network::Ws { path, host } => ("ws", None, host.clone(), Some(path.clone())),
        Network::Grpc { service_name, authority, multi_mode } => (
            "grpc",
            multi_mode.then(|| "multi".to_string()),
            authority.clone(),
            Some(service_name.clone()),
        ),
        Network::HttpUpgrade { path, host } => ("httpupgrade", None, host.clone(), Some(path.clone())),
        Network::Xhttp { path, host, mode, .. } => ("xhttp", mode.clone(), host.clone(), Some(path.clone())),
        Network::Kcp { header_type, seed } => ("kcp", header_type.clone(), None, seed.clone()),
    };
    let mut v = json!({
        "v": "2",
        "ps": server.name,
        "add": address,
        "port": port.to_string(),
        "id": id,
        "aid": alter_id.to_string(),
        "scy": security,
        "net": net,
        "type": kind.unwrap_or_else(|| "none".into()),
        "host": host.unwrap_or_default(),
        "path": path.unwrap_or_default(),
        "tls": "",
    });
    match &stream.security {
        Security::None => {}
        Security::Tls(t) => {
            v["tls"] = "tls".into();
            v["sni"] = t.sni.clone().unwrap_or_default().into();
            v["alpn"] = t.alpn.join(",").into();
            v["fp"] = t.fingerprint.clone().unwrap_or_default().into();
            if t.allow_insecure {
                v["allowInsecure"] = "1".into();
            }
        }
        Security::Reality(r) => {
            v["tls"] = "reality".into();
            v["sni"] = r.sni.clone().unwrap_or_default().into();
            v["fp"] = r.fingerprint.clone().unwrap_or_default().into();
            v["pbk"] = r.public_key.clone().into();
            v["sid"] = r.short_id.clone().unwrap_or_default().into();
            v["spx"] = r.spider_x.clone().unwrap_or_default().into();
        }
    }
    Ok(format!("vmess://{}", encode_base64(&v.to_string())))
}
