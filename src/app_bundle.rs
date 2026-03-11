use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const APP_NAME: &str = "Claude Code Rate Watcher";
const BUNDLE_ID: &str = "com.claude-code-rate-watcher";
const APP_ICON: &[u8] = include_bytes!("../assets/AppIcon.icns");

fn app_path() -> PathBuf {
    PathBuf::from(format!("/Applications/{}.app", APP_NAME))
}

/// Create or update the .app bundle in /Applications.
/// The .app contains a launcher script that execs the current binary.
/// This makes the app launchable from Spotlight, Launchpad, and Finder.
pub fn ensure_app_bundle() {
    let app = app_path();
    let macos_dir = app.join("Contents/MacOS");
    let plist_path = app.join("Contents/Info.plist");
    let launcher_path = macos_dir.join("launcher");

    let current_bin = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => return,
    };
    let version = env!("CARGO_PKG_VERSION");

    // Check if .app already exists with current version
    if plist_path.exists() {
        if let Ok(content) = fs::read_to_string(&plist_path) {
            if content.contains(&format!("<string>{}</string>", version)) {
                // Also verify launcher points to current binary
                if let Ok(script) = fs::read_to_string(&launcher_path) {
                    if script.contains(&current_bin) {
                        return; // Already up to date
                    }
                }
            }
        }
    }

    // Create .app bundle
    if let Err(e) = create_app_bundle(&app, &macos_dir, &plist_path, &launcher_path, &current_bin, version) {
        eprintln!("Failed to create .app bundle: {}", e);
    }
}

fn create_app_bundle(
    app: &PathBuf,
    macos_dir: &PathBuf,
    plist_path: &PathBuf,
    launcher_path: &PathBuf,
    binary: &str,
    version: &str,
) -> Result<(), String> {
    // Remove old .app if exists
    if app.exists() {
        fs::remove_dir_all(app).map_err(|e| e.to_string())?;
    }

    let resources_dir = app.join("Contents/Resources");
    fs::create_dir_all(macos_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&resources_dir).map_err(|e| e.to_string())?;

    // App icon
    fs::write(resources_dir.join("AppIcon.icns"), APP_ICON).map_err(|e| e.to_string())?;

    // Info.plist
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>{name}</string>
  <key>CFBundleDisplayName</key>
  <string>{name}</string>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>CFBundleShortVersionString</key>
  <string>{version}</string>
  <key>CFBundleExecutable</key>
  <string>launcher</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>LSUIElement</key>
  <true/>
</dict>
</plist>
"#,
        name = APP_NAME,
        bundle_id = BUNDLE_ID,
        version = version,
    );
    fs::write(plist_path, plist).map_err(|e| e.to_string())?;

    // Launcher script
    let script = format!(
        "#!/bin/bash\nexec \"{}\" \"$@\"\n",
        binary
    );
    fs::write(launcher_path, script).map_err(|e| e.to_string())?;
    fs::set_permissions(launcher_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    Ok(())
}
