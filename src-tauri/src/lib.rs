pub mod auth;
pub mod autostart;
pub mod config;
pub mod monitor;
pub mod network;
#[cfg(test)]
mod test_http;

use auth::ShuConnect;
use autostart::AutoStartManager;
use config::{AppConfig, ConfigManager};
use monitor::MonitorState;
use network::NetworkManager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};

#[tauri::command]
fn get_config() -> AppConfig {
    let mut cfg = ConfigManager::load();
    cfg.start_with_windows = AutoStartManager::is_enabled();
    cfg
}

#[tauri::command]
fn save_config(mut config: AppConfig) -> Result<(), String> {
    AutoStartManager::apply(config.start_with_windows, config.launch_minimized)?;
    config.start_with_windows = AutoStartManager::is_enabled();
    ConfigManager::save(&config)
}

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "portable": ConfigManager::is_portable()
    })
}

#[tauri::command]
fn is_monitoring(state: State<'_, MonitorState>) -> bool {
    state.is_running()
}

#[tauri::command]
fn toggle_monitoring(app: AppHandle, state: State<'_, MonitorState>) -> Result<bool, String> {
    if state.is_running() {
        state.stop(&app);
        Ok(false)
    } else {
        let config = ConfigManager::load();
        if config.user_id.is_empty() || config.password.is_empty() {
            return Err("User ID and Password must be configured before starting.".into());
        }
        state.start(app, config);
        Ok(true)
    }
}

/// Idempotent start used by the frontend auto-start; never stops a running monitor.
#[tauri::command]
fn start_monitoring(app: AppHandle, state: State<'_, MonitorState>) -> Result<bool, String> {
    if !state.is_running() {
        let config = ConfigManager::load();
        if config.user_id.is_empty() || config.password.is_empty() {
            return Err("User ID and Password must be configured before starting.".into());
        }
        state.start(app, config);
    }
    Ok(true)
}

#[tauri::command]
async fn manual_connect() -> Result<String, String> {
    let config = ConfigManager::load();
    if config.user_id.is_empty() || config.password.is_empty() {
        return Err("Credentials are missing in configuration.".into());
    }
    let auth = ShuConnect::new(config);
    auth.start_connect().await
}

#[tauri::command]
async fn check_online() -> bool {
    NetworkManager::check_online().await
}

pub fn run() {
    let monitor_state = MonitorState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(monitor_state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_app_info,
            save_config,
            is_monitoring,
            toggle_monitoring,
            start_monitoring,
            manual_connect,
            check_online
        ])
        .setup(|app| {
            let handle = app.handle();

            // Tray Menu Setup
            let show_item = MenuItem::with_id(handle, "show", "Show Window", true, None::<&str>)?;
            let toggle_item = MenuItem::with_id(handle, "toggle", "Toggle Monitoring", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(handle, "quit", "Exit", true, None::<&str>)?;

            let tray_menu = Menu::with_items(handle, &[&show_item, &toggle_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("SHU AutoNet")
                .menu(&tray_menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "toggle" => {
                        let state = app.state::<MonitorState>();
                        let _ = toggle_monitoring(app.clone(), state);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(handle)?;

            // Check if CLI args contain --minimized
            let args: Vec<String> = std::env::args().collect();
            let start_minimized = args.iter().any(|a| a == "--minimized");

            if let Some(window) = app.get_webview_window("main") {
                if start_minimized {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                let cfg = ConfigManager::load();
                if cfg.close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            WindowEvent::Resized(_) => {
                // Only touch the config file when the window was actually minimized
                if window.is_minimized().unwrap_or(false) && ConfigManager::load().minimize_to_tray {
                    let _ = window.unminimize();
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
