//! `#[tauri::command]` handlers — the API documented in ui/src/lib/types.ts.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use aether_core::{to_link, Subscription, TunnelMode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::core;
use crate::latency;
use crate::logs::LogLine;
use crate::profiles::{self, ImportResult};
use crate::settings::Settings;
use crate::state::{emit_snapshot, snapshot, AppState, Snapshot};

/// Errors cross the IPC boundary as plain strings.
#[derive(Debug)]
pub struct CmdError(String);

impl Serialize for CmdError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<E: Into<anyhow::Error>> From<E> for CmdError {
    fn from(e: E) -> Self {
        CmdError(format!("{:#}", e.into()))
    }
}

type CmdResult<T = ()> = Result<T, CmdError>;

fn err(msg: impl Into<String>) -> CmdError {
    CmdError(msg.into())
}

#[tauri::command]
pub fn get_snapshot(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

#[tauri::command]
pub async fn import_text(app: AppHandle, text: String) -> CmdResult<ImportResult> {
    Ok(profiles::import_text(&app, &text).await?)
}

#[tauri::command]
pub async fn import_clipboard(app: AppHandle) -> CmdResult<ImportResult> {
    let text = app.clipboard().read_text().map_err(|_| err("Clipboard is empty or not text"))?;
    if text.trim().is_empty() {
        return Err(err("Clipboard is empty"));
    }
    Ok(profiles::import_text(&app, &text).await?)
}

#[tauri::command]
pub async fn add_subscription(app: AppHandle, url: String, name: Option<String>) -> CmdResult<Subscription> {
    Ok(profiles::add_subscription(&app, &url, name).await?)
}

#[tauri::command]
pub async fn refresh_subscription(app: AppHandle, id: String) -> CmdResult<Subscription> {
    Ok(profiles::refresh_subscription(&app, &id).await?)
}

#[tauri::command]
pub async fn refresh_all_subscriptions(app: AppHandle) -> CmdResult {
    match profiles::refresh_all(&app, false).await {
        0 => Ok(()),
        n => Err(err(format!("{n} subscription(s) failed to update"))),
    }
}

#[tauri::command]
pub fn update_subscription(app: AppHandle, state: State<AppState>, subscription: Subscription) -> CmdResult {
    {
        let mut store = state.store.write().unwrap();
        let sub = store
            .profiles
            .subscriptions
            .iter_mut()
            .find(|s| s.id == subscription.id)
            .ok_or_else(|| err("subscription not found"))?;
        let renamed = sub.name != subscription.name;
        sub.name_locked |= renamed;
        sub.name = subscription.name;
        sub.url = subscription.url;
        sub.user_agent = subscription.user_agent.filter(|u| !u.trim().is_empty());
        sub.auto_update = subscription.auto_update;
        sub.update_interval_hours = subscription.update_interval_hours.max(1);
        store.save_profiles()?;
    }
    emit_snapshot(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_subscription(app: AppHandle, state: State<AppState>, id: String) -> CmdResult {
    {
        let mut store = state.store.write().unwrap();
        store.profiles.subscriptions.retain(|s| s.id != id);
        store.profiles.servers.retain(|s| s.subscription_id.as_deref() != Some(id.as_str()));
        store.fix_selection();
        store.save_profiles()?;
    }
    emit_snapshot(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_server(app: AppHandle, state: State<AppState>, id: String) -> CmdResult {
    {
        let mut store = state.store.write().unwrap();
        store.profiles.servers.retain(|s| s.id != id);
        store.profiles.latencies.remove(&id);
        store.fix_selection();
        store.save_profiles()?;
    }
    emit_snapshot(&app);
    Ok(())
}

#[tauri::command]
pub fn rename_server(app: AppHandle, state: State<AppState>, id: String, name: String) -> CmdResult {
    let name = name.trim();
    if name.is_empty() {
        return Err(err("Name must not be empty"));
    }
    {
        let mut store = state.store.write().unwrap();
        let server = store.profiles.servers.iter_mut().find(|s| s.id == id).ok_or_else(|| err("server not found"))?;
        server.name = name.to_string();
        store.save_profiles()?;
    }
    emit_snapshot(&app);
    Ok(())
}

pub async fn select(app: &AppHandle, id: &str) -> CmdResult {
    let state = app.state::<AppState>();
    let changed = {
        let mut store = state.store.write().unwrap();
        if store.server(id).is_none() {
            return Err(err("server not found"));
        }
        let changed = store.profiles.selected_id.as_deref() != Some(id);
        store.profiles.selected_id = Some(id.to_string());
        store.save_profiles()?;
        changed
    };
    emit_snapshot(app);
    if changed {
        core::reconnect_if_active(app).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn select_server(app: AppHandle, id: String) -> CmdResult {
    select(&app, &id).await
}

#[tauri::command]
pub async fn connect(app: AppHandle) -> CmdResult {
    Ok(core::connect(&app).await?)
}

#[tauri::command]
pub async fn disconnect(app: AppHandle) -> CmdResult {
    Ok(core::disconnect(&app).await?)
}

#[tauri::command]
pub async fn toggle_connection(app: AppHandle) -> CmdResult {
    Ok(core::toggle(&app).await?)
}

pub async fn apply_settings(app: &AppHandle, settings: Settings) -> CmdResult {
    let state = app.state::<AppState>();
    let old = {
        let mut store = state.store.write().unwrap();
        let old = std::mem::replace(&mut store.settings, settings.clone());
        store.save_settings()?;
        old
    };
    if old.launch_at_login != settings.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let r = if settings.launch_at_login { app.autolaunch().enable() } else { app.autolaunch().disable() };
        if let Err(e) = r {
            state.logs.push("warning", format!("Launch at login: {e}"));
        }
    }
    let status = state.engine.status();
    let _ = app.emit("status", &status);
    emit_snapshot(app);
    let needs_restart = old.core != settings.core || old.set_system_proxy != settings.set_system_proxy;
    if needs_restart {
        core::reconnect_if_active(app).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult {
    apply_settings(&app, settings).await
}

#[tauri::command]
pub async fn set_mode(app: AppHandle, mode: TunnelMode) -> CmdResult {
    let mut settings = app.state::<AppState>().store.read().unwrap().settings.clone();
    if settings.core.mode == mode {
        return Ok(());
    }
    settings.core.mode = mode;
    // reflect the mode in the status even while disconnected
    {
        let state = app.state::<AppState>();
        if !state.engine.is_active() {
            let mut status = state.engine.status();
            status.mode = mode;
            let _ = app.emit("status", &status);
        }
    }
    apply_settings(&app, settings).await
}

fn record_latency(app: &AppHandle, id: &str, ms: i64) {
    #[derive(Serialize, Clone)]
    struct LatencyUpdate<'a> {
        id: &'a str,
        latency: i64,
    }
    let state = app.state::<AppState>();
    state.store.write().unwrap().profiles.latencies.insert(id.to_string(), ms);
    let _ = app.emit("latency", LatencyUpdate { id, latency: ms });
}

pub(crate) async fn run_latency(app: &AppHandle, servers: Vec<aether_core::Server>) {
    let state = app.state::<AppState>();
    let reporter_app = app.clone();
    let report: latency::Reporter = Arc::new(move |id: &str, ms: i64| record_latency(&reporter_app, id, ms));
    latency::test_servers(
        state.paths.sidecar("xray"),
        state.paths.assets.clone(),
        state.paths.run.clone(),
        servers,
        report,
    )
    .await;
    let _ = state.store.read().unwrap().save_profiles();
}

#[tauri::command]
pub async fn ping_all(app: AppHandle) -> CmdResult {
    let state = app.state::<AppState>();
    if state.pinging.swap(true, Ordering::SeqCst) {
        return Ok(()); // already running
    }
    let servers = state.store.read().unwrap().profiles.servers.clone();
    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_latency(&task_app, servers).await;
        task_app.state::<AppState>().pinging.store(false, Ordering::SeqCst);
        emit_snapshot(&task_app);
    });
    Ok(())
}

#[tauri::command]
pub async fn ping_server(app: AppHandle, id: String) -> CmdResult<i64> {
    let server = {
        let state = app.state::<AppState>();
        let store = state.store.read().unwrap();
        store.server(&id).cloned().ok_or_else(|| err("server not found"))?
    };
    run_latency(&app, vec![server]).await;
    let ms = app.state::<AppState>().store.read().unwrap().profiles.latencies.get(&id).copied().unwrap_or(-1);
    Ok(ms)
}

#[tauri::command]
pub async fn test_real_delay(app: AppHandle) -> CmdResult<u64> {
    let port = app
        .state::<AppState>()
        .engine
        .socks_port()
        .ok_or_else(|| err("Not connected"))?;
    Ok(latency::measure(port).await?)
}

#[tauri::command]
pub async fn select_fastest(app: AppHandle) -> CmdResult<String> {
    let id = {
        let state = app.state::<AppState>();
        let store = state.store.read().unwrap();
        store
            .profiles
            .servers
            .iter()
            .filter_map(|s| store.profiles.latencies.get(&s.id).filter(|ms| **ms > 0).map(|ms| (ms, &s.id)))
            .min_by_key(|(ms, _)| **ms)
            .map(|(_, id)| id.clone())
            .ok_or_else(|| err("No latency results yet — run “Ping all” first"))?
    };
    select(&app, &id).await?;
    Ok(id)
}

#[tauri::command]
pub fn get_logs(state: State<AppState>) -> Vec<LogLine> {
    state.logs.all()
}

#[tauri::command]
pub fn clear_logs(state: State<AppState>) {
    state.logs.clear();
}

#[tauri::command]
pub fn export_link(state: State<AppState>, id: String) -> CmdResult<String> {
    let store = state.store.read().unwrap();
    let server = store.server(&id).ok_or_else(|| err("server not found"))?;
    Ok(to_link(server)?)
}

#[tauri::command]
pub fn server_qr(state: State<AppState>, id: String) -> CmdResult<String> {
    use qrcode::render::svg;
    let link = export_link(state, id)?;
    let code = qrcode::QrCode::with_error_correction_level(link.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| err(format!("Link is too long for a QR code: {e}")))?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(280, 280)
        .quiet_zone(true)
        .dark_color(svg::Color("#0b0b12"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[tauri::command]
pub async fn uninstall_vpn_service(app: AppHandle) -> CmdResult {
    #[cfg(target_os = "macos")]
    {
        let state = app.state::<AppState>();
        if state.engine.is_active() && state.engine.status().mode == TunnelMode::Tun {
            core::disconnect(&app).await?;
        }
        core::tun::uninstall_service(&state.paths).await?;
        state.logs.app("VPN service removed");
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(err("There is no background service on this platform"))
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
