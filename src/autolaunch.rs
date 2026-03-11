use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.claude-code-rate-watcher";

fn plist_path() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join("Library/LaunchAgents").join(format!("{}.plist", LABEL))
}

fn binary_path() -> String {
    let home = dirs::home_dir().expect("No home directory");
    home.join("Applications/claude-code-rate-watcher")
        .to_string_lossy()
        .into_owned()
}

pub fn is_enabled() -> bool {
    plist_path().exists()
}

pub fn enable() -> Result<(), String> {
    let plist = plist_path();
    let bin = binary_path();

    // Ensure LaunchAgents directory exists
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
        label = LABEL,
        bin = bin
    );

    fs::write(&plist, content).map_err(|e| e.to_string())?;

    Command::new("launchctl")
        .args(["load", &plist.to_string_lossy()])
        .output()
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn disable() -> Result<(), String> {
    let plist = plist_path();
    if !plist.exists() {
        return Ok(());
    }

    Command::new("launchctl")
        .args(["unload", &plist.to_string_lossy()])
        .output()
        .map_err(|e| e.to_string())?;

    fs::remove_file(&plist).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn toggle() -> Result<bool, String> {
    if is_enabled() {
        disable()?;
        Ok(false)
    } else {
        enable()?;
        Ok(true)
    }
}
