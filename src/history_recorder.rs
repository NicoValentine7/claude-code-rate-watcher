use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

use crate::supabase_auth;
use crate::supabase_client::{self, UsageSnapshot};

const MAX_QUEUE_SIZE: usize = 168; // 7 days of hourly snapshots

/// Pending snapshot waiting to be uploaded.
#[derive(Debug, Clone)]
struct PendingSnapshot {
    snapshot_type: String,
    five_hour_percent: i32,
    seven_day_percent: i32,
    five_hour_tokens: Option<i64>,
    seven_day_tokens: Option<i64>,
    is_live: bool,
    recorded_at: String,
}

/// Tracks window reset events and records usage snapshots.
pub struct HistoryRecorder {
    /// Previous reset time for 5h window (to detect resets)
    prev_reset_time: Option<DateTime<Utc>>,
    /// Peak 5h usage % since last reset
    peak_5h_percent: u32,
    /// Peak 7d usage % since last reset
    peak_7d_percent: u32,
    /// Offline queue of snapshots waiting to be uploaded
    pending_queue: Arc<Mutex<Vec<PendingSnapshot>>>,
    /// Last periodic snapshot timestamp
    last_periodic: Option<std::time::Instant>,
}

impl HistoryRecorder {
    pub fn new() -> Self {
        Self {
            prev_reset_time: None,
            peak_5h_percent: 0,
            peak_7d_percent: 0,
            pending_queue: Arc::new(Mutex::new(Vec::new())),
            last_periodic: None,
        }
    }

    /// Called on every data update (TimerTick / FileChanged).
    /// Detects window resets and updates peak tracking.
    pub fn on_data_update(
        &mut self,
        five_hour_percent: u32,
        seven_day_percent: u32,
        reset_time: Option<DateTime<Utc>>,
        five_hour_tokens: Option<i64>,
        seven_day_tokens: Option<i64>,
        is_live: bool,
    ) {
        // Update peak tracking
        self.peak_5h_percent = self.peak_5h_percent.max(five_hour_percent);
        self.peak_7d_percent = self.peak_7d_percent.max(seven_day_percent);

        // Detect window reset
        let reset_occurred = match (self.prev_reset_time, reset_time) {
            (Some(prev), Some(curr)) if curr > prev => true,
            (Some(_), None) => true,
            _ => false,
        };

        if reset_occurred {
            eprintln!(
                "[history] Window reset detected. Peak 5h: {}%, 7d: {}%",
                self.peak_5h_percent, self.peak_7d_percent
            );
            self.queue_snapshot(
                "window_reset",
                self.peak_5h_percent as i32,
                self.peak_7d_percent as i32,
                five_hour_tokens,
                seven_day_tokens,
                is_live,
            );
            self.peak_5h_percent = five_hour_percent;
            self.peak_7d_percent = seven_day_percent;
        }

        self.prev_reset_time = reset_time;
    }

    /// Record a periodic (hourly) snapshot.
    pub fn record_periodic(
        &mut self,
        five_hour_percent: u32,
        seven_day_percent: u32,
        five_hour_tokens: Option<i64>,
        seven_day_tokens: Option<i64>,
        is_live: bool,
    ) {
        self.last_periodic = Some(std::time::Instant::now());
        eprintln!(
            "[history] Periodic snapshot: 5h={}%, 7d={}%",
            five_hour_percent, seven_day_percent
        );
        self.queue_snapshot(
            "periodic",
            five_hour_percent as i32,
            seven_day_percent as i32,
            five_hour_tokens,
            seven_day_tokens,
            is_live,
        );
    }

    fn queue_snapshot(
        &self,
        snapshot_type: &str,
        five_hour_percent: i32,
        seven_day_percent: i32,
        five_hour_tokens: Option<i64>,
        seven_day_tokens: Option<i64>,
        is_live: bool,
    ) {
        let snapshot = PendingSnapshot {
            snapshot_type: snapshot_type.to_string(),
            five_hour_percent,
            seven_day_percent,
            five_hour_tokens,
            seven_day_tokens,
            is_live,
            recorded_at: Utc::now().to_rfc3339(),
        };

        let mut queue = self.pending_queue.lock().unwrap();
        queue.push(snapshot);

        // Trim to max size (drop oldest)
        while queue.len() > MAX_QUEUE_SIZE {
            queue.remove(0);
        }
    }

    /// Attempt to flush pending snapshots to Supabase.
    /// Call this periodically or after auth events.
    pub fn flush(&self) {
        if !supabase_client::is_configured() {
            return;
        }

        let session = match supabase_auth::get_session() {
            Some(s) => s,
            None => return, // Not logged in, keep queued
        };

        let mut queue = self.pending_queue.lock().unwrap();
        if queue.is_empty() {
            return;
        }

        let snapshots: Vec<PendingSnapshot> = queue.drain(..).collect();
        drop(queue); // Release lock before network calls

        let mut failed: Vec<PendingSnapshot> = Vec::new();

        for snap in snapshots {
            let usage_snap = UsageSnapshot {
                id: None,
                user_id: session.user_id.clone(),
                snapshot_type: snap.snapshot_type.clone(),
                five_hour_percent: snap.five_hour_percent,
                seven_day_percent: snap.seven_day_percent,
                five_hour_tokens: snap.five_hour_tokens,
                seven_day_tokens: snap.seven_day_tokens,
                is_live: snap.is_live,
                recorded_at: snap.recorded_at.clone(),
            };

            if let Err(e) = supabase_client::insert_snapshot(&session.access_token, &usage_snap) {
                eprintln!("[history] Upload failed: {}", e);
                failed.push(snap);
            }
        }

        // Re-queue failed ones
        if !failed.is_empty() {
            let mut queue = self.pending_queue.lock().unwrap();
            for snap in failed {
                queue.push(snap);
            }
        }
    }

    /// Get the number of pending snapshots.
    pub fn pending_count(&self) -> usize {
        self.pending_queue.lock().unwrap().len()
    }
}
