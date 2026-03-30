use std::process::Command;
use std::time::Duration;

const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const OAUTH_CLIENT_ID: &str = "5d15e876-9096-4ddf-b647-3fdb21aed944";

/// Authentication credentials for Anthropic API.
pub enum AuthCredential {
    /// Cookie-based auth (from cookie.claude keychain entry)
    Cookie(String),
    /// Bearer token auth (from Claude Code-credentials keychain entry)
    Bearer(String),
}

/// Try to obtain authentication credentials from macOS Keychain.
/// Priority: 1) Claude Code-credentials (Bearer), 2) cookie.claude (Cookie)
pub fn get_credential() -> Option<AuthCredential> {
    // Try Bearer token first (from `claude setup-token` or normal OAuth flow)
    if let Some(token) = get_bearer_token() {
        return Some(AuthCredential::Bearer(token));
    }

    // Fallback: cookie-based auth from Claude Desktop
    if let Some(cookie) = get_cookie_header() {
        return Some(AuthCredential::Cookie(cookie));
    }

    None
}

/// Check if the stored access token is expired (or about to expire in 5 minutes).
pub fn is_token_expired() -> bool {
    let raw = match read_keychain_entry() {
        Some(r) => r,
        None => return true,
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let expires_at = parsed
        .get("claudeAiOauth")
        .and_then(|o| o.get("expiresAt"))
        .and_then(|v| v.as_i64());

    match expires_at {
        Some(ms) => {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            // Expired or expires within 5 minutes
            ms < now_ms + 300_000
        }
        None => true,
    }
}

/// Refresh the OAuth token using the stored refresh token.
/// Returns Ok(new_access_token) on success, Err(message) on failure.
pub fn refresh_token() -> Result<String, String> {
    let raw = read_keychain_entry().ok_or("No keychain entry found")?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Parse error: {}", e))?;

    let oauth = parsed
        .get("claudeAiOauth")
        .ok_or("No claudeAiOauth field")?;
    let refresh_tok = oauth
        .get("refreshToken")
        .and_then(|v| v.as_str())
        .ok_or("No refreshToken found")?;

    eprintln!("[auth] Refreshing OAuth token...");

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );

    let mut response = agent
        .post(OAUTH_TOKEN_URL)
        .header("Content-Type", "application/json")
        .send_json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_tok,
            "client_id": OAUTH_CLIENT_ID,
        }))
        .map_err(|e| format!("Refresh request failed: {}", e))?;

    let body: serde_json::Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Parse refresh response: {}", e))?;

    let new_access = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access_token in response")?
        .to_string();
    let new_refresh = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let new_expires_in = body.get("expires_in").and_then(|v| v.as_i64());

    // Build updated keychain entry
    let mut full: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let oauth_obj = full.get_mut("claudeAiOauth").unwrap();
    oauth_obj["accessToken"] = serde_json::Value::String(new_access.clone());
    if let Some(rt) = new_refresh {
        oauth_obj["refreshToken"] = serde_json::Value::String(rt);
    }
    if let Some(secs) = new_expires_in {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        oauth_obj["expiresAt"] = serde_json::Value::Number((now_ms + secs * 1000).into());
    }

    // Save back to keychain
    save_keychain_entry(&serde_json::to_string(&full).unwrap())?;

    eprintln!("[auth] Token refreshed successfully");
    Ok(new_access)
}

/// Read OAuth access token from "Claude Code-credentials" keychain entry.
fn get_bearer_token() -> Option<String> {
    let raw = read_keychain_entry()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .map(|s| s.to_string())
}

/// Read session cookie from "cookie.claude" keychain entry.
fn get_cookie_header() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-a", "cookie.claude", "-w"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?.trim().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("cookieHeader")?
        .as_str()
        .map(|s| s.to_string())
}

fn read_keychain_entry() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

/// Resolve the full path to the `claude` CLI binary.
/// GUI apps on macOS don't inherit shell PATH (~/.zshrc),
/// so we search common install locations explicitly.
pub fn find_claude_binary() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".local/bin/claude"),
        home.join(".claude/local/bin/claude"),
        std::path::PathBuf::from("/opt/homebrew/bin/claude"),
        std::path::PathBuf::from("/usr/local/bin/claude"),
    ];
    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }
    // Last resort: bare "claude" — works if launched from terminal with PATH set
    None
}

fn save_keychain_entry(value: &str) -> Result<(), String> {
    // Delete existing entry first
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", "Claude Code-credentials"])
        .output();

    // Add new entry
    let output = Command::new("security")
        .args([
            "add-generic-password",
            "-s",
            "Claude Code-credentials",
            "-a",
            "Claude Code-credentials",
            "-w",
            value,
        ])
        .output()
        .map_err(|e| format!("Failed to run security command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Keychain save failed: {}", stderr));
    }

    Ok(())
}
