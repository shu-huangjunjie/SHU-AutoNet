use crate::auth::ShuConnect;
use crate::config::AppConfig;
use crate::network::NetworkManager;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Runtime};

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
            let _ = app_clone.emit("log_event", "[MONITOR] Starting Background Monitor...");
            let _ = app_clone.emit("monitor_state", true);

            while is_running_clone.load(Ordering::SeqCst) {
                let online = NetworkManager::check_online().await;

                if online {
                    // Connected normally, wait 10s before next probe
                    tokio::time::sleep(Duration::from_secs(10)).await;
                } else {
                    let _ = app_clone.emit(
                        "log_event",
                        "[MONITOR] Network Offline! Attempting reconnect...",
                    );
                    let auth = ShuConnect::new(config.clone());

                    match auth.connect().await {
                        Ok(()) => {
                            let _ = app_clone
                                .emit("log_event", "[MONITOR] Specific connection successful.");
                            tokio::time::sleep(Duration::from_secs(2)).await;

                            if NetworkManager::check_online().await {
                                let _ = app_clone.emit("log_event", "[MONITOR] Network Restored!");
                            }
                        }
                        Err(err) => {
                            let _ = app_clone.emit(
                                "log_event",
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

            let _ = app_clone.emit("log_event", "[MONITOR] Monitoring Stopped.");
            let _ = app_clone.emit("monitor_state", false);
        });

        *self.task_handle.lock() = Some(handle);
    }

    pub fn stop<R: Runtime>(&self, app: &AppHandle<R>) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            if let Some(handle) = self.task_handle.lock().take() {
                handle.abort();
            }
            let _ = app.emit("log_event", "[MONITOR] Monitoring Stopped by user.");
            let _ = app.emit("monitor_state", false);
        }
    }
}
