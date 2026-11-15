//! Share-link import/export (`vless://`, `vmess://`, `trojan://`, `ss://`,
//! `socks://`, `hysteria2://`), following the XTLS share-link proposal
//! (XTLS/Xray-core discussion #716) and the de-facto formats used by panels.

mod hysteria2;
mod shadowsocks;
mod socks;
mod stream;
mod trojan;
mod vless;
mod vmess;

use std::collections::BTreeMap;

use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD, URL_SAFE};
use base64::engine::DecodePaddingMode;
use base64::Engine;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::error::{malformed, Error, Result};
use crate::model::{Outbound, Server};

/// Characters left unescaped in names and query values.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Parses one share link into a [`Server`].
pub fn parse_link(link: &str) -> Result<Server> {
    let link = link.trim();
    let (scheme, _) = link
        .split_once("://")
        .ok_or_else(|| malformed("missing scheme"))?;
    match scheme.to_ascii_lowercase().as_str() {
        "vless" => vless::parse(link),
        "vmess" => vmess::parse(link),
        "trojan" => trojan::parse(link),
        "ss" => shadowsocks::parse(link),
        "socks" | "socks5" => socks::parse(link),
        "hysteria2" | "hy2" => hysteria2::parse(link),
        other => Err(Error::UnsupportedScheme(other.to_string())),
    }
}

/// Serializes a server back into a share link.
pub fn to_link(server: &Server) -> Result<String> {
    match &server.outbound {
        Outbound::Vless { .. } => vless::export(server),
        Outbound::Vmess { .. } => vmess::export(server),
        Outbound::Trojan { .. } => trojan::export(server),
        Outbound::Shadowsocks { .. } => shadowsocks::export(server),
        Outbound::Socks { .. } => socks::export(server),
        Outbound::Hysteria2 { .. } => hysteria2::export(server),
        Outbound::Custom { .. } => Err(Error::NotExportable),
    }
}

/// Result of a bulk import: every recognised server plus a description of
/// each line that looked like a link but failed to parse.
#[derive(Debug, Default)]
pub struct ParsedBatch {
    pub servers: Vec<Server>,
    pub errors: Vec<String>,
}

/// Parses free-form text (one link per line, possibly mixed with junk,
/// possibly base64-encoded as a whole).
pub fn parse_many(text: &str) -> ParsedBatch {
    let text = text.trim();
    let decoded;
    let text = if !text.contains("://") {
        match decode_base64(text) {
            Some(d) => {
                decoded = d;
                decoded.as_str()
            }
            None => text,
        }
    } else {
        text
    };

    let mut batch = ParsedBatch::default();
    for token in text.split(['\n', '\r', ' ', '\t']) {
        let token = token.trim();
        if !token.contains("://") {
            continue;
        }
        match parse_link(token) {
            Ok(server) => batch.servers.push(server),
            Err(Error::UnsupportedScheme(s)) if !is_proxy_like(&s) => {}
            Err(e) => batch.errors.push(format!("{}: {e}", shorten(token))),
        }
    }
    batch
}

/// Schemes we skip silently in mixed text (plain web URLs etc).
fn is_proxy_like(scheme: &str) -> bool {
    !matches!(scheme, "http" | "https" | "ftp" | "mailto" | "file")
}

fn shorten(s: &str) -> String {
    if s.chars().count() > 48 {
        format!("{}…", s.chars().take(48).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Lenient base64 decoding: standard or URL-safe alphabet, with or without
/// padding, ignoring whitespace. Returns `None` unless the result is UTF-8.
pub(crate) fn decode_base64(input: &str) -> Option<String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return None;
    }
    let indifferent = GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    let engines = [
        GeneralPurpose::new(&base64::alphabet::STANDARD, indifferent),
        GeneralPurpose::new(&base64::alphabet::URL_SAFE, indifferent),
    ];
    engines
        .iter()
        .find_map(|e| e.decode(cleaned.as_bytes()).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

pub(crate) fn encode_base64(input: &str) -> String {
    STANDARD.encode(input)
}

pub(crate) fn encode_base64_url(input: &str) -> String {
    URL_SAFE.encode(input).trim_end_matches('=').to_string()
}

pub(crate) fn pct_decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

pub(crate) fn pct_encode(s: &str) -> String {
    utf8_percent_encode(s, COMPONENT).to_string()
}

/// Common pieces of `scheme://userinfo@host:port/path?query#fragment` links.
#[derive(Debug)]
pub(crate) struct LinkParts {
    pub userinfo: String,
    pub host: String,
    pub port: u16,
    pub query: BTreeMap<String, String>,
    pub name: String,
}

/// Splits a URL-style link without relying on a URL parser, which would
/// reject many real-world links (unescaped characters, port ranges, ...).
pub(crate) fn split_link(link: &str) -> Result<LinkParts> {
    let (_, rest) = link.split_once("://").ok_or_else(|| malformed("missing scheme"))?;
    let (rest, name) = match rest.split_once('#') {
        Some((r, n)) => (r, pct_decode(n)),
        None => (rest, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((r, q)) => (r, parse_query(q)),
        None => (rest, BTreeMap::new()),
    };
    let rest = rest.trim_end_matches('/');
    let (userinfo, hostport) = match rest.rsplit_once('@') {
        Some((u, h)) => (u.to_string(), h),
        None => (String::new(), rest),
    };
    let (host, port) = split_host_port(hostport)?;
    Ok(LinkParts { userinfo, host, port, query, name })
}

pub(crate) fn split_host_port(hostport: &str) -> Result<(String, u16)> {
    let hostport = hostport.split('/').next().unwrap_or(hostport);
    let (host, port) = if let Some(stripped) = hostport.strip_prefix('[') {
        let (h, p) = stripped
            .split_once(']')
            .ok_or_else(|| malformed("unterminated IPv6 address"))?;
        (h, p.trim_start_matches(':'))
    } else {
        hostport
            .rsplit_once(':')
            .ok_or_else(|| malformed("missing port"))?
    };
    if host.is_empty() {
        return Err(malformed("missing host"));
    }
    // Hysteria2 multi-port syntax `host:443,20000-30000`: take the first port.
    let first = port.split([',', '-']).next().unwrap_or(port);
    let port: u16 = first
        .parse()
        .map_err(|_| malformed(format!("invalid port '{port}'")))?;
    if port == 0 {
        return Err(malformed("port must not be 0"));
    }
    Ok((host.to_string(), port))
}

pub(crate) fn parse_query(q: &str) -> BTreeMap<String, String> {
    q.split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| match kv.split_once('=') {
            // '+' is kept literally: base64 keys (pbk, ech) frequently contain it
            Some((k, v)) => (pct_decode(k), pct_decode(v)),
            None => (pct_decode(kv), String::new()),
        })
        .collect()
}

pub(crate) fn build_query(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k}={}", pct_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) fn format_host(host: &str) -> String {
    if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

pub(crate) fn non_empty(v: Option<&String>) -> Option<String> {
    v.filter(|s| !s.is_empty()).cloned()
}

pub(crate) fn truthy(v: Option<&String>) -> bool {
    matches!(v.map(|s| s.to_ascii_lowercase()).as_deref(), Some("1" | "true" | "yes"))
}

pub(crate) fn display_name(name: String, host: &str, port: u16) -> String {
    if name.trim().is_empty() {
        format!("{host}:{port}")
    } else {
        name.trim().to_string()
    }
}

#[cfg(test)]
mod tests;
