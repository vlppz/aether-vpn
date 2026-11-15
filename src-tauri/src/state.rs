//! Shared application state and the snapshot sent to the UI.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock, RwLock};

use aether_core::Subscription;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::{ConnectionStatus, Engine};
use crate::logs::Logs;
use crate::paths::Paths;
use crate::settings::Settings;
use crate::store::Store;

pub struct AppState {
    pub paths: Paths,
    pub store: RwLock<Store>,
    pub logs: Arc<Logs>,
    pub engine: Engine,
    pub pinging: AtomicBool,
    /// Set during exit: the main thread is blocked, so nothing may dispatch to it.
    pub shutting_down: AtomicBool,
    pub core_version: OnceLock<String>,
}

/// Mirrors `ServerView` in ui/src/lib/types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    pub protocol: &'static str,
    pub transport: &'static str,
    pub security: &'static str,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub country: Option<String>,
    pub latency: Option<i64>,
}

/// Mirrors `Snapshot` in ui/src/lib/types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub servers: Vec<ServerView>,
    pub subscriptions: Vec<Subscription>,
    pub selected_id: Option<String>,
    pub status: ConnectionStatus,
    pub settings: Settings,
    pub platform: &'static str,
    pub app_version: String,
    pub core_version: Option<String>,
}

pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

pub fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let store = state.store.read().unwrap();
    let servers = store
        .profiles
        .servers
        .iter()
        .map(|s| {
            let endpoint = s.outbound.endpoint();
            ServerView {
                id: s.id.clone(),
                name: s.name.clone(),
                subscription_id: s.subscription_id.clone(),
                protocol: s.outbound.protocol_name(),
                transport: s.outbound.transport_name(),
                security: s.outbound.security_name(),
                address: endpoint.as_ref().map(|e| e.0.clone()),
                port: endpoint.map(|e| e.1),
                country: s.country(),
                latency: store.profiles.latencies.get(&s.id).copied(),
            }
        })
        .collect();
    Snapshot {
        servers,
        subscriptions: store.profiles.subscriptions.clone(),
        selected_id: store.profiles.selected_id.clone(),
        status: state.engine.status(),
        settings: store.settings.clone(),
        platform: platform(),
        app_version: app.package_info().version.to_string(),
        core_version: state.core_version.get().cloned(),
    }
}

pub fn emit_snapshot(app: &AppHandle) {
    let _ = app.emit("snapshot", snapshot(app));
    crate::tray::refresh(app);
}

#[derive(Debug, Clone, Serialize)]
pub struct Notice {
    pub kind: &'static str,
    pub message: String,
}

pub fn notice(app: &AppHandle, kind: &'static str, message: impl Into<String>) {
    let _ = app.emit("notice", Notice { kind, message: message.into() });
}
