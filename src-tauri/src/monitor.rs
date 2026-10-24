use crate::auth::ShuConnect;
use crate::config::AppConfig;
use crate::network::NetworkManager;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Runtime};

fn log<R: Runtime>(app: &AppHandle<R>, message: impl AsRef<str>) {
    use std::io::Write;
    let message = message.as_ref();
    let _ = app.emit("log_event", message);
    let path = crate::config::ConfigManager::get_config_path().with_file_name("autonet.log");
    let truncate = std::fs::metadata(&path)
        .map(|m| m.len() > 256 * 1024)
        .unwrap_or(false);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .open(path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(file, "{} {}", timestamp, message);
    }
}

pub struct MonitorState {
    is_running: Arc<AtomicBool>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            task_handle: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn start<R: Runtime>(&self, app: AppHandle<R>, config: AppConfig) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            // Already running
            return;
        }

        let is_running_clone = self.is_running.clone();
        let app_clone = app.clone();

        // Commands and tray callbacks do not enter a Tokio runtime. Schedule on
        // Tauri's shared runtime so all monitor entry points are safe.
        let handle = tauri::async_runtime::spawn(async move {
            log(
                &app_clone,
                "[MONITOR] Starting Background Monitor (campus-compatible HTTP headers, direct connection)...",
            );
            let _ = app_clone.emit("monitor_state", true);
            let mut was_online = false;

            while is_running_clone.load(Ordering::SeqCst) {
                let online = NetworkManager::check_online().await;

                if online {
                    if !was_online {
                        log(&app_clone, "[ONLINE] Internet connection verified.");
                    }
                    was_online = true;
                    let _ = app_clone.emit("network_state", true);
                    // Connected normally, wait 10s before next probe
                    tokio::time::sleep(Duration::from_secs(10)).await;
                } else {
                    was_online = false;
                    let _ = app_clone.emit("network_state", false);
                    log(
                        &app_clone,
                        "[MONITOR] Network Offline! Attempting reconnect...",
                    );
                    let auth = ShuConnect::new(config.clone());

                    match auth.start_connect().await {
                        Ok(result) => {
                            log(
                                &app_clone,
                                format!("[MONITOR] {}. Verifying internet access...", result),
                            );
                            tokio::time::sleep(Duration::from_secs(2)).await;

                            if NetworkManager::check_online().await {
                                was_online = true;
                                let _ = app_clone.emit("network_state", true);
                                log(&app_clone, "[ONLINE] Network Restored!");
                            } else {
                                log(&app_clone, "[WARN] Portal responded, but internet access is still unavailable. Retrying in 5s...");
                            }
                        }
                        Err(err) => {
                            log(
                                &app_clone,
                                format!(
                                    "[MONITOR] Reconnect attempt failed: {}. Retrying in 5s...",
                                    err
                                ),
                            );
                        }
                    }

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }

            log(&app_clone, "[MONITOR] Monitoring Stopped.");
            let _ = app_clone.emit("monitor_state", false);
        });

        *self.task_handle.lock() = Some(handle);
    }

    pub fn stop<R: Runtime>(&self, app: &AppHandle<R>) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            if let Some(handle) = self.task_handle.lock().take() {
                handle.abort();
            }
            log(app, "[MONITOR] Monitoring Stopped by user.");
            let _ = app.emit("monitor_state", false);
        }
    }
}
