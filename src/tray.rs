use tray_icon::TrayIconBuilder;

use crate::icon::generate_status_icon;

pub struct TrayApp {
    tray: tray_icon::TrayIcon,
}

impl TrayApp {
    pub fn new() -> Self {
        let icon = generate_status_icon(0);

        // No menu — left-click toggles the popover,
        // Quit is handled inside the WebView.
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Claude Code Rate Watcher")
            .with_title("0%")
            .build()
            .expect("Failed to create tray icon");

        Self { tray }
    }

    pub fn update_percent(&self, percent: u32) {
        let new_icon = generate_status_icon(percent);
        let _ = self.tray.set_icon(Some(new_icon));
        self.tray.set_title(Some(&format!("{}%", percent)));
    }
}
