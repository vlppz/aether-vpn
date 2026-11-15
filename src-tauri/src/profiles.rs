//! Importing servers and managing subscriptions.

use std::collections::HashSet;
use std::time::Duration;

use aether_core::subscription::{self, decode_body};
use aether_core::Subscription;
use anyhow::{Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::state::{emit_snapshot, notice, AppState};

/// Mirrors `ImportResult` in ui/src/lib/types.ts.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: usize,
    pub duplicates: usize,
    pub subscriptions_added: usize,
    pub errors: Vec<String>,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn is_subscription_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && !s.contains(char::is_whitespace)
}

/// Imports free-form text: share links, base64 blobs, Xray JSON, and
/// http(s) URLs (which become subscriptions).
pub async fn import_text(app: &AppHandle, text: &str) -> Result<ImportResult> {
    let mut result = ImportResult::default();
    let mut urls = Vec::new();
    let mut rest = String::new();
    for line in text.lines() {
        let t = line.trim();
        if is_subscription_url(t) {
            urls.push(t.to_string());
        } else {
            rest.push_str(line);
            rest.push('\n');
        }
    }

    if !rest.trim().is_empty() {
        let decoded = decode_body(&rest);
        result.errors.extend(decoded.errors);
        let state = app.state::<AppState>();
        let mut store = state.store.write().unwrap();
        let mut known: HashSet<String> = store.profiles.servers.iter().map(|s| s.fingerprint()).collect();
        let mut first_new = None;
        for server in decoded.servers {
            if known.insert(server.fingerprint()) {
                first_new.get_or_insert_with(|| server.id.clone());
                store.profiles.servers.push(server);
                result.added += 1;
            } else {
                result.duplicates += 1;
            }
        }
        if store.selected().is_none() {
            store.profiles.selected_id = first_new;
        }
        store.save_profiles()?;
    }

    for url in urls {
        match add_subscription(app, &url, None).await {
            Ok(_) => result.subscriptions_added += 1,
            Err(e) => result.errors.push(format!("{url}: {e:#}")),
        }
    }
    emit_snapshot(app);
    Ok(result)
}

pub async fn add_subscription(app: &AppHandle, url: &str, name: Option<String>) -> Result<Subscription> {
    let url = url.trim();
    url::Url::parse(url).context("invalid subscription URL")?;
    let state = app.state::<AppState>();
    let existing = {
        let store = state.store.read().unwrap();
        store.profiles.subscriptions.iter().find(|s| s.url == url).map(|s| s.id.clone())
    };
    if let Some(id) = existing {
        return refresh_subscription(app, &id).await;
    }
    let mut sub = Subscription::new(url);
    if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
        sub.name = name.trim().to_string();
        sub.name_locked = true;
    }
    let id = sub.id.clone();
    let user_agent = state.store.read().unwrap().settings.user_agent.clone();
    let fetched = fetch(app, url, user_agent.as_deref()).await?;
    {
        let mut store = state.store.write().unwrap();
        store.profiles.subscriptions.push(sub);
        apply_fetch(&mut store, &id, fetched);
        store.save_profiles()?;
    }
    emit_snapshot(app);
    let store = state.store.read().unwrap();
    Ok(store.profiles.subscriptions.iter().find(|s| s.id == id).cloned().unwrap())
}

async fn fetch(app: &AppHandle, url: &str, user_agent: Option<&str>) -> Result<subscription::Decoded> {
    // Fetch through our own tunnel when it is up: many subscription hosts
    // are blocked exactly where people need a VPN.
    let proxy = app
        .state::<AppState>()
        .engine
        .socks_port()
        .map(|p| format!("socks5h://127.0.0.1:{p}"));
    Ok(subscription::fetch(url, user_agent, proxy.as_deref()).await?)
}

fn apply_fetch(store: &mut crate::store::Store, id: &str, fetched: subscription::Decoded) {
    let meta = fetched.meta;
    if let Some(sub) = store.profiles.subscriptions.iter_mut().find(|s| s.id == id) {
        if let Some(title) = meta.title.filter(|_| !sub.name_locked) {
            sub.name = title;
        }
        sub.info = meta.info;
        sub.announce = meta.announce;
        sub.support_url = meta.support_url;
        sub.web_page_url = meta.web_page_url;
        if let Some(h) = meta.update_interval_hours {
            sub.update_interval_hours = h;
        }
        sub.last_updated = Some(now_secs());
        sub.last_error = None;
    }
    store.replace_subscription_servers(id, fetched.servers);
}

pub async fn refresh_subscription(app: &AppHandle, id: &str) -> Result<Subscription> {
    let state = app.state::<AppState>();
    let (url, ua) = {
        let store = state.store.read().unwrap();
        let sub = store.profiles.subscriptions.iter().find(|s| s.id == id).context("subscription not found")?;
        (sub.url.clone(), sub.user_agent.clone().or_else(|| store.settings.user_agent.clone()))
    };
    let result = fetch(app, &url, ua.as_deref()).await;
    let out = {
        let mut store = state.store.write().unwrap();
        let out = match result {
            Ok(fetched) => {
                let count = fetched.servers.len();
                apply_fetch(&mut store, id, fetched);
                state.logs.app(format!("Subscription updated: {count} servers"));
                Ok(store.profiles.subscriptions.iter().find(|s| s.id == id).cloned().unwrap())
            }
            Err(e) => {
                if let Some(sub) = store.profiles.subscriptions.iter_mut().find(|s| s.id == id) {
                    sub.last_error = Some(format!("{e:#}"));
                }
                Err(e)
            }
        };
        store.save_profiles()?;
        out
    };
    emit_snapshot(app);
    out
}

pub async fn refresh_all(app: &AppHandle, only_due: bool) -> usize {
    let ids: Vec<String> = {
        let state = app.state::<AppState>();
        let store = state.store.read().unwrap();
        let now = now_secs();
        store
            .profiles
            .subscriptions
            .iter()
            .filter(|s| {
                !only_due
                    || (s.auto_update
                        && s.last_updated.is_none_or(|t| now - t >= s.update_interval_hours.max(1) as i64 * 3600))
            })
            .map(|s| s.id.clone())
            .collect()
    };
    let mut failed = 0;
    for id in ids {
        if let Err(e) = refresh_subscription(app, &id).await {
            failed += 1;
            app.state::<AppState>().logs.push("warning", format!("Subscription refresh failed: {e:#}"));
        }
    }
    failed
}

/// Background loop refreshing subscriptions whose interval elapsed.
pub fn spawn_auto_update(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        loop {
            let failed = refresh_all(&app, true).await;
            if failed > 0 {
                notice(&app, "error", format!("{failed} subscription(s) failed to update"));
            }
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
        }
    });
}

/// Handles `aether://import?url=…`, `aether://add/<link>` and raw share links.
pub fn deep_link_payload(link: &str) -> Option<String> {
    let rest = link.strip_prefix("aether://")?;
    if let Ok(u) = url::Url::parse(link) {
        if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "url" || k == "link") {
            return Some(v.into_owned());
        }
    }
    let rest = rest
        .strip_prefix("import/")
        .or_else(|| rest.strip_prefix("add/"))
        .unwrap_or(rest);
    let decoded = percent_decode(rest);
    (decoded.contains("://")).then_some(decoded)
}

fn percent_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={}", s.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_links() {
        assert_eq!(
            deep_link_payload("aether://import?url=https%3A%2F%2Fsub.example.com%2Fabc").as_deref(),
            Some("https://sub.example.com/abc")
        );
        assert_eq!(
            deep_link_payload("aether://add/vless%3A%2F%2Fid%40a.com%3A443%23A").as_deref(),
            Some("vless://id@a.com:443#A")
        );
        assert_eq!(deep_link_payload("aether://add/garbage"), None);
        assert_eq!(deep_link_payload("https://x"), None);
    }

    #[test]
    fn subscription_url_detection() {
        assert!(is_subscription_url("https://sub.example.com/abc?token=1"));
        assert!(!is_subscription_url("vless://id@a.com:443"));
        assert!(!is_subscription_url("https://a.com and text"));
    }
}
