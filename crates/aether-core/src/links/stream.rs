//! Transport + security query parameters shared by vless/trojan/ss links.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{non_empty, truthy};
use crate::error::{malformed, Result};
use crate::model::{Network, Reality, Security, Stream, Tls};

type Query = BTreeMap<String, String>;

/// Builds a [`Stream`] from link query parameters. `default_security` is
/// used when the link has no `security` key (trojan defaults to TLS).
pub(crate) fn parse(q: &Query, default_security: &str) -> Result<Stream> {
    let network = parse_network(q)?;
    let security = match q.get("security").map(String::as_str).unwrap_or(default_security) {
        "" | "none" => Security::None,
        "tls" => Security::Tls(parse_tls(q)),
        "reality" => Security::Reality(Reality {
            sni: non_empty(q.get("sni")),
            fingerprint: non_empty(q.get("fp")),
            public_key: non_empty(q.get("pbk"))
                .ok_or_else(|| malformed("reality link without public key (pbk)"))?,
            short_id: non_empty(q.get("sid")),
            spider_x: non_empty(q.get("spx")),
            mldsa65_verify: non_empty(q.get("pqv")),
        }),
        other => return Err(malformed(format!("unsupported security '{other}'"))),
    };
    let finalmask = q
        .get("fm")
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::from_str::<Value>(s))
        .transpose()
        .map_err(|e| malformed(format!("invalid fm JSON: {e}")))?;
    Ok(Stream { network, security, finalmask })
}

pub(crate) fn parse_tls(q: &Query) -> Tls {
    Tls {
        sni: non_empty(q.get("sni")).or_else(|| non_empty(q.get("peer"))),
        alpn: q
            .get("alpn")
            .map(|a| a.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
            .unwrap_or_default(),
        fingerprint: non_empty(q.get("fp")),
        allow_insecure: truthy(q.get("allowInsecure")) || truthy(q.get("insecure")),
        ech_config_list: non_empty(q.get("ech")),
        pinned_sha256: non_empty(q.get("pinSHA256")),
    }
}

fn parse_network(q: &Query) -> Result<Network> {
    let host = non_empty(q.get("host"));
    let path = non_empty(q.get("path"));
    let net = q.get("type").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    Ok(match net.as_str() {
        "" | "tcp" | "raw" => {
            let header_type = non_empty(q.get("headerType")).filter(|h| h != "none");
            Network::Tcp { header_type, host, path }
        }
        "ws" | "websocket" => Network::Ws { path: path.unwrap_or_else(|| "/".into()), host },
        "grpc" | "gun" => Network::Grpc {
            service_name: non_empty(q.get("serviceName")).or(path).unwrap_or_default(),
            authority: non_empty(q.get("authority")),
            multi_mode: q.get("mode").map(String::as_str) == Some("multi"),
        },
        "httpupgrade" => Network::HttpUpgrade { path: path.unwrap_or_else(|| "/".into()), host },
        "xhttp" | "splithttp" => Network::Xhttp {
            path: path.unwrap_or_else(|| "/".into()),
            host,
            mode: non_empty(q.get("mode")).filter(|m| m != "auto"),
            extra: q
                .get("extra")
                .filter(|s| !s.is_empty())
                .map(|s| serde_json::from_str(s))
                .transpose()
                .map_err(|e| malformed(format!("invalid xhttp extra JSON: {e}")))?,
        },
        "kcp" | "mkcp" => Network::Kcp {
            header_type: non_empty(q.get("headerType")).filter(|h| h != "none"),
            seed: non_empty(q.get("seed")),
        },
        "http" | "h2" | "quic" => {
            return Err(malformed(format!("transport '{net}' was removed from Xray-core")))
        }
        other => return Err(malformed(format!("unknown transport '{other}'"))),
    })
}

/// Inverse of [`parse`]: query pairs describing the stream.
pub(crate) fn export(stream: &Stream) -> Vec<(&'static str, String)> {
    let mut q: Vec<(&'static str, String)> = vec![("type", stream.network.name().to_string())];
    match &stream.network {
        Network::Tcp { header_type, host, path } => {
            q.push(("headerType", header_type.clone().unwrap_or_default()));
            q.push(("host", host.clone().unwrap_or_default()));
            q.push(("path", path.clone().unwrap_or_default()));
        }
        Network::Ws { path, host } | Network::HttpUpgrade { path, host } => {
            q.push(("path", path.clone()));
            q.push(("host", host.clone().unwrap_or_default()));
        }
        Network::Grpc { service_name, authority, multi_mode } => {
            q.push(("serviceName", service_name.clone()));
            q.push(("authority", authority.clone().unwrap_or_default()));
            if *multi_mode {
                q.push(("mode", "multi".into()));
            }
        }
        Network::Xhttp { path, host, mode, extra } => {
            q.push(("path", path.clone()));
            q.push(("host", host.clone().unwrap_or_default()));
            q.push(("mode", mode.clone().unwrap_or_default()));
            q.push(("extra", extra.as_ref().map(Value::to_string).unwrap_or_default()));
        }
        Network::Kcp { header_type, seed } => {
            q.push(("headerType", header_type.clone().unwrap_or_default()));
            q.push(("seed", seed.clone().unwrap_or_default()));
        }
    }
    match &stream.security {
        Security::None => q.push(("security", "none".into())),
        Security::Tls(tls) => {
            q.push(("security", "tls".into()));
            q.extend(export_tls(tls));
        }
        Security::Reality(r) => {
            q.push(("security", "reality".into()));
            q.push(("sni", r.sni.clone().unwrap_or_default()));
            q.push(("fp", r.fingerprint.clone().unwrap_or_default()));
            q.push(("pbk", r.public_key.clone()));
            q.push(("sid", r.short_id.clone().unwrap_or_default()));
            q.push(("spx", r.spider_x.clone().unwrap_or_default()));
            q.push(("pqv", r.mldsa65_verify.clone().unwrap_or_default()));
        }
    }
    if let Some(fm) = &stream.finalmask {
        q.push(("fm", fm.to_string()));
    }
    q
}

pub(crate) fn export_tls(tls: &Tls) -> Vec<(&'static str, String)> {
    vec![
        ("sni", tls.sni.clone().unwrap_or_default()),
        ("alpn", tls.alpn.join(",")),
        ("fp", tls.fingerprint.clone().unwrap_or_default()),
        ("allowInsecure", if tls.allow_insecure { "1".into() } else { String::new() }),
        ("ech", tls.ech_config_list.clone().unwrap_or_default()),
        ("pinSHA256", tls.pinned_sha256.clone().unwrap_or_default()),
    ]
}
