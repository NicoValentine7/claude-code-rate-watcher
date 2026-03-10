use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct JournalEntry {
    #[serde(rename = "type")]
    entry_type: String,
    timestamp: Option<String>,
    message: Option<MessageData>,
}

#[derive(Debug, Deserialize)]
struct MessageData {
    id: Option<String>,
    role: Option<String>,
    usage: Option<UsageData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsageData {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub usage: UsageData,
    pub message_id: String,
}

pub fn parse_session_file(path: &Path) -> Vec<UsageRecord> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);
    let mut records: Vec<UsageRecord> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.is_empty() {
            continue;
        }

        let entry: JournalEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.entry_type != "assistant" {
            continue;
        }

        let message = match entry.message {
            Some(m) => m,
            None => continue,
        };

        if message.role.as_deref() != Some("assistant") {
            continue;
        }

        let usage = match message.usage {
            Some(u) => u,
            None => continue,
        };

        let message_id = match message.id {
            Some(id) => id,
            None => continue,
        };

        let timestamp = match entry.timestamp {
            Some(ref ts) => match ts.parse::<DateTime<Utc>>() {
                Ok(dt) => dt,
                Err(_) => continue,
            },
            None => continue,
        };

        records.push(UsageRecord {
            timestamp,
            usage,
            message_id,
        });
    }

    deduplicate_by_message_id(records)
}

fn deduplicate_by_message_id(records: Vec<UsageRecord>) -> Vec<UsageRecord> {
    let mut best: HashMap<String, UsageRecord> = HashMap::new();

    for record in records {
        let key = record.message_id.clone();
        match best.get(&key) {
            Some(existing) => {
                if record.usage.output_tokens > existing.usage.output_tokens {
                    best.insert(key, record);
                }
            }
            None => {
                best.insert(key, record);
            }
        }
    }

    best.into_values().collect()
}

pub fn load_all_sessions() -> Vec<UsageRecord> {
    let projects_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    let pattern = match projects_dir.join("**/*.jsonl").to_str() {
        Some(p) => p.to_string(),
        None => return Vec::new(),
    };

    let mut all_records = Vec::new();

    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            let records = parse_session_file(&entry);
            all_records.extend(records);
        }
    }

    all_records
}
