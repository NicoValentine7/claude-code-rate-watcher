use tray_icon::TrayIconBuilder;

use crate::icon::generate_status_icon;

pub struct TrayApp {
    tray: tray_icon::TrayIcon,
    debug_label: Option<String>,
}

impl TrayApp {
    pub fn new() -> Self {
        let icon = generate_status_icon(0);
        let debug_label = std::env::var("CCRW_DEBUG_LABEL").ok();

        let title = match &debug_label {
            Some(label) => format!("[{}] 0%", label),
            None => "0%".to_string(),
        };

        // No menu — left-click toggles the popover,
        // Quit is handled inside the WebView.
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Claude Code Rate Watcher")
            .with_title(&title)
            .build()
            .expect("Failed to create tray icon");

        Self { tray, debug_label }
    }

    pub fn update_percent(&self, percent: u32) {
        let new_icon = generate_status_icon(percent);
        let _ = self.tray.set_icon(Some(new_icon));
        let title = match &self.debug_label {
            Some(label) => format!("[{}] {}%", label, percent),
            None => format!("{}%", percent),
        };
        self.tray.set_title(Some(&title));
    }
}
