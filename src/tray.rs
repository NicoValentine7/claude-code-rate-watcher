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

    pub fn update_percent(&self, claude_pct: u32, codex_pct: u32) {
        let max_pct = claude_pct.max(codex_pct);
        let new_icon = generate_status_icon(max_pct);
        let _ = self.tray.set_icon(Some(new_icon));

        if codex_pct > 0 {
            self.tray
                .set_title(Some(&format!("C:{}% X:{}%", claude_pct, codex_pct)));
        } else {
            self.tray.set_title(Some(&format!("{}%", claude_pct)));
        }
    }
}
