# Auto-Launch & Online Updater Design

## 1. Login Auto-Launch

### Mechanism
- LaunchAgent plist at `~/Library/LaunchAgents/com.claude-code-rate-watcher.plist`
- Binary path: `~/Applications/claude-code-rate-watcher`

### Module: `src/autolaunch.rs`
- `is_enabled() -> bool` — check plist existence
- `enable(binary_path: &str)` — write plist, `launchctl load`
- `disable()` — `launchctl unload`, delete plist

### UI Integration
- Toggle in popover.html footer
- IPC command: `toggle_launch_at_login`
- State derived from plist file existence

## 2. Online Auto-Updater

### Mechanism
- Poll GitHub Releases API for latest version
- Compare with compiled-in version using semver
- Download universal binary tarball, replace, restart

### Module: `src/updater.rs`
- `check_update() -> Option<UpdateInfo>` — GET GitHub API, compare versions
- `download_and_apply(info: &UpdateInfo)` — download tarball, extract, replace binary, exec restart
- Check interval: on startup + every 6 hours
- GitHub API: `GET https://api.github.com/repos/{owner}/{repo}/releases/latest`

### Update Flow
1. Background thread checks for updates
2. New version found → send `UserEvent::UpdateAvailable` to main loop
3. Popover shows "Update available vX.Y.Z" banner
4. User clicks → IPC `apply_update` → download + replace + restart
5. Restart via `std::os::unix::process::CommandExt::exec()`

### UI
- Banner in popover when update available (dismissible)
- "Updating..." state with progress indication
- Error display if download fails

## 3. Integration Points

### main.rs changes
- New `UserEvent` variants: `UpdateAvailable(UpdateInfo)`, `UpdateApply`
- Spawn updater check thread
- Handle IPC commands: `toggle_launch_at_login`, `apply_update`

### popover.html changes
- Auto-launch toggle in footer
- Update banner (hidden by default, shown via JS call)

### Cargo.toml additions
- `semver` crate for version comparison
- `flate2` + `tar` for tarball extraction
