use std::process::Command;

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

/// Read OAuth access token from "Claude Code-credentials" keychain entry.
fn get_bearer_token() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?.trim().to_string();
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
