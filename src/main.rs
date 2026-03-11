mod api_client;
mod auth;
mod autolaunch;
mod dashboard;
mod file_watcher;
mod history_recorder;
mod icon;
mod notification;
mod session_parser;
mod supabase_auth;
mod supabase_client;
mod tray;
mod updater;
mod usage_tracker;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::TrayIconEvent;
use wry::WebViewBuilder;

pub enum AppEvent {
    FileChanged,
    TimerTick,
    UpdateAvailable(updater::UpdateInfo),
    OpenDashboard,
    SnapshotTick,
    AuthCompleted(supabase_auth::SupabaseSession),
    AuthFailed(String),
}

const DEBOUNCE_INTERVAL: Duration = Duration::from_secs(1);
const POPOVER_WIDTH: f64 = 340.0;
const POPOVER_HEIGHT: f64 = 470.0;

fn main() {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // --- Popover window (borderless, hidden initially) ---
    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_visible(false)
        .with_inner_size(LogicalSize::new(POPOVER_WIDTH, POPOVER_HEIGHT))
        .with_title("Claude Rate Watcher")
        .build(&event_loop)
        .expect("Failed to create window");

    // --- Updater (check for new versions) ---
    let app_updater = std::sync::Arc::new(updater::Updater::new());

    // --- WebView inside the popover ---
    let html = include_str!("popover.html");

    let proxy_ipc = proxy.clone();
    let updater_ipc = app_updater.clone();
    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(html)
        .with_ipc_handler(move |msg| {
            match msg.body().as_str() {
                "quit" => std::process::exit(0),
                "open_usage" => {
                    let _ = std::process::Command::new("open")
                        .arg("https://claude.ai/settings/usage")
                        .spawn();
                }
                "auth_login" => {
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", "tell application \"Terminal\" to do script \"claude login\""])
                        .spawn();
                }
                "toggle_launch_at_login" => {
                    let _ = autolaunch::toggle();
                }
                "apply_update" => {
                    if let Some(info) = updater_ipc.get_available() {
                        std::thread::spawn(move || {
                            if let Err(e) = updater::Updater::apply_update(&info) {
                                eprintln!("Update failed: {}", e);
                            }
                        });
                    }
                }
                "open_dashboard" => {
                    let _ = proxy_ipc.send_event(AppEvent::OpenDashboard);
                }
                _ => {}
            }
        })
        .build(&window)
        .expect("Failed to create webview");

    // --- Tray icon (menu bar) ---
    let tray_app = tray::TrayApp::new();

    // --- Notification state ---
    let mut notifier = notification::NotificationState::new();

    // --- API poller (fetches real rate limit data) ---
    let api_poller = api_client::ApiPoller::new();
    api_poller.poll(); // Initial fetch

    // --- History recorder ---
    let mut history_recorder = history_recorder::HistoryRecorder::new();

    // --- Initial data load ---
    let mut all_records = session_parser::load_all_sessions();
    let summary = usage_tracker::calculate_usage(&all_records);
    let api_data = api_poller.get_data();
    let effective_pct = api_data.five_hour_percent.unwrap_or(0);
    tray_app.update_percent(effective_pct);
    push_to_webview(&webview, &summary, &api_data);
    let autolaunch_enabled = autolaunch::is_enabled();
    let _ = webview.evaluate_script(&format!("setAutoLaunch({})", autolaunch_enabled));
    let mut last_reload = Instant::now();

    // --- File watcher ---
    let (tx, rx) = mpsc::channel();
    let _watcher = file_watcher::start_watcher(tx);

    let proxy_watcher = proxy.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            let _ = proxy_watcher.send_event(AppEvent::FileChanged);
        }
    });

    // --- Timer (30s tick for countdown updates) ---
    let proxy_timer = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let _ = proxy_timer.send_event(AppEvent::TimerTick);
    });

    // --- Snapshot timer (1h for periodic snapshots) ---
    let proxy_snapshot = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3600));
        let _ = proxy_snapshot.send_event(AppEvent::SnapshotTick);
    });

    // --- Update checker thread ---
    let proxy_update = proxy.clone();
    let updater_clone = app_updater.clone();
    std::thread::spawn(move || {
        // Check on startup (after a short delay)
        std::thread::sleep(Duration::from_secs(5));
        if let Some(info) = updater_clone.check() {
            let _ = proxy_update.send_event(AppEvent::UpdateAvailable(info));
        }
        // Then check periodically
        loop {
            std::thread::sleep(Duration::from_secs(6 * 3600));
            if let Some(info) = updater_clone.check() {
                let _ = proxy_update.send_event(AppEvent::UpdateAvailable(info));
            }
        }
    });

    // --- Event receivers ---
    let tray_channel = TrayIconEvent::receiver();
    let mut popover_visible = false;
    let mut dashboard_state: Option<dashboard::Dashboard> = None;

    // --- Main event loop ---
    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        // Tray icon click → toggle popover
        if let Ok(TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            rect,
            ..
        }) = tray_channel.try_recv()
        {
            popover_visible = !popover_visible;
            if popover_visible {
                // Position below the tray icon, centered horizontally
                let x = rect.position.x + (rect.size.width as f64 / 2.0) - (POPOVER_WIDTH / 2.0);
                let y = rect.position.y + rect.size.height as f64 + 4.0;
                window.set_outer_position(PhysicalPosition::new(x, y));
                window.set_visible(true);
                window.set_focus();
            } else {
                window.set_visible(false);
            }
        }

        match event {
            // Hide popover when it loses focus (but not the dashboard)
            Event::WindowEvent {
                event: WindowEvent::Focused(false),
                window_id,
                ..
            } if window_id == window.id() => {
                popover_visible = false;
                window.set_visible(false);
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                // If dashboard window is closed, drop it
                if let Some(ref dash) = dashboard_state {
                    if window_id == dash.window_id() {
                        dashboard_state = None;
                    }
                }
            }

            Event::UserEvent(AppEvent::FileChanged) => {
                if last_reload.elapsed() < DEBOUNCE_INTERVAL {
                    return;
                }
                last_reload = Instant::now();

                all_records = session_parser::load_all_sessions();
                let summary = usage_tracker::calculate_usage(&all_records);
                api_poller.poll();
                let api_data = api_poller.get_data();
                let effective_pct = api_data.five_hour_percent.unwrap_or(0);
                let weekly_pct = api_data.seven_day_percent.unwrap_or(0);
                tray_app.update_percent(effective_pct);
                push_to_webview(&webview, &summary, &api_data);
                notifier.check_and_notify(effective_pct);

                // Feed history recorder
                history_recorder.on_data_update(
                    effective_pct,
                    weekly_pct,
                    summary.reset_time,
                    Some(summary.total_tokens() as i64),
                    Some(summary.weekly_total_tokens() as i64),
                    api_data.is_live,
                );
            }

            Event::UserEvent(AppEvent::TimerTick) => {
                let summary = usage_tracker::calculate_usage(&all_records);
                api_poller.poll();
                let api_data = api_poller.get_data();
                let effective_pct = api_data.five_hour_percent.unwrap_or(0);
                let weekly_pct = api_data.seven_day_percent.unwrap_or(0);
                tray_app.update_percent(effective_pct);
                push_to_webview(&webview, &summary, &api_data);

                // Feed history recorder
                history_recorder.on_data_update(
                    effective_pct,
                    weekly_pct,
                    summary.reset_time,
                    Some(summary.total_tokens() as i64),
                    Some(summary.weekly_total_tokens() as i64),
                    api_data.is_live,
                );

                // Flush pending snapshots in background
                history_recorder.flush();
            }

            Event::UserEvent(AppEvent::SnapshotTick) => {
                let summary = usage_tracker::calculate_usage(&all_records);
                let api_data = api_poller.get_data();
                let effective_pct = api_data.five_hour_percent.unwrap_or(0);
                let weekly_pct = api_data.seven_day_percent.unwrap_or(0);

                history_recorder.record_periodic(
                    effective_pct,
                    weekly_pct,
                    Some(summary.total_tokens() as i64),
                    Some(summary.weekly_total_tokens() as i64),
                    api_data.is_live,
                );
                history_recorder.flush();
            }

            Event::UserEvent(AppEvent::OpenDashboard) => {
                if let Some(ref dash) = dashboard_state {
                    dash.show();
                } else {
                    let dash = dashboard::Dashboard::new(event_loop_target);
                    dash.refresh_history();
                    dashboard_state = Some(dash);
                }
            }

            Event::UserEvent(AppEvent::UpdateAvailable(ref info)) => {
                let js = format!(
                    "showUpdateBanner('{}')",
                    info.version.replace('\'', "\\'")
                );
                let _ = webview.evaluate_script(&js);
            }

            Event::UserEvent(AppEvent::AuthCompleted(ref _session)) => {
                if let Some(ref dash) = dashboard_state {
                    dash.update_auth_state();
                    dash.refresh_history();
                }
                // Flush any pending snapshots
                history_recorder.flush();
            }

            Event::UserEvent(AppEvent::AuthFailed(ref msg)) => {
                eprintln!("[main] Auth failed: {}", msg);
            }

            _ => {}
        }
    });
}

fn push_to_webview(
    webview: &wry::WebView,
    summary: &usage_tracker::UsageSummary,
    api_data: &api_client::ApiRateLimitData,
) {
    let payload = summary.to_payload(api_data);
    if let Ok(json) = serde_json::to_string(&payload) {
        let _ = webview.evaluate_script(&format!("updateData({})", json));
    }
}
