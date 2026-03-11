use tao::dpi::LogicalSize;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder};
use wry::{WebView, WebViewBuilder};

use crate::supabase_auth;
use crate::supabase_client;
use crate::AppEvent;

const DASHBOARD_WIDTH: f64 = 820.0;
const DASHBOARD_HEIGHT: f64 = 620.0;

/// Manages the dashboard window lifecycle.
pub struct Dashboard {
    window: Window,
    webview: WebView,
}

impl Dashboard {
    /// Create and show the dashboard window.
    pub fn new(event_loop: &EventLoopWindowTarget<AppEvent>) -> Self {
        let window = WindowBuilder::new()
            .with_title("Rate Watcher - Dashboard")
            .with_inner_size(LogicalSize::new(DASHBOARD_WIDTH, DASHBOARD_HEIGHT))
            .with_resizable(true)
            .with_visible(true)
            .build(event_loop)
            .expect("Failed to create dashboard window");

        let html = include_str!("dashboard.html");

        let webview = WebViewBuilder::new()
            .with_html(html)
            .with_ipc_handler(move |msg| {
                let body = msg.body().to_string();
                Self::handle_ipc(&body);
            })
            .build(&window)
            .expect("Failed to create dashboard webview");

        let dashboard = Self { window, webview };

        // Load initial state
        dashboard.update_auth_state();

        dashboard
    }

    fn handle_ipc(message: &str) {
        match message {
            "login_github" => {
                eprintln!("[dashboard] Login with GitHub requested");
                let rx = supabase_auth::start_login("github");
                std::thread::spawn(move || {
                    if let Ok(result) = rx.recv() {
                        match result {
                            Ok(session) => {
                                eprintln!("[dashboard] GitHub login success: {}", session.user_email);
                            }
                            Err(e) => eprintln!("[dashboard] GitHub login failed: {}", e),
                        }
                    }
                });
            }
            "login_google" => {
                eprintln!("[dashboard] Login with Google requested");
                let rx = supabase_auth::start_login("google");
                std::thread::spawn(move || {
                    if let Ok(result) = rx.recv() {
                        match result {
                            Ok(session) => {
                                eprintln!("[dashboard] Google login success: {}", session.user_email);
                            }
                            Err(e) => eprintln!("[dashboard] Google login failed: {}", e),
                        }
                    }
                });
            }
            "logout" => {
                supabase_auth::logout();
            }
            _ => {}
        }
    }

    /// Update the auth state display in the dashboard.
    pub fn update_auth_state(&self) {
        if let Some(session) = supabase_auth::get_session() {
            let js = format!(
                "setLoggedIn('{}', '{}')",
                session.user_email.replace('\'', "\\'"),
                session.provider.replace('\'', "\\'")
            );
            let _ = self.webview.evaluate_script(&js);
        } else {
            let _ = self.webview.evaluate_script("setLoggedOut()");
        }
    }

    /// Push history data to the dashboard.
    pub fn load_history(&self, json_data: &str) {
        let js = format!("loadHistory({})", json_data);
        let _ = self.webview.evaluate_script(&js);
    }

    /// Fetch and display history from Supabase.
    pub fn refresh_history(&self) {
        let session = match supabase_auth::get_session() {
            Some(s) => s,
            None => return,
        };

        if !supabase_client::is_configured() {
            return;
        }

        // Fetch last 30 days
        let since = (chrono::Utc::now() - chrono::TimeDelta::days(30)).to_rfc3339();

        match supabase_client::fetch_snapshots(&session.access_token, &since, None) {
            Ok(rows) => {
                if let Ok(json) = serde_json::to_string(&rows) {
                    self.load_history(&json);
                }
            }
            Err(e) => eprintln!("[dashboard] Fetch history failed: {}", e),
        }
    }

    /// Show/focus the dashboard window.
    pub fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
        self.update_auth_state();
    }

    /// Get the window ID for event matching.
    pub fn window_id(&self) -> tao::window::WindowId {
        self.window.id()
    }
}
