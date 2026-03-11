# CLAUDE.md

## Project Overview

Claude Code Rate Watcher - macOS メニューバーアプリ。Claude Code の API レート制限使用量をリアルタイム監視する。

## Tech Stack

- **Language**: Rust (edition 2024)
- **GUI**: tao (windowing) + wry (WebView) + tray-icon (メニューバー)
- **File monitoring**: notify crate
- **Notifications**: notify-rust (macOS native)
- **Platform**: macOS only (Apple Silicon + Intel)

## Architecture

```
main.rs              - イベントループ、ウィンドウ管理、IPC
├── api_client.rs    - Anthropic API ポーリング（OAuth + Haiku probe）
├── auth.rs          - macOS Keychain からの認証情報取得
├── autolaunch.rs    - ログイン時自動起動（LaunchAgent plist 管理）
├── file_watcher.rs  - ~/.claude/projects/ の JSONL ファイル変更検知
├── session_parser.rs - JSONL セッションファイルのパース、トークン使用量抽出
├── usage_tracker.rs  - 使用量計算（5h / 168h ウィンドウ）、閾値判定
├── updater.rs       - GitHub Releases ベースの自動アップデート
├── tray.rs          - メニューバートレイアイコン管理
├── notification.rs  - システム通知（レートリミット付き）
├── icon.rs          - 使用率に応じた動的カラーアイコン生成
└── popover.html     - WebView UI（使用量表示、トークン詳細）
```

## Key Constants

- 5h window limit: 25,000,000 tokens (weighted) — `usage_tracker.rs`
- Weekly (168h) limit: 225,000,000 tokens (weighted) — `usage_tracker.rs`
- Output token weight: 5x input
- Cache read weight: 1/10 input
- Notification thresholds: 75% (warning), 90% (critical)
- UI update interval: 30 seconds
- File change debounce: 1 second

## Build & Run

```bash
cargo build --release    # リリースビルド
cargo run                # 開発実行
```

## Release Process

1. `Cargo.toml` の `version` を更新（semver 準拠: `MAJOR.MINOR.PATCH`）
2. 変更をコミット
3. タグを作成してプッシュ:
   ```bash
   git tag v0.x.0
   git push origin main --tags
   ```
4. GitHub Actions (`.github/workflows/release.yml`) が自動実行:
   - aarch64 + x86_64 のクロスビルド
   - `lipo` でユニバーサルバイナリ作成
   - `.app` バンドル作成（`Info.plist` + ユニバーサルバイナリ、`LSUIElement=true`）
   - Ad-hoc コード署名（`codesign --force --deep -s -`）
   - `claude-code-rate-watcher-macos-universal.tar.gz`（auto-updater 用）として GitHub Releases に公開
   - `claude-code-rate-watcher.dmg`（初回インストール用、.app + /Applications シンボリックリンク）も公開
   - `softprops/action-gh-release@v2` でリリースノート自動生成

### リリースタイミング

- **`src/` 配下の Rust コードに変更があった場合は必ずリリースする**（バージョンを上げてタグをプッシュ）
- `docs/` のみの変更（リリースページの見た目変更等）はリリース不要（Pages が自動デプロイされる）
- PR マージ後にコード変更が含まれていた場合もリリースを忘れないこと

### 重要な注意点

- **バージョンとタグは必ず一致させる**: `Cargo.toml` の version が `0.3.0` ならタグは `v0.3.0`
- **バージョンを上げずにリリースすると自動アップデートが動作しない**
- タグが `v*` パターンにマッチした push でのみ Release ワークフローが起動する

### GitHub Pages

- **URL**: https://nicovalentine7.github.io/claude-code-rate-watcher/
- **ソース**: `docs/` ディレクトリ（GitHub Pages の設定で `/docs` を指定）
- `docs/index.html` — リリースページ（EN/JA 言語切替対応）
- ダウンロードリンクは `https://github.com/NicoValentine7/claude-code-rate-watcher/releases/latest` を指す → 常に最新リリースが配布される
- `docs/` 内のファイルを変更して main に push すると、自動的に Pages がデプロイされる

### Auto-Updater

- `updater.rs` が GitHub Releases API (`https://api.github.com/repos/{owner}/{repo}/releases/latest`) でバージョンチェック
- チェックタイミング: 起動 5 秒後 + 6 時間ごと
- `Cargo.toml` の version と GitHub Release のタグ名を semver 比較
- 新バージョン検出 → popover にバナー表示 → ユーザーがクリックで tarball ダウンロード → バイナリ置換 → 再起動

### インストール先

- ユーザー配置先: `/Applications/Claude Code Rate Watcher.app`（DMG からドラッグ&ドロップ）
- LaunchAgent plist: `~/Library/LaunchAgents/com.claude-code-rate-watcher.plist`
- ログイン時自動起動: popover の「Launch at Login」トグルで ON/OFF

## Data Source

`~/.claude/projects/**/*.jsonl` — Claude Code のセッションジャーナルファイル

## UI Colors (Apple HIG)

- Green (#34C759): 0-69%
- Orange (#FF9F0A): 70-89%
- Red (#FF3B30): 90-100%
