use crate::auth::{self, AuthCredential};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const USAGE_API_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const MESSAGES_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CACHE_TTL: Duration = Duration::from_secs(60);
const CACHE_TTL_RATE_LIMITED: Duration = Duration::from_secs(300);

/// Rate limit data from the API.
#[derive(Debug, Clone, Default, Serialize)]
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
    /// Short error message for UI display
    pub error_message: Option<String>,
    /// Technical error detail (HTTP status, response body)
    pub error_detail: Option<String>,
    /// Consecutive failure count
    pub retry_count: u32,
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

/// Error info from an API call attempt.
struct ApiError {
    message: String,
    detail: String,
    is_auth_error: bool,
    is_rate_limited: bool,
}

/// Shared state for API rate limit data with caching.
pub struct ApiPoller {
    data: Arc<Mutex<ApiRateLimitData>>,
    last_fetch: Arc<Mutex<Option<Instant>>>,
    cache_ttl: Arc<Mutex<Duration>>,
}

impl ApiPoller {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(ApiRateLimitData::default())),
            last_fetch: Arc::new(Mutex::new(None)),
            cache_ttl: Arc::new(Mutex::new(CACHE_TTL)),
        }
    }

    pub fn get_data(&self) -> ApiRateLimitData {
        self.data.lock().unwrap().clone()
    }

    /// Fetch rate limit data from the API.
    pub fn poll(&self) {
        // Check cache TTL
        {
            let last = self.last_fetch.lock().unwrap();
            let ttl = *self.cache_ttl.lock().unwrap();
            if let Some(t) = *last {
                if t.elapsed() < ttl {
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
                data.error_message = Some("No credentials found".into());
                eprintln!("[api] No credentials found in keychain");
                return;
            }
        };

        // Check if token is expired before making API calls
        if let AuthCredential::Bearer(_) = &credential {
            if auth::is_token_expired() {
                eprintln!("[api] Token expired, attempting refresh...");
                match auth::refresh_token() {
                    Ok(_) => {
                        eprintln!("[api] Token refreshed, retrying with new credential");
                        // Re-read credential after refresh
                        if let Some(new_cred) = auth::get_credential() {
                            self.try_fetch(&new_cred);
                            return;
                        }
                    }
                    Err(e) => {
                        eprintln!("[api] Token refresh failed: {}", e);
                        let mut data = self.data.lock().unwrap();
                        data.auth_missing = true;
                        data.is_live = false;
                        data.error_message = Some("Token refresh failed".into());
                        data.error_detail = Some(e);
                        data.retry_count += 1;
                        return;
                    }
                }
            }
        }

        self.try_fetch(&credential);
    }

    fn try_fetch(&self, credential: &AuthCredential) {
        // Try /api/oauth/usage first
        match try_usage_api(credential) {
            Ok(result) => {
                *self.data.lock().unwrap() = result;
                *self.last_fetch.lock().unwrap() = Some(Instant::now());
                *self.cache_ttl.lock().unwrap() = CACHE_TTL;
                return;
            }
            Err(err) => {
                eprintln!("[api] /api/oauth/usage failed: {} — {}", err.message, err.detail);

                if err.is_auth_error {
                    // Try refresh and retry once
                    if let AuthCredential::Bearer(_) = credential {
                        if let Ok(_) = auth::refresh_token() {
                            if let Some(new_cred) = auth::get_credential() {
                                if let Ok(result) = try_usage_api(&new_cred) {
                                    *self.data.lock().unwrap() = result;
                                    *self.last_fetch.lock().unwrap() = Some(Instant::now());
                                    *self.cache_ttl.lock().unwrap() = CACHE_TTL;
                                    return;
                                }
                            }
                        }
                    }
                }

                if err.is_rate_limited {
                    let mut data = self.data.lock().unwrap();
                    data.is_live = false;
                    data.error_message = Some("Rate limited (429)".into());
                    data.error_detail = Some(err.detail);
                    data.retry_count += 1;
                    *self.last_fetch.lock().unwrap() = Some(Instant::now());
                    *self.cache_ttl.lock().unwrap() = CACHE_TTL_RATE_LIMITED;
                    return;
                }
            }
        }

        // Fallback: Haiku probe (Bearer token only)
        if let AuthCredential::Bearer(token) = credential {
            match try_haiku_probe(token) {
                Ok(result) => {
                    *self.data.lock().unwrap() = result;
                    *self.last_fetch.lock().unwrap() = Some(Instant::now());
                    *self.cache_ttl.lock().unwrap() = CACHE_TTL;
                    return;
                }
                Err(err) => {
                    eprintln!("[api] Haiku probe failed: {} — {}", err.message, err.detail);

                    if err.is_auth_error {
                        // Last resort: try refresh
                        match auth::refresh_token() {
                            Ok(_) => {
                                if let Some(AuthCredential::Bearer(new_token)) =
                                    auth::get_credential()
                                {
                                    if let Ok(result) = try_haiku_probe(&new_token) {
                                        *self.data.lock().unwrap() = result;
                                        *self.last_fetch.lock().unwrap() = Some(Instant::now());
                                        *self.cache_ttl.lock().unwrap() = CACHE_TTL;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[api] Refresh failed after Haiku 401: {}", e);
                                let mut data = self.data.lock().unwrap();
                                data.auth_missing = true;
                                data.is_live = false;
                                data.error_message = Some("Login required".into());
                                data.error_detail = Some(format!("Refresh failed: {}", e));
                                data.retry_count += 1;
                                return;
                            }
                        }
                    }

                    // Both methods failed with non-auth error
                    let mut data = self.data.lock().unwrap();
                    data.is_live = false;
                    data.error_message = Some(err.message);
                    data.error_detail = Some(err.detail);
                    data.retry_count += 1;
                    *self.last_fetch.lock().unwrap() = Some(Instant::now());
                }
            }
        } else {
            // Cookie-based auth, usage API failed, no Haiku fallback
            let mut data = self.data.lock().unwrap();
            data.is_live = false;
            data.error_message = Some("API unavailable".into());
            data.retry_count += 1;
            *self.last_fetch.lock().unwrap() = Some(Instant::now());
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
fn try_usage_api(credential: &AuthCredential) -> Result<ApiRateLimitData, ApiError> {
    let agent = build_agent();

    let mut request = agent
        .get(USAGE_API_URL)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "claude-code-rate-watcher/0.2.0");

    request = match credential {
        AuthCredential::Bearer(token) => {
            request.header("Authorization", &format!("Bearer {}", token))
        }
        AuthCredential::Cookie(cookie) => request.header("Cookie", cookie),
    };

    let mut response = request.call().map_err(|e| {
        let (msg, detail, is_auth, is_rate) = classify_ureq_error(&e);
        ApiError {
            message: msg,
            detail,
            is_auth_error: is_auth,
            is_rate_limited: is_rate,
        }
    })?;

    let body: UsageResponse = response.body_mut().read_json().map_err(|e| ApiError {
        message: "Invalid response".into(),
        detail: format!("JSON parse error: {}", e),
        is_auth_error: false,
        is_rate_limited: false,
    })?;

    Ok(ApiRateLimitData {
        five_hour_percent: body.five_hour.as_ref().map(|w| w.utilization.round() as u32),
        seven_day_percent: body.seven_day.as_ref().map(|w| w.utilization.round() as u32),
        five_hour_resets_at: body.five_hour.as_ref().and_then(|w| w.resets_at.clone()),
        seven_day_resets_at: body.seven_day.as_ref().and_then(|w| w.resets_at.clone()),
        is_live: true,
        auth_missing: false,
        error_message: None,
        error_detail: None,
        retry_count: 0,
    })
}

/// Haiku probe: send minimal request, read unified rate limit headers.
fn try_haiku_probe(bearer_token: &str) -> Result<ApiRateLimitData, ApiError> {
    let agent = build_agent();

    let response = agent
        .post(MESSAGES_API_URL)
        .header("Authorization", &format!("Bearer {}", bearer_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "claude-code-rate-watcher/0.2.0")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .send_json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "h"}]
        }))
        .map_err(|e| {
            let (msg, detail, is_auth, is_rate) = classify_ureq_error(&e);
            ApiError {
                message: msg,
                detail,
                is_auth_error: is_auth,
                is_rate_limited: is_rate,
            }
        })?;

    let headers = response.headers();

    let get_header = |name: &str| -> Option<String> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
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
        Ok(ApiRateLimitData {
            five_hour_percent: h5_util,
            seven_day_percent: h7_util,
            five_hour_resets_at: h5_reset,
            seven_day_resets_at: h7_reset,
            is_live: true,
            auth_missing: false,
            error_message: None,
            error_detail: None,
            retry_count: 0,
        })
    } else {
        Err(ApiError {
            message: "No rate limit headers".into(),
            detail: "Haiku probe succeeded but no unified rate limit headers found".into(),
            is_auth_error: false,
            is_rate_limited: false,
        })
    }
}

fn classify_ureq_error(err: &ureq::Error) -> (String, String, bool, bool) {
    match err {
        ureq::Error::StatusCode(status) => {
            let code = *status;
            let is_auth = code == 401 || code == 403;
            let is_rate = code == 429;
            let msg = if is_auth {
                "Authentication error".to_string()
            } else if is_rate {
                "Rate limited (429)".to_string()
            } else {
                format!("HTTP {}", code)
            };
            let detail = format!("HTTP {} from API", code);
            (msg, detail, is_auth, is_rate)
        }
        _ => {
            let detail = format!("{}", err);
            ("Network error".to_string(), detail, false, false)
        }
    }
}
