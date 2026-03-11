use serde::{Deserialize, Serialize};
use std::time::Duration;

// TODO: Replace these with your actual Supabase project values
const SUPABASE_URL: &str = "https://YOUR_PROJECT.supabase.co";
const SUPABASE_ANON_KEY: &str = "YOUR_ANON_KEY";

/// Public reference to the anon key for use by supabase_auth.
pub const SUPABASE_ANON_KEY_PUB: &str = SUPABASE_ANON_KEY;

/// A snapshot of usage data to be stored in Supabase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub user_id: String,
    pub snapshot_type: String, // "periodic" or "window_reset"
    pub five_hour_percent: i32,
    pub seven_day_percent: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day_tokens: Option<i64>,
    pub is_live: bool,
    pub recorded_at: String, // ISO 8601
}

/// Row returned from Supabase query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRow {
    pub id: String,
    pub snapshot_type: String,
    pub five_hour_percent: i32,
    pub seven_day_percent: i32,
    pub five_hour_tokens: Option<i64>,
    pub seven_day_tokens: Option<i64>,
    pub is_live: bool,
    pub recorded_at: String,
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    )
}

/// Insert a snapshot into Supabase.
pub fn insert_snapshot(access_token: &str, snapshot: &UsageSnapshot) -> Result<(), String> {
    let agent = build_agent();
    let url = format!("{}/rest/v1/usage_snapshots", SUPABASE_URL);

    agent
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", &format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .send_json(snapshot)
        .map_err(|e| format!("Insert snapshot failed: {}", e))?;

    Ok(())
}

/// Fetch snapshots from Supabase since a given ISO timestamp.
pub fn fetch_snapshots(
    access_token: &str,
    since: &str,
    snapshot_type: Option<&str>,
) -> Result<Vec<SnapshotRow>, String> {
    let agent = build_agent();
    let mut url = format!(
        "{}/rest/v1/usage_snapshots?select=*&recorded_at=gte.{}&order=recorded_at.asc",
        SUPABASE_URL, since
    );
    if let Some(st) = snapshot_type {
        url.push_str(&format!("&snapshot_type=eq.{}", st));
    }

    let mut response = agent
        .get(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", &format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .call()
        .map_err(|e| format!("Fetch snapshots failed: {}", e))?;

    let rows: Vec<SnapshotRow> = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Parse snapshots failed: {}", e))?;

    Ok(rows)
}

/// Check if the Supabase constants have been configured.
pub fn is_configured() -> bool {
    !SUPABASE_URL.contains("YOUR_PROJECT") && !SUPABASE_ANON_KEY.contains("YOUR_ANON_KEY")
}
