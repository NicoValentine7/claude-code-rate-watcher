use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

// TODO: Replace with your actual Supabase project URL
const SUPABASE_URL: &str = "https://YOUR_PROJECT.supabase.co";
const CALLBACK_PORT: u16 = 19532;
const CALLBACK_URL: &str = "http://localhost:19532/auth/callback";

/// Supabase session stored in Keychain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SupabaseSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // unix timestamp in seconds
    pub user_id: String,
    pub user_email: String,
    pub provider: String, // "github" or "google"
}

const KEYCHAIN_SERVICE: &str = "claude-rate-watcher-supabase";

/// Generate a cryptographically random code_verifier for PKCE.
fn generate_code_verifier() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Compute code_challenge = base64url(sha256(code_verifier)).
fn compute_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// Start the OAuth login flow in a background thread.
/// Returns a receiver that will receive the session on success.
pub fn start_login(
    provider: &str,
) -> mpsc::Receiver<Result<SupabaseSession, String>> {
    let provider = provider.to_string();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result = do_login(&provider);
        let _ = tx.send(result);
    });

    rx
}

fn do_login(provider: &str) -> Result<SupabaseSession, String> {
    let code_verifier = generate_code_verifier();
    let code_challenge = compute_code_challenge(&code_verifier);

    // Bind the callback server
    let listener = TcpListener::bind(format!("127.0.0.1:{}", CALLBACK_PORT))
        .map_err(|e| format!("Failed to bind callback port {}: {}", CALLBACK_PORT, e))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("Failed to set blocking mode: {}", e))?;

    // Open browser for OAuth
    let auth_url = format!(
        "{}/auth/v1/authorize?provider={}&redirect_to={}&code_challenge={}&code_challenge_method=S256",
        SUPABASE_URL, provider, CALLBACK_URL, code_challenge
    );
    let _ = Command::new("open").arg(&auth_url).spawn();

    // Wait for callback (timeout: 120s)
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Set nonblocking: {}", e))?;

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(120);
    let auth_code;

    loop {
        if start.elapsed() > timeout {
            return Err("Login timed out (120s)".to_string());
        }

        match listener.accept() {
            Ok((stream, _)) => {
                auth_code = handle_callback(stream)?;
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(format!("Accept failed: {}", e)),
        }
    }

    // Exchange code for tokens
    let session = exchange_code(&auth_code, &code_verifier)?;

    // Save to Keychain
    save_session(&session)?;

    eprintln!("[supabase_auth] Login successful: {}", session.user_email);
    Ok(session)
}

fn handle_callback(mut stream: std::net::TcpStream) -> Result<String, String> {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("Read callback: {}", e))?;

    // Parse: GET /auth/callback?code=XYZ HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or("Invalid HTTP request")?
        .to_string();

    let code = path
        .split('?')
        .nth(1)
        .ok_or("No query params in callback")?
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some("code"), Some(val)) => Some(val.to_string()),
                _ => None,
            }
        })
        .ok_or("No 'code' parameter in callback")?;

    // Send response HTML
    let body = r#"<!DOCTYPE html><html><body style="font-family:-apple-system,sans-serif;text-align:center;padding:60px;background:#1a1a1c;color:#fff">
<h2>Login Successful</h2><p style="color:rgba(255,255,255,0.5)">You can close this tab and return to the app.</p>
</body></html>"#;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());

    Ok(code)
}

fn exchange_code(code: &str, code_verifier: &str) -> Result<SupabaseSession, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );

    let url = format!("{}/auth/v1/token?grant_type=pkce", SUPABASE_URL);

    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .header("apikey", crate::supabase_client::SUPABASE_ANON_KEY_PUB)
        .send_json(&serde_json::json!({
            "auth_code": code,
            "code_verifier": code_verifier,
        }))
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let body: serde_json::Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Parse token response: {}", e))?;

    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access_token")?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or("No refresh_token")?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let user = body.get("user").ok_or("No user object")?;
    let user_id = user
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("No user.id")?
        .to_string();
    let user_email = user
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let provider = user
        .get("app_metadata")
        .and_then(|m| m.get("provider"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    Ok(SupabaseSession {
        access_token,
        refresh_token,
        expires_at: now_secs + expires_in,
        user_id,
        user_email,
        provider,
    })
}

/// Refresh the Supabase access token.
pub fn refresh_session(session: &SupabaseSession) -> Result<SupabaseSession, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );

    let url = format!("{}/auth/v1/token?grant_type=refresh_token", SUPABASE_URL);

    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .header("apikey", crate::supabase_client::SUPABASE_ANON_KEY_PUB)
        .send_json(&serde_json::json!({
            "refresh_token": session.refresh_token,
        }))
        .map_err(|e| format!("Refresh failed: {}", e))?;

    let body: serde_json::Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("Parse refresh response: {}", e))?;

    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access_token in refresh")?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or(&session.refresh_token)
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let new_session = SupabaseSession {
        access_token,
        refresh_token,
        expires_at: now_secs + expires_in,
        user_id: session.user_id.clone(),
        user_email: session.user_email.clone(),
        provider: session.provider.clone(),
    };

    save_session(&new_session)?;
    Ok(new_session)
}

/// Get stored session from Keychain, refreshing if expired.
pub fn get_session() -> Option<SupabaseSession> {
    let session = load_session()?;

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Refresh if expires within 5 minutes
    if session.expires_at < now_secs + 300 {
        eprintln!("[supabase_auth] Token expired, refreshing...");
        match refresh_session(&session) {
            Ok(new_session) => return Some(new_session),
            Err(e) => {
                eprintln!("[supabase_auth] Refresh failed: {}", e);
                return None;
            }
        }
    }

    Some(session)
}

/// Clear stored session (logout).
pub fn logout() {
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", KEYCHAIN_SERVICE])
        .output();
    eprintln!("[supabase_auth] Logged out");
}

fn save_session(session: &SupabaseSession) -> Result<(), String> {
    let json = serde_json::to_string(session)
        .map_err(|e| format!("Serialize session: {}", e))?;

    // Delete existing
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", KEYCHAIN_SERVICE])
        .output();

    // Add new
    let output = Command::new("security")
        .args([
            "add-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_SERVICE,
            "-w",
            &json,
        ])
        .output()
        .map_err(|e| format!("Keychain save: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Keychain save failed: {}", stderr));
    }

    Ok(())
}

fn load_session() -> Option<SupabaseSession> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?.trim().to_string();
    serde_json::from_str(&raw).ok()
}
