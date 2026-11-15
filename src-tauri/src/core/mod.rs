//! Connection engine: owns the running Xray (local or via the TUN helper),
//! the system proxy, the stats poller, and the connection state machine
//! `Disconnected → Connecting → Connected → Disconnecting`.

pub mod process;
pub mod stats;
pub mod tun;

use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use aether_core::{build_xray_config, CoreSettings, Server, TunnelMode};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::task::JoinHandle;

use self::process::XrayProcess;
use self::tun::TunSession;
use crate::logs::now_ms;
use crate::paths::write_atomic;
use crate::settings::Settings;
use crate::state::{notice, AppState};
use crate::{geo, sysproxy, tray};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

/// Mirrors `ConnectionStatus` in ui/src/lib/types.ts.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub state: ConnState,
    pub mode: TunnelMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_city: Option<String>,
}

enum Runner {
    Local(XrayProcess),
    Tun(TunSession),
}

impl Runner {
    fn is_running(&self) -> bool {
        match self {
            Runner::Local(p) => p.is_running(),
            Runner::Tun(t) => t.is_running(),
        }
    }

    async fn stop(self) {
        match self {
            Runner::Local(p) => p.stop().await,
            Runner::Tun(t) => t.stop().await,
        }
    }
}

#[derive(Default)]
struct Inner {
    runner: Option<Runner>,
    tasks: Vec<JoinHandle<()>>,
    owns_system_proxy: bool,
}

#[derive(Default)]
pub struct Engine {
    /// Serialises connect/disconnect; held across awaits.
    inner: tokio::sync::Mutex<Inner>,
    status: Mutex<ConnectionStatus>,
    generation: AtomicU64,
    socks_port: AtomicU16,
}

impl Engine {
    pub fn status(&self) -> ConnectionStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status().state, ConnState::Connected | ConnState::Connecting)
    }

    /// SOCKS port of the running tunnel, if connected.
    pub fn socks_port(&self) -> Option<u16> {
        (self.status().state == ConnState::Connected).then(|| self.socks_port.load(Ordering::Relaxed))
    }
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut ConnectionStatus)) {
    let state = app.state::<AppState>();
    let status = {
        let mut s = state.engine.status.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("status", &status);
    tray::refresh(app);
}

fn port_free(host: &str, port: u16) -> bool {
    std::net::TcpListener::bind((host, port)).is_ok()
}

/// Makes the SOCKS/HTTP/metrics ports distinct and free. Other VPN clients
/// (Happ, v2rayN…) often sit on the usual ports, so a busy port moves to the
/// next free one instead of failing the connection.
fn pick_ports(core: &mut CoreSettings, warn: impl Fn(String)) -> Result<()> {
    let host = if core.allow_lan { "0.0.0.0" } else { "127.0.0.1" };
    let mut taken = Vec::new();
    for (slot, what) in [
        (&mut core.socks_port, "SOCKS"),
        (&mut core.http_port, "HTTP"),
        (&mut core.metrics_port, "metrics"),
    ] {
        let wanted = *slot;
        let port = (0..200u16)
            .filter_map(|i| wanted.checked_add(i))
            .find(|p| *p >= 1024 && !taken.contains(p) && port_free(host, *p))
            .with_context(|| format!("no free port found for {what} near {wanted}"))?;
        if port != wanted {
            warn(format!("{what} port {wanted} is busy (another VPN app?) — using {port} instead"));
        }
        taken.push(port);
        *slot = port;
    }
    Ok(())
}

pub async fn connect(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let mut inner = state.engine.inner.lock().await;
    if inner.runner.is_some() {
        return Ok(());
    }
    let (server, settings) = {
        let store = state.store.read().unwrap();
        let server = store.selected().cloned().context("No server selected")?;
        (server, store.settings.clone())
    };
    let mode = settings.core.mode;
    set_status(app, |s| {
        *s = ConnectionStatus {
            state: ConnState::Connecting,
            mode,
            server_id: Some(server.id.clone()),
            ..Default::default()
        }
    });
    state.logs.app(format!(
        "Connecting to “{}” ({}, {} mode)",
        server.name,
        server.outbound.protocol_name(),
        if mode == TunnelMode::Tun { "TUN" } else { "proxy" }
    ));

    match start(app, &state, &mut inner, &server, &settings).await {
        Ok(core) => {
            set_status(app, |s| {
                s.state = ConnState::Connected;
                s.since = Some(now_ms());
            });
            state.logs.app("Connected");
            inner.tasks.push(stats::spawn(app.clone(), core.metrics_port));
            inner.tasks.push(spawn_geo(app.clone(), core.socks_port));
            Ok(())
        }
        Err(e) => {
            teardown(app, &state, &mut inner).await;
            let msg = format!("{e:#}");
            state.logs.push("error", format!("Connection failed: {msg}"));
            set_status(app, |s| {
                s.state = ConnState::Error;
                s.error = Some(msg);
            });
            Err(e)
        }
    }
}

async fn start(
    app: &AppHandle,
    state: &AppState,
    inner: &mut Inner,
    server: &Server,
    settings: &Settings,
) -> Result<CoreSettings> {
    let mut core = settings.core.clone();
    pick_ports(&mut core, |msg| state.logs.push("warning", msg))?;
    let core = &core;

    let config = build_xray_config(server, core)?;
    let config_path = state.paths.run.join("config.json");
    write_atomic(&config_path, &serde_json::to_vec_pretty(&config)?)?;

    let generation = state.engine.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let exit_app = app.clone();
    let on_exit: process::OnExit = Box::new(move |code| {
        tauri::async_runtime::spawn(async move { unexpected_exit(&exit_app, generation, code).await });
    });

    let runner = match core.mode {
        TunnelMode::Proxy => Runner::Local(XrayProcess::spawn(
            &state.paths.sidecar("xray"),
            &config_path,
            &state.paths.assets,
            state.logs.clone(),
            on_exit,
        )?),
        TunnelMode::Tun => {
            let name = core.tun.name.clone();
            tokio::task::spawn_blocking(move || tun::check_no_other_vpn(&name)).await??;
            Runner::Tun(TunSession::start(&state.paths, &config_path, core.tun.ipv6, state.logs.clone(), on_exit).await?)
        }
    };
    inner.runner = Some(runner);
    state.engine.socks_port.store(core.socks_port, Ordering::Relaxed);

    // Ready once the SOCKS inbound accepts connections.
    let mut ready = false;
    for _ in 0..80 {
        if !inner.runner.as_ref().is_some_and(Runner::is_running) {
            break;
        }
        if tokio::net::TcpStream::connect(("127.0.0.1", core.socks_port)).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !ready {
        tokio::time::sleep(Duration::from_millis(150)).await; // let stderr drain
        let errors = state.logs.recent_errors(3);
        if errors.is_empty() {
            bail!("Xray did not start");
        }
        bail!("Xray did not start: {}", errors.join(" · "));
    }

    if core.mode == TunnelMode::Tun {
        // Routes appear a moment after the interface; then make sure they won.
        let name = core.tun.name.clone();
        let mut via = None;
        for _ in 0..20 {
            let n = name.clone();
            via = tokio::task::spawn_blocking(tun::route_interface).await?;
            if via.as_deref() == Some(n.as_str()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        match via {
            Some(v) if v == name => state.logs.app(format!("System traffic is routed through {name}")),
            Some(v) => bail!(
                "the tunnel is up, but system traffic still goes through {v}{}",
                if tun::is_vpn_interface(&v) { " — another VPN has priority; disconnect it first" } else { "" }
            ),
            None => {}
        }
    }

    if core.mode == TunnelMode::Proxy && settings.set_system_proxy {
        {
            let mut store = state.store.write().unwrap();
            store.profiles.proxy_dirty = true;
            let _ = store.save_profiles();
        }
        let port = core.socks_port;
        match tokio::task::spawn_blocking(move || sysproxy::enable(port)).await? {
            Ok(backup) => {
                {
                    let mut store = state.store.write().unwrap();
                    store.profiles.proxy_backup = Some(backup);
                    store.profiles.proxy_port = Some(port);
                    let _ = store.save_profiles();
                }
                inner.owns_system_proxy = true;
                state.logs.app(format!("System proxy set to 127.0.0.1:{port}"));
            }
            Err(e) => {
                state.logs.push("warning", format!("Could not set system proxy: {e:#}"));
                notice(app, "error", format!("Could not set the system proxy: {e}"));
            }
        }
    }
    Ok(core.clone())
}

fn spawn_geo(app: AppHandle, socks_port: u16) -> JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Some(info) = geo::lookup(socks_port).await {
            let state = app.state::<AppState>();
            if state.engine.status().state == ConnState::Connected {
                state.logs.app(format!("Exit IP {}", info.ip));
                set_status(&app, |s| {
                    s.exit_ip = Some(info.ip);
                    s.exit_country = info.country;
                    s.exit_city = info.city;
                });
            }
        }
    })
}

/// Stops everything that `start` may have set up. Safe to call repeatedly.
async fn teardown(app: &AppHandle, state: &AppState, inner: &mut Inner) {
    for t in inner.tasks.drain(..) {
        t.abort();
    }
    if inner.owns_system_proxy {
        inner.owns_system_proxy = false;
        let backup = state.store.read().unwrap().profiles.proxy_backup.clone();
        if let Ok(Err(e)) = tokio::task::spawn_blocking(move || sysproxy::restore(backup.as_ref())).await {
            state.logs.push("warning", format!("Could not restore system proxy: {e:#}"));
        }
    }
    {
        let mut store = state.store.write().unwrap();
        if store.profiles.proxy_dirty || store.profiles.proxy_backup.is_some() {
            store.profiles.proxy_dirty = false;
            store.profiles.proxy_backup = None;
            store.profiles.proxy_port = None;
            let _ = store.save_profiles();
        }
    }
    if let Some(r) = inner.runner.take() {
        r.stop().await;
    }
    let _ = app.emit("traffic", stats::TrafficTick::default());
}

pub async fn disconnect(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let mut inner = state.engine.inner.lock().await;
    let mode = state.engine.status().mode;
    if inner.runner.is_some() {
        set_status(app, |s| s.state = ConnState::Disconnecting);
        state.logs.app("Disconnecting");
    }
    teardown(app, &state, &mut inner).await;
    set_status(app, |s| {
        *s = ConnectionStatus { state: ConnState::Disconnected, mode, ..Default::default() }
    });
    Ok(())
}

pub async fn toggle(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>().engine.status().state;
    match state {
        ConnState::Connected | ConnState::Connecting => disconnect(app).await,
        _ => connect(app).await,
    }
}

/// Re-applies the current selection/settings if a tunnel is up.
pub async fn reconnect_if_active(app: &AppHandle) -> Result<()> {
    if app.state::<AppState>().engine.is_active() {
        disconnect(app).await?;
        connect(app).await?;
    }
    Ok(())
}

async fn unexpected_exit(app: &AppHandle, generation: u64, code: Option<i32>) {
    let state = app.state::<AppState>();
    let mut inner = state.engine.inner.lock().await;
    if state.engine.generation.load(Ordering::SeqCst) != generation || inner.runner.is_none() {
        return; // a newer connection exists or we already stopped
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    let errors = state.logs.recent_errors(2);
    teardown(app, &state, &mut inner).await;
    let mut msg = format!("Xray stopped unexpectedly (exit code {})", code.map_or("?".into(), |c| c.to_string()));
    if !errors.is_empty() {
        msg = format!("{msg}: {}", errors.join(" · "));
    }
    state.logs.push("error", msg.clone());
    notice(app, "error", msg.clone());
    set_status(app, |s| {
        s.state = ConnState::Error;
        s.error = Some(msg);
        s.since = None;
    });
}

/// Called on app exit: restore the OS to how we found it.
pub fn shutdown_blocking(app: &AppHandle) {
    app.state::<AppState>().shutting_down.store(true, Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::block_on(async move {
        let _ = tokio::time::timeout(Duration::from_secs(10), disconnect(&app)).await;
    });
}

/// Repairs the system proxy if a previous run crashed while owning it.
pub fn repair_after_crash(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (dirty, port, backup) = {
        let store = state.store.read().unwrap();
        (
            store.profiles.proxy_dirty,
            store.profiles.proxy_port.unwrap_or(store.settings.core.socks_port),
            store.profiles.proxy_backup.clone(),
        )
    };
    if dirty {
        if sysproxy::points_to(port) {
            let _ = sysproxy::restore(backup.as_ref());
            state.logs.app("Restored system proxy left over from an unclean exit");
        }
        let mut store = state.store.write().unwrap();
        store.profiles.proxy_dirty = false;
        store.profiles.proxy_backup = None;
        store.profiles.proxy_port = None;
        let _ = store.save_profiles();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_and_duplicate_ports_move() {
        let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = busy.local_addr().unwrap().port();
        let mut core = CoreSettings { socks_port: port, http_port: port, metrics_port: port, ..Default::default() };
        let warnings = std::sync::Mutex::new(Vec::new());
        pick_ports(&mut core, |w| warnings.lock().unwrap().push(w)).unwrap();
        let ports = [core.socks_port, core.http_port, core.metrics_port];
        assert!(ports.iter().all(|p| *p != port), "busy port reused: {ports:?}");
        assert!(ports[0] != ports[1] && ports[1] != ports[2] && ports[0] != ports[2]);
        assert_eq!(warnings.lock().unwrap().len(), 3);
    }
}
