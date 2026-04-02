# Claude Code Rate Watcher

A macOS menu bar app that monitors your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) API rate limit usage in real time.

![Screenshot](docs/screenshot.png)

## Features

- **Real-time tracking** — Monitors token usage across a sliding 5-hour window directly from your menu bar
- **Weekly limit monitoring** — Also tracks the 168-hour (weekly) token usage window
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
swift build -c release
# Binary is at .build/.../release/ccrw
./scripts/build_app.sh release
# App bundle is at dist/Claude Code Rate Watcher.app
```

## Requirements

- **macOS** (Apple Silicon and Intel supported)
- **Claude Code** must be installed — the app reads session data from `~/.claude/projects/`
- **Swift 6 toolchain** (only if building from source)

## How It Works

The app watches `~/.claude/projects/**/*.jsonl` session files for changes and calculates your token usage using cost-weighted values:

| Token Type | Weight |
|---|---|
| Input tokens | 1x |
| Output tokens | 5x |
| Cache creation | 1x |
| Cache read | 0.1x |

### Estimated Rate Limits (Max plan)

| Window | Estimated Limit |
|---|---|
| 5 hours | 25,000,000 weighted tokens |
| Weekly (168h) | 225,000,000 weighted tokens |

> These are heuristic estimates for the Max plan. You can adjust the constants in `Sources/CCRWCore/UsageCalculator.swift` to match your plan.

## Updating

If installed via Homebrew:

```bash
brew upgrade NicoValentine7/tap/claude-code-rate-watcher
```

If installed from source or direct download, the Swift app is wired for Sparkle-based updates. Signed release packaging should provide `CCRW_SPARKLE_FEED_URL` and `CCRW_SPARKLE_PUBLIC_KEY`.

## Uninstall

### Homebrew

```bash
brew uninstall claude-code-rate-watcher
```

### Manual cleanup

Remove the app and, if needed, disable it in macOS Login Items:

```bash
rm -rf "dist/Claude Code Rate Watcher.app"
```

## Contributing

Contributions are welcome! Feel free to open issues or pull requests.

## License

[MIT](LICENSE)
