use shu_autonet_lib::{config::AppConfig, monitor::MonitorState};
use std::sync::mpsc;
use std::time::Duration;
use tauri::Listener;

// UI commands and tray callbacks run on threads without a Tokio context.
#[test]
fn monitor_starts_and_restarts_without_tokio_context() {
    let app = tauri::test::mock_app();
    let state = MonitorState::new();
    let (events_tx, events_rx) = mpsc::channel();
    app.listen("monitor_state", move |event| {
        events_tx.send(event.payload().to_string()).unwrap();
    });
    assert!(tokio::runtime::Handle::try_current().is_err());

    for _ in 0..2 {
        state.start(app.handle().clone(), AppConfig::default());
        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "true"
        );
        assert!(state.is_running());

        state.start(app.handle().clone(), AppConfig::default());
        assert!(events_rx.recv_timeout(Duration::from_millis(50)).is_err());

        state.stop(app.handle());
        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "false"
        );
        assert!(!state.is_running());
    }
}
