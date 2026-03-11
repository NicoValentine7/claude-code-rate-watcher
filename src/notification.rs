use chrono::{DateTime, TimeDelta, Utc};
use notify_rust::Notification;

pub struct NotificationState {
    last_warning_sent: Option<DateTime<Utc>>,
    last_codex_warning_sent: Option<DateTime<Utc>>,
}

impl NotificationState {
    pub fn new() -> Self {
        Self {
            last_warning_sent: None,
            last_codex_warning_sent: None,
        }
    }

    pub fn check_and_notify(&mut self, usage_percent: u32) {
        self.check_provider("Claude", usage_percent, &mut self.last_warning_sent.clone());
    }

    pub fn check_and_notify_codex(&mut self, usage_percent: u32) {
        self.check_provider(
            "Codex",
            usage_percent,
            &mut self.last_codex_warning_sent.clone(),
        );
    }

    fn check_provider(
        &mut self,
        provider: &str,
        usage_percent: u32,
        last_sent: &mut Option<DateTime<Utc>>,
    ) {
        let now = Utc::now();

        // Don't re-send within 10 minutes
        if let Some(last) = last_sent {
            if now - *last < TimeDelta::minutes(10) {
                return;
            }
        }

        if usage_percent >= 90 {
            self.send(
                &format!("{} rate limit critical!", provider),
                &format!("Usage at {}%. You may hit the limit soon.", usage_percent),
            );
            *last_sent = Some(now);
        } else if usage_percent >= 75 {
            self.send(
                &format!("{} rate limit warning", provider),
                &format!("Usage at {}% of estimated 5h limit.", usage_percent),
            );
            *last_sent = Some(now);
        }

        // Write back to self
        if provider == "Claude" {
            self.last_warning_sent = *last_sent;
        } else {
            self.last_codex_warning_sent = *last_sent;
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
