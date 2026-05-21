# Rate Watcher

A macOS menu bar app that monitors your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) and Codex rate limit usage in real time. Distributed as `claude-code-rate-watcher`.

![Screenshot](docs/screenshot.png)

## Features

- **Real-time tracking** — Monitors rate limit usage across a sliding 5-hour window directly from your menu bar
- **Source clarity** — Shows whether the menu bar percentage is currently coming from Claude Code, Codex, or both
- **Weekly limit monitoring** — Also tracks the 168-hour (weekly) token usage window
- **Codex support** — Reads Codex local session rate limit data from `~/.codex/sessions/` and `~/.codex/archived_sessions/`
- **Color-coded icon** — Menu bar icon changes color based on usage level (green → orange → red)
- **Reset countdown** — Shows time remaining until your rate limit resets
- **Threshold notifications** — Native macOS alerts when usage hits 75% and 90%
- **Auto-update** — Checks for new versions automatically and updates with one click
- **Launch at Login** — Starts automatically when you log in (enabled by default, can be toggled off)

## Install

### Homebrew (recommended)

```bash
brew install NicoValentine7/tap/claude-code-rate-watcher
```

Then launch it:

```bash
ccrw
```

That's it! On first launch, **Launch at Login** is automatically enabled so it starts every time you log in. You can toggle this off in the popover menu if you prefer.

### Build from source

```bash
git clone https://github.com/NicoValentine7/claude-code-rate-watcher.git
cd claude-code-rate-watcher
cargo build --release
# Binary is at target/release/ccrw
```

## Requirements

- **macOS** (Apple Silicon and Intel supported)
- **Claude Code** or **Codex** must be installed — the app reads local session/rate-limit data from `~/.claude/` and `~/.codex/`
- **Rust toolchain** (only if building from source)

## How It Works

For Claude Code, the app uses Claude Code's statusline/API rate limit data.

For Codex, the app watches `~/.codex/sessions/**/*.jsonl` and `~/.codex/archived_sessions/*.jsonl`, then reads the `rate_limits` snapshots written by Codex.

Both sources report 5-hour and weekly percentages directly. The menu bar shows the higher 5-hour value, and the popover labels which source is driving that menu bar percentage.

## Updating

If installed via Homebrew:

```bash
brew upgrade NicoValentine7/tap/claude-code-rate-watcher
```

If installed from source or direct download, the app checks for updates automatically and shows an update banner in the popover when a new version is available.

## Uninstall

### Homebrew

```bash
brew uninstall claude-code-rate-watcher
```

### Manual cleanup

Remove the Launch Agent (if auto-start was enabled):

```bash
launchctl unload ~/Library/LaunchAgents/com.claude-code-rate-watcher.plist
rm ~/Library/LaunchAgents/com.claude-code-rate-watcher.plist
```

## Contributing

Contributions are welcome! Feel free to open issues or pull requests.

## License

[MIT](LICENSE)
