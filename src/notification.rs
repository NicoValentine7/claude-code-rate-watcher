use chrono::{DateTime, TimeDelta, Utc};
use notify_rust::Notification;

pub struct NotificationState {
    last_warning_sent: Option<DateTime<Utc>>,
}

impl NotificationState {
    pub fn new() -> Self {
        Self {
            last_warning_sent: None,
        }
    }

    pub fn check_and_notify(&mut self, usage_percent: u32) {
        let now = Utc::now();

        // Don't re-send within 10 minutes
        if let Some(last) = self.last_warning_sent {
            if now - last < TimeDelta::minutes(10) {
                return;
            }
        }

        if usage_percent >= 90 {
            self.send(
                "🔴 Rate limit critical!",
                &format!("Usage at {}%. You may hit the limit soon.", usage_percent),
            );
            self.last_warning_sent = Some(now);
        } else if usage_percent >= 75 {
            self.send(
                "🟡 Rate limit warning",
                &format!("Usage at {}% of estimated 5h limit.", usage_percent),
            );
            self.last_warning_sent = Some(now);
        }
    }

    fn send(&self, title: &str, body: &str) {
        let _ = Notification::new()
            .summary(title)
            .body(body)
            .appname("Claude Rate Watcher")
            .show();
    }
}
