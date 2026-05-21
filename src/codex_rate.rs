use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_SESSION_FILES_TO_SCAN: usize = 50;
const LIVE_THRESHOLD_MINUTES: i64 = 10;

#[derive(Debug, Clone, Serialize)]
pub struct CodexRateLimitData {
    pub five_hour_percent: Option<u32>,
    pub seven_day_percent: Option<u32>,
    pub five_hour_resets_at: Option<String>,
    pub seven_day_resets_at: Option<String>,
    pub plan_label: Option<String>,
    pub is_live: bool,
    pub last_updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexJournalEntry {
    timestamp: Option<String>,
    payload: Option<CodexPayload>,
}

#[derive(Debug, Deserialize)]
struct CodexPayload {
    rate_limits: Option<CodexRateLimits>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexRateLimits {
    limit_id: Option<String>,
    primary: Option<CodexWindow>,
    secondary: Option<CodexWindow>,
    plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexWindow {
    used_percent: Option<f64>,
    window_minutes: Option<u64>,
    resets_at: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct SessionFile {
    path: PathBuf,
    modified: SystemTime,
    len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSignature {
    path: PathBuf,
    modified: SystemTime,
    len: u64,
}

#[derive(Debug, Clone)]
struct RateLimitSnapshot {
    timestamp: DateTime<Utc>,
    rate_limits: CodexRateLimits,
}

#[derive(Debug, Default)]
pub struct CodexRateCache {
    last_signature: Option<Vec<FileSignature>>,
    latest_snapshot: Option<RateLimitSnapshot>,
}

impl CodexRateCache {
    pub fn load_latest(&mut self) -> Option<CodexRateLimitData> {
        let files = collect_candidate_session_files()?;
        self.load_latest_from_session_files(files)
    }

    fn load_latest_from_session_files(
        &mut self,
        files: Vec<SessionFile>,
    ) -> Option<CodexRateLimitData> {
        let signature = files
            .iter()
            .map(|file| FileSignature {
                path: file.path.clone(),
                modified: file.modified,
                len: file.len,
            })
            .collect::<Vec<_>>();

        if self.last_signature.as_ref() != Some(&signature) {
            self.latest_snapshot = files
                .iter()
                .filter_map(|file| parse_session_file(&file.path))
                .max_by_key(|snapshot| snapshot.timestamp);
            self.last_signature = Some(signature);
        }

        self.latest_snapshot.clone().map(to_rate_limit_data)
    }
}

fn collect_candidate_session_files() -> Option<Vec<SessionFile>> {
    let home = dirs::home_dir()?;
    let mut files = Vec::new();

    collect_session_files(
        &home.join(".codex").join("sessions").join("**/*.jsonl"),
        &mut files,
    );
    collect_session_files(
        &home
            .join(".codex")
            .join("archived_sessions")
            .join("*.jsonl"),
        &mut files,
    );

    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    files.truncate(MAX_SESSION_FILES_TO_SCAN);

    Some(files)
}

fn collect_session_files(pattern_path: &Path, files: &mut Vec<SessionFile>) {
    let Some(pattern) = pattern_path.to_str() else {
        return;
    };

    let Ok(entries) = glob::glob(pattern) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(metadata) = std::fs::metadata(&entry) else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        files.push(SessionFile {
            path: entry,
            modified,
            len: metadata.len(),
        });
    }
}

fn parse_session_file(path: &Path) -> Option<RateLimitSnapshot> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut latest: Option<RateLimitSnapshot> = None;

    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<CodexJournalEntry>(&line) else {
            continue;
        };
        let Some(rate_limits) = entry.payload.and_then(|payload| payload.rate_limits) else {
            continue;
        };
        if rate_limits
            .limit_id
            .as_deref()
            .is_some_and(|id| !id.eq_ignore_ascii_case("codex"))
        {
            continue;
        }

        let Some(timestamp) = entry
            .timestamp
            .as_deref()
            .and_then(|ts| ts.parse::<DateTime<Utc>>().ok())
        else {
            continue;
        };

        let snapshot = RateLimitSnapshot {
            timestamp,
            rate_limits,
        };
        if latest
            .as_ref()
            .is_none_or(|existing| snapshot.timestamp > existing.timestamp)
        {
            latest = Some(snapshot);
        }
    }

    latest
}

fn to_rate_limit_data(snapshot: RateLimitSnapshot) -> CodexRateLimitData {
    let mut five_hour = None;
    let mut seven_day = None;

    assign_window(
        &snapshot.rate_limits.primary,
        true,
        &mut five_hour,
        &mut seven_day,
    );
    assign_window(
        &snapshot.rate_limits.secondary,
        false,
        &mut five_hour,
        &mut seven_day,
    );

    let now = Utc::now();
    let is_live =
        now.signed_duration_since(snapshot.timestamp).num_minutes() <= LIVE_THRESHOLD_MINUTES;

    CodexRateLimitData {
        five_hour_percent: five_hour.and_then(|w| percent_for_window(w, now)),
        seven_day_percent: seven_day.and_then(|w| percent_for_window(w, now)),
        five_hour_resets_at: five_hour.and_then(|w| resets_at_to_rfc3339(&w.resets_at)),
        seven_day_resets_at: seven_day.and_then(|w| resets_at_to_rfc3339(&w.resets_at)),
        plan_label: plan_label(snapshot.rate_limits.plan_type.as_deref()),
        is_live,
        last_updated_at: Some(snapshot.timestamp.to_rfc3339()),
    }
}

fn assign_window<'a>(
    window: &'a Option<CodexWindow>,
    primary_fallback: bool,
    five_hour: &mut Option<&'a CodexWindow>,
    seven_day: &mut Option<&'a CodexWindow>,
) {
    let Some(window) = window else {
        return;
    };

    match window.window_minutes {
        Some(300) => *five_hour = Some(window),
        Some(10_080) => *seven_day = Some(window),
        _ if primary_fallback => *five_hour = Some(window),
        _ => *seven_day = Some(window),
    }
}

fn percent(value: Option<f64>) -> Option<u32> {
    value.map(|pct| pct.round().clamp(0.0, 100.0) as u32)
}

fn percent_for_window(window: &CodexWindow, now: DateTime<Utc>) -> Option<u32> {
    if reset_at_datetime(&window.resets_at).is_some_and(|reset| reset <= now) {
        return Some(0);
    }
    percent(window.used_percent)
}

fn reset_at_datetime(value: &Option<serde_json::Value>) -> Option<DateTime<Utc>> {
    match value.as_ref()? {
        serde_json::Value::Number(n) => n
            .as_i64()
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        serde_json::Value::String(s) => {
            if let Ok(epoch) = s.parse::<i64>() {
                DateTime::from_timestamp(epoch, 0)
            } else {
                s.parse::<DateTime<Utc>>().ok()
            }
        }
        _ => None,
    }
}

fn resets_at_to_rfc3339(value: &Option<serde_json::Value>) -> Option<String> {
    match value.as_ref()? {
        serde_json::Value::String(s) => reset_at_datetime(value)
            .map(|dt| dt.to_rfc3339())
            .or_else(|| Some(s.clone())),
        _ => reset_at_datetime(value).map(|dt| dt.to_rfc3339()),
    }
}

fn plan_label(plan_type: Option<&str>) -> Option<String> {
    let plan = plan_type?;
    let normalized = plan.to_ascii_lowercase();
    let label = match normalized.as_str() {
        "prolite" => "Pro Lite",
        "pro" => "Pro",
        "plus" => "Plus",
        "team" => "Team",
        "enterprise" => "Enterprise",
        other => other,
    };
    Some(label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_session_file_extracts_latest_codex_rate_limits() {
        let path =
            std::env::temp_dir().join(format!("ccrw-codex-rate-test-{}.jsonl", std::process::id()));
        let mut file = File::create(&path).unwrap();
        let future_reset = (Utc::now() + chrono::TimeDelta::hours(1)).timestamp();
        writeln!(
            file,
            r#"{{"timestamp":"2026-05-20T09:49:00Z","payload":{{"rate_limits":{{"limit_id":"codex","primary":{{"used_percent":20.0,"window_minutes":300,"resets_at":{future_reset}}},"secondary":{{"used_percent":3.0,"window_minutes":10080,"resets_at":{future_reset}}},"plan_type":"prolite"}}}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-05-20T09:50:00Z","payload":{{"rate_limits":{{"limit_id":"codex","primary":{{"used_percent":21.0,"window_minutes":300,"resets_at":{future_reset}}},"secondary":{{"used_percent":4.0,"window_minutes":10080,"resets_at":{future_reset}}},"plan_type":"prolite"}}}}}}"#
        )
        .unwrap();

        let snapshot = parse_session_file(&path).unwrap();
        let data = to_rate_limit_data(snapshot);

        assert_eq!(data.five_hour_percent, Some(21));
        assert_eq!(data.seven_day_percent, Some(4));
        assert_eq!(data.plan_label.as_deref(), Some("Pro Lite"));
        assert!(data.five_hour_resets_at.is_some());
        assert!(data.seven_day_resets_at.is_some());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn expired_reset_window_displays_zero_percent() {
        let snapshot = RateLimitSnapshot {
            timestamp: Utc::now(),
            rate_limits: CodexRateLimits {
                limit_id: Some("codex".to_string()),
                primary: Some(CodexWindow {
                    used_percent: Some(70.0),
                    window_minutes: Some(300),
                    resets_at: Some(serde_json::json!(1)),
                }),
                secondary: None,
                plan_type: Some("prolite".to_string()),
            },
        };

        let data = to_rate_limit_data(snapshot);

        assert_eq!(data.five_hour_percent, Some(0));
        assert!(data.five_hour_resets_at.is_some());
    }

    #[test]
    fn cache_reuses_snapshot_until_file_signature_changes() {
        let path = std::env::temp_dir().join(format!(
            "ccrw-codex-cache-test-{}.jsonl",
            std::process::id()
        ));
        let future_reset = (Utc::now() + chrono::TimeDelta::hours(1)).timestamp();

        write_session_file(&path, 20.0, future_reset, false);
        let original_signature = session_file(&path);

        let mut cache = CodexRateCache::default();
        let first = cache
            .load_latest_from_session_files(vec![original_signature.clone()])
            .unwrap();
        assert_eq!(first.five_hour_percent, Some(20));

        write_session_file(&path, 30.0, future_reset, true);

        let cached = cache
            .load_latest_from_session_files(vec![original_signature])
            .unwrap();
        assert_eq!(cached.five_hour_percent, Some(20));

        let reparsed = cache
            .load_latest_from_session_files(vec![session_file(&path)])
            .unwrap();
        assert_eq!(reparsed.five_hour_percent, Some(30));

        let _ = std::fs::remove_file(path);
    }

    fn write_session_file(path: &Path, used_percent: f64, future_reset: i64, pad: bool) {
        let mut content = format!(
            r#"{{"timestamp":"2026-05-20T09:50:00Z","payload":{{"rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{used_percent},"window_minutes":300,"resets_at":{future_reset}}},"secondary":{{"used_percent":4.0,"window_minutes":10080,"resets_at":{future_reset}}},"plan_type":"prolite"}}}}}}"#
        );
        content.push('\n');
        if pad {
            content.push('\n');
        }
        std::fs::write(path, content).unwrap();
    }

    fn session_file(path: &Path) -> SessionFile {
        let metadata = std::fs::metadata(path).unwrap();
        SessionFile {
            path: path.to_path_buf(),
            modified: metadata.modified().unwrap(),
            len: metadata.len(),
        }
    }
}
