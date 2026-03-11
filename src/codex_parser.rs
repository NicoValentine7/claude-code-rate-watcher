use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Deserialize)]
struct CodexEntry {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    entry_type: String,
    payload: Option<CodexPayload>,
}

#[derive(Debug, Deserialize)]
struct CodexPayload {
    #[serde(rename = "type")]
    payload_type: Option<String>,
    info: Option<CodexTokenInfo>,
    rate_limits: Option<CodexRateLimits>,
}

#[derive(Debug, Deserialize)]
struct CodexTokenInfo {
    total_token_usage: Option<CodexTokenUsage>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexTokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimits {
    primary: Option<CodexWindow>,
    secondary: Option<CodexWindow>,
    plan_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexWindow {
    used_percent: Option<f64>,
    resets_at: Option<i64>,
}

/// Codex usage data extracted from JSONL session files.
#[derive(Debug, Clone, Serialize)]
pub struct CodexUsage {
    pub primary_percent: Option<u32>,
    pub secondary_percent: Option<u32>,
    pub primary_resets_at: Option<String>,   // ISO 8601
    pub secondary_resets_at: Option<String>, // ISO 8601
    pub plan_type: Option<String>,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub timestamp: Option<String>,
}

fn codex_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("archived_sessions"))
}

/// Load the latest rate limit data from Codex session files.
///
/// Scans `~/.codex/archived_sessions/rollout-*.jsonl`, only considering
/// files modified within the last 7 days. Returns the most recent
/// `token_count` record that has non-null `rate_limits`.
pub fn load_latest_usage() -> Option<CodexUsage> {
    let dir = codex_sessions_dir()?;
    if !dir.exists() {
        return None;
    }

    let seven_days_ago = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    // Collect recent JSONL files, sorted by modification time (newest first)
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified >= seven_days_ago {
                            files.push((path, modified));
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| b.1.cmp(&a.1));

    // Find the latest token_count record with rate_limits across all files
    let mut best: Option<(DateTime<Utc>, CodexUsage)> = None;

    for (path, _) in &files {
        if let Some((ts, usage)) = parse_latest_token_count(path) {
            match &best {
                Some((existing_ts, _)) if ts > *existing_ts => {
                    best = Some((ts, usage));
                }
                None => {
                    best = Some((ts, usage));
                }
                _ => {}
            }
        }
    }

    best.map(|(_, usage)| usage)
}

fn parse_latest_token_count(path: &Path) -> Option<(DateTime<Utc>, CodexUsage)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut best: Option<(DateTime<Utc>, CodexUsage)> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.is_empty() {
            continue;
        }

        let entry: CodexEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.entry_type != "event_msg" {
            continue;
        }

        let payload = match entry.payload {
            Some(p) => p,
            None => continue,
        };

        if payload.payload_type.as_deref() != Some("token_count") {
            continue;
        }

        let rate_limits = match payload.rate_limits {
            Some(rl) => rl,
            None => continue,
        };

        // Need at least primary window data
        let primary = match &rate_limits.primary {
            Some(p) if p.used_percent.is_some() => p,
            _ => continue,
        };

        let timestamp = match entry.timestamp {
            Some(ref ts) => match ts.parse::<DateTime<Utc>>() {
                Ok(dt) => dt,
                Err(_) => continue,
            },
            None => continue,
        };

        let (input, cached, output, reasoning, total) =
            if let Some(ref info) = payload.info {
                if let Some(ref tu) = info.total_token_usage {
                    (
                        tu.input_tokens,
                        tu.cached_input_tokens,
                        tu.output_tokens,
                        tu.reasoning_output_tokens,
                        tu.total_tokens,
                    )
                } else {
                    (0, 0, 0, 0, 0)
                }
            } else {
                (0, 0, 0, 0, 0)
            };

        let usage = CodexUsage {
            primary_percent: primary.used_percent.map(|p| (p * 100.0).round() as u32),
            secondary_percent: rate_limits
                .secondary
                .as_ref()
                .and_then(|s| s.used_percent)
                .map(|p| (p * 100.0).round() as u32),
            primary_resets_at: primary
                .resets_at
                .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
                .map(|dt| dt.to_rfc3339()),
            secondary_resets_at: rate_limits
                .secondary
                .as_ref()
                .and_then(|s| s.resets_at)
                .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
                .map(|dt| dt.to_rfc3339()),
            plan_type: rate_limits.plan_type.clone(),
            input_tokens: input,
            cached_input_tokens: cached,
            output_tokens: output,
            reasoning_output_tokens: reasoning,
            total_tokens: total,
            timestamp: Some(timestamp.to_rfc3339()),
        };

        match &best {
            Some((existing_ts, _)) if timestamp > *existing_ts => {
                best = Some((timestamp, usage));
            }
            None => {
                best = Some((timestamp, usage));
            }
            _ => {}
        }
    }

    best
}
