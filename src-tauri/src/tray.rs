//! System tray: quick connect, mode/routing switches and recent servers.

use aether_core::country::flag;
use aether_core::{RoutingPreset, TunnelMode};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::commands;
use crate::core::{self, ConnState};
use crate::state::AppState;

const TRAY_ID: &str = "main";
const ICON_OFF: &[u8] = include_bytes!("../icons/tray-off.png");
const ICON_ON: &[u8] = include_bytes!("../icons/tray-on.png");
const MAX_SERVERS: usize = 20;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON_OFF)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Aether")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                if !cfg!(target_os = "macos") {
                    show_main_window(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Shows the window; on macOS the Dock icon comes back with it.
pub fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Hides the window to the tray; on macOS Aether also leaves the Dock and
/// ⌘Tab, living in the menu bar only until it is opened again.
pub fn hide_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let status = state.engine.status();
    let (settings, servers, selected) = {
        let store = state.store.read().unwrap();
        let servers: Vec<(String, String)> = store
            .profiles
            .servers
            .iter()
            .take(MAX_SERVERS)
            .map(|s| {
                let label = match s.country() {
                    Some(c) if !s.name.contains(&flag(&c)) => format!("{} {}", flag(&c), s.name),
                    _ => s.name.clone(),
                };
                (s.id.clone(), label)
            })
            .collect();
        (store.settings.clone(), servers, store.profiles.selected_id.clone())
    };

    let status_label = match status.state {
        ConnState::Connected => "● Connected",
        ConnState::Connecting => "◌ Connecting…",
        ConnState::Disconnecting => "◌ Disconnecting…",
        ConnState::Error => "⚠ Connection error",
        ConnState::Disconnected => "○ Disconnected",
    };
    let busy = matches!(status.state, ConnState::Connecting | ConnState::Disconnecting);
    let active = matches!(status.state, ConnState::Connected | ConnState::Connecting);

    let status_item = MenuItem::with_id(app, "status", status_label, false, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        if active { "Disconnect" } else { "Connect" },
        !busy && selected.is_some(),
        None::<&str>,
    )?;

    let mode = settings.core.mode;
    let mode_menu = Submenu::with_items(
        app,
        "Mode",
        true,
        &[
            &CheckMenuItem::with_id(app, "mode:proxy", "System proxy", true, mode == TunnelMode::Proxy, None::<&str>)?,
            &CheckMenuItem::with_id(app, "mode:tun", "TUN — all traffic", true, mode == TunnelMode::Tun, None::<&str>)?,
        ],
    )?;
    let preset = settings.core.routing.preset;
    let routing_menu = Submenu::with_items(
        app,
        "Routing",
        true,
        &[
            &CheckMenuItem::with_id(app, "route:global", "Global", true, preset == RoutingPreset::Global, None::<&str>)?,
            &CheckMenuItem::with_id(app, "route:smart", "Smart", true, preset == RoutingPreset::Smart, None::<&str>)?,
            &CheckMenuItem::with_id(app, "route:direct", "Direct", true, preset == RoutingPreset::Direct, None::<&str>)?,
        ],
    )?;

    let server_items: Vec<CheckMenuItem<Wry>> = servers
        .iter()
        .map(|(id, label)| {
            CheckMenuItem::with_id(app, format!("server:{id}"), label, true, selected.as_deref() == Some(id), None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;
    let server_refs: Vec<&dyn IsMenuItem<Wry>> = server_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let servers_menu = Submenu::with_items(app, "Servers", !server_refs.is_empty(), &server_refs)?;

    Menu::with_items(
        app,
        &[
            &status_item,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &servers_menu,
            &mode_menu,
            &routing_menu,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "show", "Open Aether", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit Aether", true, None::<&str>)?,
        ],
    )
}

/// Rebuilds the menu and icon to reflect the current state.
pub fn refresh(app: &AppHandle) {
    if app.state::<AppState>().shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    let state = app.state::<AppState>();
    let connected = state.engine.status().state == ConnState::Connected;
    if let Ok(icon) = Image::from_bytes(if connected { ICON_ON } else { ICON_OFF }) {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
    }
    let _ = tray.set_tooltip(Some(if connected { "Aether — connected" } else { "Aether" }));
}

fn on_menu(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let result: Result<(), String> = async {
            match id.as_str() {
                "toggle" => core::toggle(&app).await.map_err(|e| format!("{e:#}")),
                "show" => {
                    show_main_window(&app);
                    Ok(())
                }
                "quit" => {
                    app.exit(0);
                    Ok(())
                }
                other => {
                    if let Some(mode) = other.strip_prefix("mode:") {
                        let mode = if mode == "tun" { TunnelMode::Tun } else { TunnelMode::Proxy };
                        commands::set_mode(app.clone(), mode).await.map_err(|e| format!("{e:?}"))
                    } else if let Some(preset) = other.strip_prefix("route:") {
                        let mut settings = app.state::<AppState>().store.read().unwrap().settings.clone();
                        settings.core.routing.preset = match preset {
                            "global" => RoutingPreset::Global,
                            "direct" => RoutingPreset::Direct,
                            _ => RoutingPreset::Smart,
                        };
                        commands::apply_settings(&app, settings).await.map_err(|e| format!("{e:?}"))
                    } else if let Some(server) = other.strip_prefix("server:") {
                        commands::select(&app, server).await.map_err(|e| format!("{e:?}"))
                    } else {
                        Ok(())
                    }
                }
            }
        }
        .await;
        if let Err(e) = result {
            crate::state::notice(&app, "error", e);
        }
        refresh(&app);
    });
}
