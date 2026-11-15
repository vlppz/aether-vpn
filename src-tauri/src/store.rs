//! Persistent state: servers, subscriptions, selection and settings.

use std::collections::HashMap;
use std::path::PathBuf;

use aether_core::{Server, Subscription};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::paths::write_atomic;
use crate::settings::Settings;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profiles {
    pub servers: Vec<Server>,
    pub subscriptions: Vec<Subscription>,
    pub selected_id: Option<String>,
    /// Last measured latency per server id (ms, -1 = failed).
    pub latencies: HashMap<String, i64>,
    /// Set while Aether owns the OS proxy settings, so a crash can be
    /// repaired on the next start.
    pub proxy_dirty: bool,
    /// OS proxy settings from before we changed them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_backup: Option<serde_json::Value>,
    /// Port the OS proxy was pointed at (may differ from settings if it was busy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_port: Option<u16>,
}

pub struct Store {
    dir: PathBuf,
    pub profiles: Profiles,
    pub settings: Settings,
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(path: &std::path::Path) -> T {
    match std::fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_else(|e| {
            tracing::warn!("{} is corrupt ({e}); keeping a backup and starting fresh", path.display());
            let _ = std::fs::copy(path, path.with_extension("json.bak"));
            T::default()
        }),
        Err(_) => T::default(),
    }
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let profiles = load_json(&dir.join("profiles.json"));
        let settings = load_json(&dir.join("settings.json"));
        Self { dir, profiles, settings }
    }

    pub fn save_profiles(&self) -> Result<()> {
        write_atomic(&self.dir.join("profiles.json"), &serde_json::to_vec_pretty(&self.profiles)?)
    }

    pub fn save_settings(&self) -> Result<()> {
        write_atomic(&self.dir.join("settings.json"), &serde_json::to_vec_pretty(&self.settings)?)
    }

    pub fn server(&self, id: &str) -> Option<&Server> {
        self.profiles.servers.iter().find(|s| s.id == id)
    }

    pub fn selected(&self) -> Option<&Server> {
        self.profiles.selected_id.as_deref().and_then(|id| self.server(id))
    }

    /// Ensures something sensible is selected after servers changed.
    pub fn fix_selection(&mut self) {
        let valid = self.selected().is_some();
        if !valid {
            self.profiles.selected_id = self.profiles.servers.first().map(|s| s.id.clone());
        }
    }

    /// Replaces the servers of a subscription, keeping ids of servers that
    /// did not change so selection and latency survive a refresh.
    pub fn replace_subscription_servers(&mut self, sub_id: &str, fresh: Vec<Server>) {
        let old: HashMap<String, String> = self
            .profiles
            .servers
            .iter()
            .filter(|s| s.subscription_id.as_deref() == Some(sub_id))
            .map(|s| (s.fingerprint(), s.id.clone()))
            .collect();
        let insert_at = self
            .profiles
            .servers
            .iter()
            .position(|s| s.subscription_id.as_deref() == Some(sub_id))
            .unwrap_or(self.profiles.servers.len());
        self.profiles.servers.retain(|s| s.subscription_id.as_deref() != Some(sub_id));
        let fresh: Vec<Server> = fresh
            .into_iter()
            .map(|mut s| {
                if let Some(id) = old.get(&s.fingerprint()) {
                    s.id = id.clone();
                }
                s.subscription_id = Some(sub_id.to_string());
                s
            })
            .collect();
        let at = insert_at.min(self.profiles.servers.len());
        self.profiles.servers.splice(at..at, fresh);
        let ids: std::collections::HashSet<&String> = self.profiles.servers.iter().map(|s| &s.id).collect();
        self.profiles.latencies.retain(|id, _| ids.contains(id));
        self.fix_selection();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::parse_link;

    #[test]
    fn refresh_keeps_ids_of_unchanged_servers() {
        let dir = std::env::temp_dir().join(format!("aether-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut store = Store::load(dir.clone());

        let a = parse_link("vless://id@a.com:443#A").unwrap();
        let b = parse_link("vless://id@b.com:443#B").unwrap();
        store.replace_subscription_servers("sub", vec![a.clone(), b.clone()]);
        let b_id = store.profiles.servers[1].id.clone();
        store.profiles.selected_id = Some(b_id.clone());

        let c = parse_link("vless://id@c.com:443#C").unwrap();
        let b2 = parse_link("vless://id@b.com:443#B renamed").unwrap();
        store.replace_subscription_servers("sub", vec![b2, c]);
        assert_eq!(store.profiles.servers.len(), 2);
        assert_eq!(store.profiles.servers[0].id, b_id);
        assert_eq!(store.profiles.servers[0].name, "B renamed");
        assert_eq!(store.profiles.selected_id.as_deref(), Some(b_id.as_str()));

        store.save_profiles().unwrap();
        let reloaded = Store::load(dir.clone());
        assert_eq!(reloaded.profiles.servers.len(), 2);
        std::fs::remove_dir_all(dir).ok();
    }
}
