//! Subscription download and decoding.
//!
//! Supported bodies: base64 of newline-separated links, plain links, a JSON
//! Xray config, or a JSON array of Xray configs (Remnawave/Marzban "JSON"
//! subscriptions). Metadata comes from the de-facto headers
//! (`subscription-userinfo`, `profile-title`, `profile-update-interval`,
//! `support-url`, `profile-web-page-url`, `announce`) or from equivalent
//! `#key: value` lines at the top of the body.

use serde_json::Value;

#[cfg(feature = "fetch")]
use crate::error::{Error, Result};
use crate::links::{decode_base64, parse_many};
use crate::model::{Outbound, Server, UserInfo};

pub const DEFAULT_USER_AGENT: &str = concat!("Aether/", env!("CARGO_PKG_VERSION"), " (Xray-core)");

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub info: Option<UserInfo>,
    pub update_interval_hours: Option<u32>,
    pub announce: Option<String>,
    pub support_url: Option<String>,
    pub web_page_url: Option<String>,
}

impl Metadata {
    /// Fills fields that are still empty from `other`.
    pub fn merge(&mut self, other: Metadata) {
        self.title = self.title.take().or(other.title);
        self.info = self.info.take().or(other.info);
        self.update_interval_hours = self.update_interval_hours.or(other.update_interval_hours);
        self.announce = self.announce.take().or(other.announce);
        self.support_url = self.support_url.take().or(other.support_url);
        self.web_page_url = self.web_page_url.take().or(other.web_page_url);
    }

    fn set(&mut self, key: &str, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "profile-title" => self.title = Some(maybe_base64(value)),
            "subscription-userinfo" => self.info = parse_userinfo(value),
            "profile-update-interval" => self.update_interval_hours = value.parse().ok().filter(|h| *h > 0),
            "announce" => self.announce = Some(maybe_base64(value)),
            "support-url" => self.support_url = Some(value.to_string()),
            "profile-web-page-url" => self.web_page_url = Some(value.to_string()),
            "content-disposition" if self.title.is_none() => self.title = filename_from_disposition(value),
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
pub struct Decoded {
    pub meta: Metadata,
    pub servers: Vec<Server>,
    pub errors: Vec<String>,
}

/// `base64:...` values are used by panels for non-ASCII titles.
fn maybe_base64(v: &str) -> String {
    v.strip_prefix("base64:")
        .and_then(decode_base64)
        .unwrap_or_else(|| v.to_string())
}

fn filename_from_disposition(v: &str) -> Option<String> {
    let mut plain = None;
    for part in v.split(';').map(str::trim) {
        if let Some(enc) = part.strip_prefix("filename*=") {
            let enc = enc.split("''").last().unwrap_or(enc);
            return Some(crate::links::pct_decode(enc.trim_matches('"')));
        }
        if let Some(name) = part.strip_prefix("filename=") {
            plain = Some(name.trim_matches('"').to_string());
        }
    }
    plain.filter(|s| !s.is_empty())
}

/// Parses `upload=1; download=2; total=3; expire=4`.
pub fn parse_userinfo(v: &str) -> Option<UserInfo> {
    let mut info = UserInfo::default();
    let mut any = false;
    for part in v.split(';') {
        let Some((k, val)) = part.split_once('=') else { continue };
        // some panels emit floats like `1.073741824e+09`
        let n = val.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
        let Some(n) = n else { continue };
        any = true;
        match k.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload = n as u64,
            "download" => info.download = n as u64,
            "total" => info.total = n as u64,
            "expire" => info.expire = (n > 0.0).then_some(n as i64),
            _ => {}
        }
    }
    any.then_some(info)
}

/// Decodes a subscription body (without HTTP headers).
pub fn decode_body(body: &str) -> Decoded {
    let body = body.trim_start_matches('\u{feff}').trim();
    let mut out = Decoded::default();

    if body.starts_with('[') || body.starts_with('{') {
        match serde_json::from_str::<Value>(body) {
            Ok(v) => {
                decode_json(v, &mut out);
                return out;
            }
            Err(e) => out.errors.push(format!("invalid JSON: {e}")),
        }
    }

    // Metadata lines (`#profile-title: x`) may precede plain-text links.
    let text = if body.contains("://") {
        body.to_string()
    } else {
        decode_base64(body).unwrap_or_else(|| body.to_string())
    };
    for line in text.lines() {
        if let Some((k, v)) = line.trim().strip_prefix('#').and_then(|l| l.split_once(':')) {
            if !v.trim_start().starts_with("//") {
                out.meta.set(k, v);
            }
        }
    }
    let batch = parse_many(&text);
    out.servers = batch.servers;
    out.errors.extend(batch.errors);
    out
}

fn decode_json(v: Value, out: &mut Decoded) {
    let configs = match v {
        Value::Array(items) => items,
        obj @ Value::Object(_) => vec![obj],
        _ => {
            out.errors.push("JSON subscription must be an object or array".into());
            return;
        }
    };
    for (i, config) in configs.into_iter().enumerate() {
        if !config.get("outbounds").is_some_and(Value::is_array) {
            out.errors.push(format!("config #{} has no outbounds (not an Xray config?)", i + 1));
            continue;
        }
        let name = config
            .get("remarks")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("Config {}", i + 1));
        out.servers.push(Server::new(name, Outbound::Custom { config }));
    }
}

/// Header lookup abstraction so decoding stays independent of the HTTP client.
pub fn metadata_from_headers<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> Metadata {
    let mut meta = Metadata::default();
    for (k, v) in headers {
        meta.set(k, v);
    }
    meta
}

/// Downloads and decodes a subscription.
#[cfg(feature = "fetch")]
pub async fn fetch(url: &str, user_agent: Option<&str>, proxy: Option<&str>) -> Result<Decoded> {
    let mut builder = reqwest::Client::builder()
        .user_agent(user_agent.unwrap_or(DEFAULT_USER_AGENT))
        .timeout(std::time::Duration::from_secs(20));
    if let Some(proxy) = proxy {
        let proxy = reqwest::Proxy::all(proxy).map_err(|e| Error::Subscription(e.to_string()))?;
        builder = builder.proxy(proxy);
    }
    let client = builder.build().map_err(|e| Error::Subscription(e.to_string()))?;
    let resp = client
        .get(url)
        .header("Accept", "*/*")
        .send()
        .await
        .map_err(|e| Error::Subscription(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Subscription(format!("server returned HTTP {status}")));
    }
    let header_meta = metadata_from_headers(
        resp.headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str(), v))),
    );
    let body = resp.text().await.map_err(|e| Error::Subscription(e.to_string()))?;
    let mut decoded = decode_body(&body);
    let body_meta = std::mem::take(&mut decoded.meta);
    decoded.meta = header_meta;
    decoded.meta.merge(body_meta);
    if decoded.servers.is_empty() {
        let hint = decoded.errors.first().cloned().unwrap_or_else(|| "no servers found".into());
        return Err(Error::Subscription(hint));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::encode_base64;

    #[test]
    fn userinfo_header() {
        let info = parse_userinfo("upload=455727941; download=6174315083; total=1073741824000; expire=1671815872").unwrap();
        assert_eq!(info.upload, 455727941);
        assert_eq!(info.total, 1073741824000);
        assert_eq!(info.expire, Some(1671815872));
        let info = parse_userinfo("upload=0;download=1.5e3;total=0;expire=0").unwrap();
        assert_eq!(info.download, 1500);
        assert_eq!(info.expire, None);
        assert!(parse_userinfo("garbage").is_none());
    }

    #[test]
    fn headers_and_base64_title() {
        let meta = metadata_from_headers([
            ("Profile-Title", "base64:0JzQvtGPINC/0L7QtNC/0LjRgdC60LA="),
            ("profile-update-interval", "6"),
            ("support-url", "https://t.me/support"),
            ("content-disposition", "attachment; filename=\"ignored\""),
        ]);
        assert_eq!(meta.title.as_deref(), Some("Моя подписка"));
        assert_eq!(meta.update_interval_hours, Some(6));
        assert_eq!(meta.support_url.as_deref(), Some("https://t.me/support"));

        let meta = metadata_from_headers([("content-disposition", "attachment; filename*=UTF-8''My%20VPN")]);
        assert_eq!(meta.title.as_deref(), Some("My VPN"));
    }

    #[test]
    fn base64_body() {
        let body = encode_base64("vless://id@a.com:443#A\ntrojan://pw@b.com:443#B\n");
        let d = decode_body(&body);
        assert_eq!(d.servers.len(), 2);
        assert!(d.errors.is_empty());
    }

    #[test]
    fn plain_body_with_metadata_lines() {
        let body = "#profile-title: Team VPN\n#announce: base64:SGVsbG8=\nvless://id@a.com:443#A\n";
        let d = decode_body(body);
        assert_eq!(d.meta.title.as_deref(), Some("Team VPN"));
        assert_eq!(d.meta.announce.as_deref(), Some("Hello"));
        assert_eq!(d.servers.len(), 1);
    }

    #[test]
    fn json_array_body() {
        let body = r#"[
            {"remarks":"🇩🇪 DE","outbounds":[{"protocol":"vless","settings":{"vnext":[{"address":"de.x","port":443,"users":[{"id":"u"}]}]}}]},
            {"remarks":"broken"},
            {"outbounds":[{"protocol":"freedom"}]}
        ]"#;
        let d = decode_body(body);
        assert_eq!(d.servers.len(), 2);
        assert_eq!(d.servers[0].name, "🇩🇪 DE");
        assert_eq!(d.servers[0].country().as_deref(), Some("DE"));
        assert_eq!(d.servers[1].name, "Config 3");
        assert_eq!(d.errors.len(), 1);
    }

    #[test]
    fn header_metadata_wins_over_body() {
        let mut meta = metadata_from_headers([("profile-title", "Header")]);
        meta.merge(Metadata { title: Some("Body".into()), update_interval_hours: Some(3), ..Default::default() });
        assert_eq!(meta.title.as_deref(), Some("Header"));
        assert_eq!(meta.update_interval_hours, Some(3));
    }
}
