use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;
use serde::Deserialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_OWNER: &str = "NicoValentine7";
const GITHUB_REPO: &str = "claude-code-rate-watcher";
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600); // 6 hours
const ASSET_NAME: &str = "claude-code-rate-watcher-macos-universal.tar.gz";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub struct Updater {
    state: Arc<Mutex<UpdaterState>>,
}

struct UpdaterState {
    last_check: Option<Instant>,
    available: Option<UpdateInfo>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(UpdaterState {
                last_check: None,
                available: None,
            })),
        }
    }

    /// Check for updates (rate-limited to CHECK_INTERVAL).
    pub fn check(&self) -> Option<UpdateInfo> {
        let mut state = self.state.lock().unwrap();
        if let Some(last) = state.last_check {
            if last.elapsed() < CHECK_INTERVAL {
                return state.available.clone();
            }
        }
        state.last_check = Some(Instant::now());

        match check_github_release() {
            Ok(info) => {
                state.available = info.clone();
                info
            }
            Err(_) => state.available.clone(),
        }
    }

    pub fn get_available(&self) -> Option<UpdateInfo> {
        self.state.lock().unwrap().available.clone()
    }

    /// Download, extract, replace binary, and restart.
    pub fn apply_update(info: &UpdateInfo) -> Result<(), String> {
        let binary = current_binary_path()?;
        let tarball = download_tarball(&info.download_url)?;
        let new_binary = extract_binary(&tarball)?;

        // Replace the current binary
        let backup = binary.with_extension("bak");
        fs::rename(&binary, &backup).map_err(|e| format!("backup failed: {}", e))?;

        match fs::rename(&new_binary, &binary) {
            Ok(_) => {
                // Set executable permission
                let perms = fs::Permissions::from_mode(0o755);
                let _ = fs::set_permissions(&binary, perms);
                // Remove backup
                let _ = fs::remove_file(&backup);
            }
            Err(e) => {
                // Restore backup
                let _ = fs::rename(&backup, &binary);
                return Err(format!("replace failed: {}", e));
            }
        }

        // Restart: exec the new binary
        restart(&binary);
    }
}

fn check_github_release() -> Result<Option<UpdateInfo>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_OWNER, GITHUB_REPO
    );

    let response: GitHubRelease = ureq::get(&url)
        .header("User-Agent", "claude-code-rate-watcher")
        .header("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;

    let remote_tag = response.tag_name.trim_start_matches('v');
    let remote_ver = semver::Version::parse(remote_tag).map_err(|e| e.to_string())?;
    let current_ver = semver::Version::parse(CURRENT_VERSION).map_err(|e| e.to_string())?;

    if remote_ver > current_ver {
        if let Some(asset) = response.assets.iter().find(|a| a.name == ASSET_NAME) {
            return Ok(Some(UpdateInfo {
                version: remote_tag.to_string(),
                download_url: asset.browser_download_url.clone(),
            }));
        }
    }

    Ok(None)
}

/// Detect if running from a Homebrew Cellar install.
pub fn is_homebrew_install() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| {
            let s = p.to_string_lossy();
            s.contains("/Cellar/") || s.contains("/homebrew/")
        })
        .unwrap_or(false)
}

fn current_binary_path() -> Result<PathBuf, String> {
    if is_homebrew_install() {
        return Err(
            "Auto-update disabled for Homebrew installs. Use `brew upgrade NicoValentine7/tap/claude-code-rate-watcher`."
                .to_string(),
        );
    }
    std::env::current_exe().map_err(|e| format!("cannot find current exe: {}", e))
}

fn download_tarball(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .header("User-Agent", "claude-code-rate-watcher")
        .call()
        .map_err(|e| format!("download failed: {}", e))?;
    let mut buf = Vec::new();
    response
        .into_body()
        .as_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {}", e))?;
    Ok(buf)
}

fn extract_binary(tarball: &[u8]) -> Result<PathBuf, String> {
    let decoder = GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(decoder);
    let tmp_dir =
        std::env::temp_dir().join(format!("crw-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    archive.unpack(&tmp_dir).map_err(|e| format!("extract failed: {}", e))?;

    // Try new name first, then legacy name for backward compatibility
    let binary = tmp_dir.join("ccrw");
    if binary.exists() {
        return Ok(binary);
    }
    let legacy = tmp_dir.join("claude-code-rate-watcher");
    if legacy.exists() {
        Ok(legacy)
    } else {
        Err("binary not found in tarball".to_string())
    }
}

fn restart(binary: &PathBuf) -> ! {
    restart_app(binary);
}

pub fn restart_app(binary: &PathBuf) -> ! {
    // If running from .app bundle, use `open` to relaunch so macOS sets up
    // the proper NSApplication context (required for menu bar tray icon).
    if let Some(app_path) = find_parent_app_bundle(binary) {
        let _ = std::process::Command::new("open")
            .args(["-a", &app_path.to_string_lossy()])
            .spawn();
        std::process::exit(0);
    }

    // Terminal launch: exec directly
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(binary).exec();
    eprintln!("restart failed: {}", err);
    std::process::exit(1);
}

/// Walk up the path to find a `.app` bundle parent directory.
fn find_parent_app_bundle(binary: &PathBuf) -> Option<PathBuf> {
    let mut path = binary.as_path();
    while let Some(parent) = path.parent() {
        if parent.extension().is_some_and(|ext| ext == "app") {
            return Some(parent.to_path_buf());
        }
        path = parent;
    }
    None
}
