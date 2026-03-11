use crate::auth::{self, AuthCredential};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const USAGE_API_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const MESSAGES_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Rate limit data from the API.
#[derive(Debug, Clone, Default)]
pub struct ApiRateLimitData {
    /// 5-hour utilization percentage (0-100)
    pub five_hour_percent: Option<u32>,
    /// 7-day utilization percentage (0-100)
    pub seven_day_percent: Option<u32>,
    /// 5-hour reset time (ISO 8601 or unix timestamp)
    pub five_hour_resets_at: Option<String>,
    /// 7-day reset time
    pub seven_day_resets_at: Option<String>,
    /// Whether data came from the API (true) or unavailable (false)
    pub is_live: bool,
    /// Whether authentication credentials are missing
    pub auth_missing: bool,
}

#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
}

#[derive(Deserialize)]
struct UsageWindow {
    utilization: f64,
    resets_at: Option<String>,
}

/// Shared state for API rate limit data with caching.
pub struct ApiPoller {
    data: Arc<Mutex<ApiRateLimitData>>,
    last_fetch: Arc<Mutex<Option<Instant>>>,
}

impl ApiPoller {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(ApiRateLimitData::default())),
            last_fetch: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_data(&self) -> ApiRateLimitData {
        self.data.lock().unwrap().clone()
    }

    /// Fetch rate limit data from the API (called from a background thread).
    pub fn poll(&self) {
        // Check cache TTL
        {
            let last = self.last_fetch.lock().unwrap();
            if let Some(t) = *last {
                if t.elapsed() < CACHE_TTL {
                    return;
                }
            }
        }

        let credential = match auth::get_credential() {
            Some(c) => c,
            None => {
                let mut data = self.data.lock().unwrap();
                data.auth_missing = true;
                data.is_live = false;
                return;
            }
        };

        // Try /api/oauth/usage first (works with both Cookie and Bearer)
        if let Some(result) = try_usage_api(&credential) {
            *self.data.lock().unwrap() = result;
            *self.last_fetch.lock().unwrap() = Some(Instant::now());
            return;
        }

        // Fallback: Haiku probe (Bearer token only)
        if let AuthCredential::Bearer(ref token) = credential {
            if let Some(result) = try_haiku_probe(token) {
                *self.data.lock().unwrap() = result;
                *self.last_fetch.lock().unwrap() = Some(Instant::now());
            }
        }
    }
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build(),
    )
}

/// Call GET /api/oauth/usage
fn try_usage_api(credential: &AuthCredential) -> Option<ApiRateLimitData> {
    let agent = build_agent();

    let mut request = agent
        .get(USAGE_API_URL)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "claude-code-rate-watcher/0.1.0");

    request = match credential {
        AuthCredential::Bearer(token) => {
            request.header("Authorization", &format!("Bearer {}", token))
        }
        AuthCredential::Cookie(cookie) => request.header("Cookie", cookie),
    };

    let mut response = request.call().ok()?;

    let body: UsageResponse = response.body_mut().read_json().ok()?;

    Some(ApiRateLimitData {
        five_hour_percent: body.five_hour.as_ref().map(|w| w.utilization.round() as u32),
        seven_day_percent: body.seven_day.as_ref().map(|w| w.utilization.round() as u32),
        five_hour_resets_at: body.five_hour.as_ref().and_then(|w| w.resets_at.clone()),
        seven_day_resets_at: body.seven_day.as_ref().and_then(|w| w.resets_at.clone()),
        is_live: true,
        auth_missing: false,
    })
}

/// Haiku probe: send minimal request, read unified rate limit headers.
fn try_haiku_probe(bearer_token: &str) -> Option<ApiRateLimitData> {
    let agent = build_agent();

    let response = agent
        .post(MESSAGES_API_URL)
        .header("Authorization", &format!("Bearer {}", bearer_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "claude-code-rate-watcher/0.1.0")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .send_json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "h"}]
        }))
        .ok()?;

    let headers = response.headers();

    let get_header = |name: &str| -> Option<String> {
        headers.get(name).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
    };

    let h5_util: Option<u32> = get_header("anthropic-ratelimit-unified-5h-utilization")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v * 100.0).round() as u32);

    let h7_util: Option<u32> = get_header("anthropic-ratelimit-unified-7d-utilization")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v * 100.0).round() as u32);

    let h5_reset = get_header("anthropic-ratelimit-unified-5h-reset");
    let h7_reset = get_header("anthropic-ratelimit-unified-7d-reset");

    if h5_util.is_some() || h7_util.is_some() {
        Some(ApiRateLimitData {
            five_hour_percent: h5_util,
            seven_day_percent: h7_util,
            five_hour_resets_at: h5_reset,
            seven_day_resets_at: h7_reset,
            is_live: true,
            auth_missing: false,
        })
    } else {
        None
    }
}
