use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

use crate::session_parser::UsageRecord;

pub const WINDOW_HOURS: i64 = 5;
pub const WEEKLY_HOURS: i64 = 7 * 24; // 168 hours

// Estimated token limits (Max plan heuristic).
// Anthropic doesn't publish exact numbers, so these are rough estimates.
// Adjust based on your own experience.
pub const ESTIMATED_LIMIT_5H: u64 = 25_000_000;
pub const ESTIMATED_LIMIT_WEEKLY: u64 = 225_000_000;

/// Token stats for a time window.
struct WindowStats {
    input: u64,
    output: u64,
    cache_creation: u64,
    cache_read: u64,
    message_count: usize,
    oldest_timestamp: Option<DateTime<Utc>>,
}

pub struct UsageSummary {
    // 5-hour window
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub reset_time: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub usage_percent: u32,
    // Weekly window
    pub weekly_input_tokens: u64,
    pub weekly_output_tokens: u64,
    pub weekly_cache_creation_tokens: u64,
    pub weekly_cache_read_tokens: u64,
    pub weekly_reset_time: Option<DateTime<Utc>>,
    pub weekly_message_count: usize,
    pub weekly_usage_percent: u32,
}

/// JSON-serializable data sent to the WebView.
#[derive(Serialize)]
pub struct UsagePayload {
    pub usage_percent: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_tokens: u64,
    pub message_count: usize,
    pub reset_text: Option<String>,
    // Weekly
    pub weekly_usage_percent: u32,
    pub weekly_total_tokens: u64,
    pub weekly_message_count: usize,
    pub weekly_reset_text: Option<String>,
    // API live data (overrides local estimates when available)
    pub api_5h_percent: Option<u32>,
    pub api_7d_percent: Option<u32>,
    pub api_5h_reset: Option<String>,
    pub api_7d_reset: Option<String>,
    pub is_live: bool,
    pub auth_missing: bool,
}

impl UsageSummary {
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens
            + self.total_output_tokens
            + self.total_cache_creation_tokens
            + self.total_cache_read_tokens
    }

    pub fn weekly_total_tokens(&self) -> u64 {
        self.weekly_input_tokens
            + self.weekly_output_tokens
            + self.weekly_cache_creation_tokens
            + self.weekly_cache_read_tokens
    }

    pub fn to_payload(
        &self,
        api_data: &crate::api_client::ApiRateLimitData,
    ) -> UsagePayload {
        UsagePayload {
            usage_percent: self.usage_percent,
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
            total_cache_creation_tokens: self.total_cache_creation_tokens,
            total_cache_read_tokens: self.total_cache_read_tokens,
            total_tokens: self.total_tokens(),
            message_count: self.message_count,
            reset_text: self.reset_time.map(format_remaining_time_short),
            weekly_usage_percent: self.weekly_usage_percent,
            weekly_total_tokens: self.weekly_total_tokens(),
            weekly_message_count: self.weekly_message_count,
            weekly_reset_text: self.weekly_reset_time.map(format_remaining_time_long),
            api_5h_percent: api_data.five_hour_percent,
            api_7d_percent: api_data.seven_day_percent,
            api_5h_reset: api_data.five_hour_resets_at.clone(),
            api_7d_reset: api_data.seven_day_resets_at.clone(),
            is_live: api_data.is_live,
            auth_missing: api_data.auth_missing,
        }
    }
}

fn calc_window_stats(records: &[UsageRecord], window_start: DateTime<Utc>) -> WindowStats {
    let in_window: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| r.timestamp >= window_start)
        .collect();

    WindowStats {
        input: in_window.iter().map(|r| r.usage.input_tokens).sum(),
        output: in_window.iter().map(|r| r.usage.output_tokens).sum(),
        cache_creation: in_window
            .iter()
            .map(|r| r.usage.cache_creation_input_tokens)
            .sum(),
        cache_read: in_window
            .iter()
            .map(|r| r.usage.cache_read_input_tokens)
            .sum(),
        message_count: in_window.len(),
        oldest_timestamp: in_window.iter().map(|r| r.timestamp).min(),
    }
}

/// Weighted token cost estimation:
/// - output tokens cost ~5x more than input in API pricing
/// - cache reads are cheap (~10% of regular input)
/// - cache creation is same as regular input
fn weighted_tokens(stats: &WindowStats) -> u64 {
    stats.input + (stats.output * 5) + stats.cache_creation + (stats.cache_read / 10)
}

pub fn calculate_usage(records: &[UsageRecord]) -> UsageSummary {
    let now = Utc::now();

    // 5-hour window
    let h5 = calc_window_stats(records, now - TimeDelta::hours(WINDOW_HOURS));
    let h5_reset = h5
        .oldest_timestamp
        .map(|t| t + TimeDelta::hours(WINDOW_HOURS));
    let h5_pct = ((weighted_tokens(&h5) as f64 / ESTIMATED_LIMIT_5H as f64) * 100.0).min(100.0)
        as u32;

    // Weekly window
    let w7 = calc_window_stats(records, now - TimeDelta::hours(WEEKLY_HOURS));
    let w7_reset = w7
        .oldest_timestamp
        .map(|t| t + TimeDelta::hours(WEEKLY_HOURS));
    let w7_pct = ((weighted_tokens(&w7) as f64 / ESTIMATED_LIMIT_WEEKLY as f64) * 100.0)
        .min(100.0) as u32;

    UsageSummary {
        total_input_tokens: h5.input,
        total_output_tokens: h5.output,
        total_cache_creation_tokens: h5.cache_creation,
        total_cache_read_tokens: h5.cache_read,
        reset_time: h5_reset,
        message_count: h5.message_count,
        usage_percent: h5_pct,
        weekly_input_tokens: w7.input,
        weekly_output_tokens: w7.output,
        weekly_cache_creation_tokens: w7.cache_creation,
        weekly_cache_read_tokens: w7.cache_read,
        weekly_reset_time: w7_reset,
        weekly_message_count: w7.message_count,
        weekly_usage_percent: w7_pct,
    }
}

fn format_remaining_time_short(reset: DateTime<Utc>) -> String {
    let remaining = reset - Utc::now();
    let total_secs = remaining.num_seconds();
    if total_secs <= 0 {
        return "Window clear".to_string();
    }
    let h = remaining.num_hours();
    let m = remaining.num_minutes() % 60;
    format!("Resets in: {}h {:02}m", h, m)
}

fn format_remaining_time_long(reset: DateTime<Utc>) -> String {
    let remaining = reset - Utc::now();
    let total_secs = remaining.num_seconds();
    if total_secs <= 0 {
        return "Window clear".to_string();
    }
    let d = remaining.num_days();
    let h = remaining.num_hours() % 24;
    let m = remaining.num_minutes() % 60;
    if d > 0 {
        format!("Resets in: {}d {}h {:02}m", d, h, m)
    } else {
        format!("Resets in: {}h {:02}m", h, m)
    }
}
