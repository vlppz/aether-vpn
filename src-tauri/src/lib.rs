//! Aether desktop app: Tauri shell around `aether-core` and the Xray sidecar.

mod commands;
mod core;
mod geo;
mod latency;
mod logs;
mod paths;
mod profiles;
mod settings;
mod state;
mod store;
mod sysproxy;
mod tray;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock, RwLock};

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;

use crate::logs::Logs;
use crate::paths::Paths;
use crate::state::{emit_snapshot, notice, AppState};
use crate::store::Store;

fn handle_deep_links(app: &AppHandle, urls: Vec<String>) {
    for link in urls {
        let Some(payload) = profiles::deep_link_payload(&link) else { continue };
        let app = app.clone();
        tray::show_main_window(&app);
        tauri::async_runtime::spawn(async move {
            match profiles::import_text(&app, &payload).await {
                Ok(r) if r.added + r.subscriptions_added > 0 => notice(
                    &app,
                    "success",
                    format!("Imported {} server(s), {} subscription(s)", r.added, r.subscriptions_added),
                ),
                Ok(r) => notice(&app, "info", r.errors.first().cloned().unwrap_or_else(|| "Nothing new to import".into())),
                Err(e) => notice(&app, "error", format!("{e:#}")),
            }
        });
    }
}

fn detect_core_version(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let out = tokio::process::Command::new(state.paths.sidecar("xray")).arg("version").output().await;
        if let Ok(out) = out {
            // "Xray 26.3.27 (Xray, Penetrates Everything.) d2758a0 (go1.26.1 darwin/arm64)"
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(v) = text.split_whitespace().nth(1) {
                let _ = state.core_version.set(v.to_string());
                emit_snapshot(&app);
            }
        }
    });
}

/// Ctrl-C / SIGTERM should still restore the system proxy and stop Xray.
fn spawn_signal_handler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else {
                return;
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        // same path as tray → Quit: RunEvent::Exit runs core::shutdown_blocking
        app.exit(0);
    });
}

/// Debug builds only: `AETHER_E2E_LINK=<share link or subscription URL>`
/// (+ `AETHER_E2E_SERVER=<name part>` for subscriptions) imports, selects and
/// connects on startup so scripts/e2e-local.sh can drive the real app.
#[cfg(debug_assertions)]
fn e2e_hook(app: AppHandle) {
    let Ok(link) = std::env::var("AETHER_E2E_LINK") else { return };
    tauri::async_runtime::spawn(async move {
        let result = async {
            profiles::import_text(&app, &link).await?;
            // a share link, or a subscription URL + AETHER_E2E_SERVER name filter
            let wanted = aether_core::parse_link(&link).ok().map(|s| s.fingerprint());
            let name_filter = std::env::var("AETHER_E2E_SERVER").unwrap_or_default();
            let id = {
                let state = app.state::<AppState>();
                let store = state.store.read().unwrap();
                store
                    .profiles
                    .servers
                    .iter()
                    .find(|s| match &wanted {
                        Some(fp) => s.fingerprint() == *fp,
                        None => s.name.contains(&name_filter),
                    })
                    .map(|s| s.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("imported server not found"))?
            };
            commands::select(&app, &id).await.map_err(|e| anyhow::anyhow!("{e:?}"))?;
            core::connect(&app).await
        }
        .await;
        match result {
            Ok(()) => eprintln!("AETHER_E2E: connected"),
            Err(e) => eprintln!("AETHER_E2E: failed: {e:#}"),
        }
        // Real-delay test of every server through a throw-away Xray.
        let servers = app.state::<AppState>().store.read().unwrap().profiles.servers.clone();
        commands::run_latency(&app, servers).await;
        let state = app.state::<AppState>();
        let store = state.store.read().unwrap();
        for (id, ms) in &store.profiles.latencies {
            eprintln!("AETHER_E2E: latency {id} {ms}");
        }
    });
}

/// macOS app menu where ⌘Q hides to the tray (when "Close to tray" is on)
/// and "Quit Aether" (⌥⌘Q) really quits. The system's own terminate request
/// (logout, shutdown) is left alone so it is never blocked.
#[cfg(target_os = "macos")]
fn install_app_menu(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    let name = app.package_info().name.clone();
    let app_menu = Submenu::with_items(
        app,
        &name,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "menu:close-to-tray", "Close to Tray", true, Some("CmdOrCtrl+Q"))?,
            &MenuItem::with_id(app, "menu:quit", "Quit Aether", true, Some("CmdOrCtrl+Alt+Q"))?,
        ],
    )?;
    // the Edit menu is what makes ⌘C/⌘V/⌘A work inside text fields
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    app.set_menu(Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?)?;
    app.on_menu_event(|app, event| match event.id.as_ref() {
        "menu:close-to-tray" => {
            let to_tray = app.state::<AppState>().store.read().unwrap().settings.close_to_tray;
            if to_tray {
                tray::hide_main_window(app);
            } else {
                app.exit(0);
            }
        }
        "menu:quit" => app.exit(0),
        _ => {}
    });
    Ok(())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let paths = Paths::resolve(&handle)?;
    let store = Store::load(paths.data.clone());
    let logs = Arc::new(Logs::new(handle.clone()));
    logs.app(format!("Aether {} starting", handle.package_info().version));

    let start_hidden = store.settings.start_minimized || std::env::args().any(|a| a == "--minimized");
    let auto_connect = store.settings.auto_connect && store.selected().is_some();

    app.manage(AppState {
        paths,
        store: RwLock::new(store),
        logs,
        engine: Default::default(),
        pinging: AtomicBool::new(false),
        shutting_down: AtomicBool::new(false),
        core_version: OnceLock::new(),
    });

    // macOS keeps native traffic lights over the content (titleBarStyle
    // Overlay); elsewhere the UI draws its own title bar.
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.set_decorations(false);
    }

    core::repair_after_crash(&handle);
    #[cfg(target_os = "macos")]
    install_app_menu(&handle)?;
    tray::create(&handle)?;
    detect_core_version(&handle);
    profiles::spawn_auto_update(handle.clone());

    #[cfg(any(windows, target_os = "linux"))]
    {
        // registers the aether:// scheme for unbundled/dev runs
        let _ = handle.deep_link().register_all();
    }
    let link_app = handle.clone();
    handle.deep_link().on_open_url(move |event| {
        handle_deep_links(&link_app, event.urls().iter().map(|u| u.to_string()).collect());
    });
    if let Ok(Some(urls)) = handle.deep_link().get_current() {
        handle_deep_links(&handle, urls.iter().map(|u| u.to_string()).collect());
    }

    spawn_signal_handler(handle.clone());
    #[cfg(debug_assertions)]
    e2e_hook(handle.clone());

    if start_hidden {
        tray::hide_main_window(&handle); // menu bar only, no Dock icon
    } else {
        tray::show_main_window(&handle);
    }
    if auto_connect {
        let app = handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = core::connect(&app).await {
                notice(&app, "error", format!("Auto-connect failed: {e:#}"));
            }
        });
    }
    Ok(())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "aether=info,warn".into()),
        )
        .init();

    let app = tauri::Builder::default()
        // must be first so a second launch focuses the existing window
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let to_tray = app.state::<AppState>().store.read().unwrap().settings.close_to_tray;
                if to_tray {
                    api.prevent_close();
                    tray::hide_main_window(app);
                } else {
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::import_text,
            commands::import_clipboard,
            commands::add_subscription,
            commands::refresh_subscription,
            commands::refresh_all_subscriptions,
            commands::update_subscription,
            commands::remove_subscription,
            commands::remove_server,
            commands::rename_server,
            commands::select_server,
            commands::connect,
            commands::disconnect,
            commands::toggle_connection,
            commands::save_settings,
            commands::set_mode,
            commands::ping_all,
            commands::ping_server,
            commands::test_real_delay,
            commands::select_fastest,
            commands::get_logs,
            commands::clear_logs,
            commands::export_link,
            commands::server_qr,
            commands::uninstall_vpn_service,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Aether");

    let cleaned_up = AtomicBool::new(false);
    app.run(move |app, event| match event {
        RunEvent::ExitRequested { .. } | RunEvent::Exit => {
            if !cleaned_up.swap(true, std::sync::atomic::Ordering::SeqCst) {
                core::shutdown_blocking(app);
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::show_main_window(app),
        _ => {}
    });
}
