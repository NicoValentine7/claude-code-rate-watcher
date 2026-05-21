mod api_client;
mod app_bundle;
mod auth;
mod autolaunch;
mod codex_rate;
mod file_watcher;
mod icon;
mod notification;
mod statusline;
mod tray;
mod updater;

use std::fs;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tao::window::WindowBuilder;
use tray_icon::TrayIconEvent;
use wry::WebViewBuilder;

enum AppEvent {
    FileChanged,
    StatusLineUpdate,
    TimerTick,
    UpdateAvailable(updater::UpdateInfo),
    UpdateNotAvailable,
    Resize(f64),
    AuthLoginFailed(String),
    ManualRefresh,
}

const DEBOUNCE_INTERVAL: Duration = Duration::from_secs(1);
const POPOVER_WIDTH: f64 = 340.0;
const POPOVER_HEIGHT: f64 = 200.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("ccrw {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Single instance guard: kill any existing ccrw process
    ensure_single_instance();

    // Create .app bundle in /Applications on first run
    app_bundle::ensure_app_bundle();

    let mut event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();

    // --- Popover window (borderless, hidden initially) ---
    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_visible(false)
        .with_inner_size(LogicalSize::new(POPOVER_WIDTH, POPOVER_HEIGHT))
        .with_title("Rate Watcher")
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
        .with_background_color((0, 0, 0, 0))
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
                    let proxy_auth = proxy_ipc.clone();
                    std::thread::spawn(move || {
                        let claude_bin = auth::find_claude_binary()
                            .unwrap_or_else(|| std::path::PathBuf::from("claude"));
                        eprintln!("[auth] Running: {} auth login", claude_bin.display());
                        match std::process::Command::new(&claude_bin)
                            .args(["auth", "login"])
                            .output()
                        {
                            Ok(output) if output.status.success() => {
                                eprintln!("[auth] Login succeeded");
                                let _ = proxy_auth.send_event(AppEvent::TimerTick);
                            }
                            Ok(output) => {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                eprintln!("[auth] Login failed: {}", stderr);
                                let _ = proxy_auth.send_event(AppEvent::AuthLoginFailed(
                                    stderr.trim().to_string(),
                                ));
                            }
                            Err(e) => {
                                eprintln!("[auth] Failed to run claude: {}", e);
                                let msg = if e.kind() == std::io::ErrorKind::NotFound {
                                    "Claude Code CLI not found. Install it first.".to_string()
                                } else {
                                    format!("Failed to start login: {}", e)
                                };
                                let _ = proxy_auth.send_event(AppEvent::AuthLoginFailed(msg));
                            }
                        }
                    });
                }
                "toggle_launch_at_login" => {
                    let _ = autolaunch::toggle();
                }
                msg if msg.starts_with("resize:") => {
                    if let Ok(h) = msg[7..].parse::<f64>() {
                        let _ = proxy_ipc.send_event(AppEvent::Resize(h));
                    }
                }
                "manual_refresh" => {
                    let _ = proxy_ipc.send_event(AppEvent::ManualRefresh);
                }
                "check_update" => {
                    let updater_check = updater_ipc.clone();
                    let proxy_check = proxy_ipc.clone();
                    std::thread::spawn(move || match updater_check.check() {
                        Some(info) => {
                            let _ = proxy_check.send_event(AppEvent::UpdateAvailable(info));
                        }
                        None => {
                            let _ = proxy_check.send_event(AppEvent::UpdateNotAvailable);
                        }
                    });
                }
                "apply_update" => {
                    if updater::is_homebrew_install() {
                        // Homebrew install: run brew upgrade in background, then restart
                        std::thread::spawn(|| {
                            let status = std::process::Command::new("brew")
                                .args(["upgrade", "NicoValentine7/tap/claude-code-rate-watcher"])
                                .status();
                            match status {
                                Ok(s) if s.success() => {
                                    // Restart with the upgraded binary
                                    let binary = std::env::current_exe().unwrap();
                                    updater::restart_app(&binary);
                                }
                                Ok(s) => eprintln!("brew upgrade exited with: {}", s),
                                Err(e) => eprintln!("brew upgrade failed: {}", e),
                            }
                        });
                    } else if let Some(info) = updater_ipc.get_available() {
                        std::thread::spawn(move || {
                            if let Err(e) = updater::Updater::apply_update(&info) {
                                eprintln!("Update failed: {}", e);
                            }
                        });
                    }
                }
                _ => {}
            }
            let _ = &proxy_ipc; // keep proxy alive
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

    // --- Initial data load ---
    let mut codex_cache = codex_rate::CodexRateCache::default();
    let api_data = api_poller.get_data();
    render_rate_state(&webview, &tray_app, &mut codex_cache, &api_data);
    // Set version in UI
    let _ = webview.evaluate_script(&format!("setVersion('{}')", env!("CARGO_PKG_VERSION")));
    // Enable auto-launch by default on first run
    if !autolaunch::is_enabled() {
        let _ = autolaunch::enable();
    }
    let autolaunch_enabled = autolaunch::is_enabled();
    let _ = webview.evaluate_script(&format!("setAutoLaunch({})", autolaunch_enabled));
    // Always install statusline integration (no toggle needed)
    if !statusline::is_installed() {
        if let Err(e) = statusline::install() {
            eprintln!("[statusline] Auto-install failed: {}", e);
        }
    }
    let mut last_reload = Instant::now();

    // --- File watcher ---
    let (tx, rx) = mpsc::channel();
    let _watcher = file_watcher::start_watcher(tx);

    let proxy_watcher = proxy.clone();
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            match msg {
                file_watcher::WatcherMessage::FileChanged => {
                    let _ = proxy_watcher.send_event(AppEvent::FileChanged);
                }
                file_watcher::WatcherMessage::StatusLineUpdate => {
                    let _ = proxy_watcher.send_event(AppEvent::StatusLineUpdate);
                }
            }
        }
    });

    // --- Timer (30s tick for countdown updates) ---
    let proxy_timer = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let _ = proxy_timer.send_event(AppEvent::TimerTick);
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

    // --- Main event loop ---
    event_loop.run(move |event, _, control_flow| {
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
            api_poller.set_active(popover_visible);
            if popover_visible {
                // Position below the tray icon, centered horizontally
                let x = rect.position.x + (rect.size.width as f64 / 2.0) - (POPOVER_WIDTH / 2.0);
                let y = rect.position.y + rect.size.height as f64 + 4.0;
                window.set_outer_position(PhysicalPosition::new(x, y));
                window.set_visible(true);
                window.set_focus();
                // Force immediate refresh when opening popover
                api_poller.poll();
                let api_data = api_poller.get_data();
                render_rate_state(&webview, &tray_app, &mut codex_cache, &api_data);
            } else {
                window.set_visible(false);
            }
        }

        match event {
            // Hide popover when it loses focus
            Event::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } => {
                popover_visible = false;
                api_poller.set_active(false);
                window.set_visible(false);
            }

            Event::UserEvent(AppEvent::StatusLineUpdate) => {
                // StatusLine data file was updated by Claude Code
                if let Some(data) = statusline::read_rate_data() {
                    api_poller.set_statusline_data(data.clone());
                    let effective_pct =
                        render_rate_state(&webview, &tray_app, &mut codex_cache, &data);
                    notifier.check_and_notify(effective_pct);
                } else {
                    api_poller.clear_statusline();
                }
            }

            Event::UserEvent(AppEvent::FileChanged) => {
                if last_reload.elapsed() < DEBOUNCE_INTERVAL {
                    return;
                }
                last_reload = Instant::now();

                api_poller.poll();
                let api_data = api_poller.get_data();
                let effective_pct =
                    render_rate_state(&webview, &tray_app, &mut codex_cache, &api_data);
                notifier.check_and_notify(effective_pct);
            }

            Event::UserEvent(AppEvent::TimerTick) => {
                api_poller.poll();
                let api_data = api_poller.get_data();
                render_rate_state(&webview, &tray_app, &mut codex_cache, &api_data);
            }

            Event::UserEvent(AppEvent::ManualRefresh) => {
                if api_poller.force_poll() {
                    let api_data = api_poller.get_data();
                    render_rate_state(&webview, &tray_app, &mut codex_cache, &api_data);
                    let _ = webview.evaluate_script("setRefreshResult(true)");
                } else {
                    let remaining = api_poller.get_cooldown_remaining().unwrap_or(0);
                    let _ =
                        webview.evaluate_script(&format!("setRefreshResult(false, {})", remaining));
                }
            }

            Event::UserEvent(AppEvent::Resize(height)) => {
                let clamped = height.max(100.0).min(600.0);
                window.set_inner_size(LogicalSize::new(POPOVER_WIDTH, clamped));
            }

            Event::UserEvent(AppEvent::UpdateAvailable(ref info)) => {
                let js = format!("showUpdateBanner('{}')", info.version.replace('\'', "\\'"));
                let _ = webview.evaluate_script(&js);
            }

            Event::UserEvent(AppEvent::UpdateNotAvailable) => {
                let _ = webview.evaluate_script("showUpToDate()");
            }

            Event::UserEvent(AppEvent::AuthLoginFailed(ref msg)) => {
                let escaped = msg
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'")
                    .replace('\n', "\\n");
                let _ = webview.evaluate_script(&format!("showAuthError('{}')", escaped));
            }

            _ => {}
        }
    });
}

fn render_rate_state(
    webview: &wry::WebView,
    tray_app: &tray::TrayApp,
    codex_cache: &mut codex_rate::CodexRateCache,
    api_data: &api_client::ApiRateLimitData,
) -> u32 {
    let codex_data = codex_cache.load_latest();
    let menu_bar = menu_bar_summary(api_data, codex_data.as_ref());
    tray_app.update_percent(menu_bar.percent);
    push_to_webview(webview, api_data, codex_data.as_ref(), &menu_bar);
    menu_bar.percent
}

fn menu_bar_summary(
    api_data: &api_client::ApiRateLimitData,
    codex_data: Option<&codex_rate::CodexRateLimitData>,
) -> MenuBarSummary {
    let claude_pct = api_data.five_hour_percent.unwrap_or(0);
    let codex_pct = codex_data.and_then(|d| d.five_hour_percent).unwrap_or(0);
    let has_claude = api_data.five_hour_percent.is_some() && !api_data.auth_missing;
    let has_codex = codex_data.and_then(|d| d.five_hour_percent).is_some();

    let (source, label, percent) = if has_codex && codex_pct > claude_pct {
        ("codex", "Codex", codex_pct)
    } else if has_claude && has_codex && claude_pct == codex_pct {
        ("both", "Both", claude_pct)
    } else if has_claude {
        ("claude", "Claude Code", claude_pct)
    } else if has_codex {
        ("codex", "Codex", codex_pct)
    } else {
        ("none", "No data", 0)
    };

    MenuBarSummary {
        percent,
        source,
        label,
    }
}

#[derive(serde::Serialize)]
struct MenuBarSummary {
    percent: u32,
    source: &'static str,
    label: &'static str,
}

#[derive(serde::Serialize)]
struct DashboardPayload<'a> {
    claude: &'a api_client::ApiRateLimitData,
    codex: Option<&'a codex_rate::CodexRateLimitData>,
    menu_bar: &'a MenuBarSummary,
}

fn push_to_webview(
    webview: &wry::WebView,
    api_data: &api_client::ApiRateLimitData,
    codex_data: Option<&codex_rate::CodexRateLimitData>,
    menu_bar: &MenuBarSummary,
) {
    let payload = DashboardPayload {
        claude: api_data,
        codex: codex_data,
        menu_bar,
    };

    if let Ok(json) = serde_json::to_string(&payload) {
        let _ = webview.evaluate_script(&format!("updateData({})", json));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_summary_prefers_codex_when_codex_is_higher() {
        let api_data = api_data(Some(20), false);
        let codex_data = codex_data(Some(42));

        let summary = menu_bar_summary(&api_data, Some(&codex_data));

        assert_eq!(summary.percent, 42);
        assert_eq!(summary.source, "codex");
        assert_eq!(summary.label, "Codex");
    }

    #[test]
    fn menu_bar_summary_prefers_claude_when_claude_is_higher() {
        let api_data = api_data(Some(70), false);
        let codex_data = codex_data(Some(12));

        let summary = menu_bar_summary(&api_data, Some(&codex_data));

        assert_eq!(summary.percent, 70);
        assert_eq!(summary.source, "claude");
        assert_eq!(summary.label, "Claude Code");
    }

    #[test]
    fn menu_bar_summary_marks_tied_sources_as_both() {
        let api_data = api_data(Some(33), false);
        let codex_data = codex_data(Some(33));

        let summary = menu_bar_summary(&api_data, Some(&codex_data));

        assert_eq!(summary.percent, 33);
        assert_eq!(summary.source, "both");
        assert_eq!(summary.label, "Both");
    }

    #[test]
    fn menu_bar_summary_reports_no_data_when_sources_are_missing() {
        let api_data = api_data(None, true);

        let summary = menu_bar_summary(&api_data, None);

        assert_eq!(summary.percent, 0);
        assert_eq!(summary.source, "none");
        assert_eq!(summary.label, "No data");
    }

    fn api_data(
        five_hour_percent: Option<u32>,
        auth_missing: bool,
    ) -> api_client::ApiRateLimitData {
        api_client::ApiRateLimitData {
            five_hour_percent,
            auth_missing,
            ..Default::default()
        }
    }

    fn codex_data(five_hour_percent: Option<u32>) -> codex_rate::CodexRateLimitData {
        codex_rate::CodexRateLimitData {
            five_hour_percent,
            seven_day_percent: None,
            five_hour_resets_at: None,
            seven_day_resets_at: None,
            plan_label: None,
            is_live: true,
            last_updated_at: None,
        }
    }
}

/// Kill any other running ccrw process so only one instance runs at a time.
fn ensure_single_instance() {
    let pid_file = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ccrw.pid");

    let my_pid = std::process::id();

    // Check for existing instance
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Ok(old_pid) = content.trim().parse::<i32>() {
            // Check if process is still alive
            if unsafe { libc::kill(old_pid, 0) } == 0 {
                // Kill the old instance gracefully
                unsafe { libc::kill(old_pid, libc::SIGTERM) };
                // Wait briefly for it to exit
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }

    // Write our PID
    let _ = fs::write(&pid_file, my_pid.to_string());
}
