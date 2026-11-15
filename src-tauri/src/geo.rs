//! Exit-IP lookup through the active tunnel, for the Home screen.

use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct ExitInfo {
    pub ip: String,
    pub country: Option<String>,
    pub city: Option<String>,
}

#[derive(Deserialize)]
struct IpWhoIs {
    success: Option<bool>,
    ip: Option<String>,
    country_code: Option<String>,
    city: Option<String>,
}

#[derive(Deserialize)]
struct IpInfo {
    ip: Option<String>,
    country: Option<String>,
    city: Option<String>,
}

pub async fn lookup(socks_port: u16) -> Option<ExitInfo> {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks_port}")).ok()?)
        .timeout(Duration::from_secs(8))
        .build()
        .ok()?;
    if let Ok(r) = client.get("https://ipwho.is/").send().await {
        if let Ok(v) = r.json::<IpWhoIs>().await {
            if v.success != Some(false) {
                if let Some(ip) = v.ip {
                    return Some(ExitInfo { ip, country: v.country_code, city: v.city });
                }
            }
        }
    }
    let v = client.get("https://ipinfo.io/json").send().await.ok()?.json::<IpInfo>().await.ok()?;
    Some(ExitInfo { ip: v.ip?, country: v.country, city: v.city })
}
