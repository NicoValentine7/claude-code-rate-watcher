# Claude Code Rate Watcher

Claude Code の API レート制限使用量をメニューバーからリアルタイム監視する macOS アプリ。

## Features

- **5 時間ウィンドウ追跡** — スライディングウィンドウでトークン使用量をリアルタイム表示
- **週次制限モニタリング** — 168 時間の週次トークン使用量も同時に追跡
- **閾値アラート** — 使用量が 75% / 90% を超えると macOS 通知でお知らせ
- **リセットカウントダウン** — レート制限のリセットまでの残り時間を表示
- **カラーコードアイコン** — 使用率に応じてメニューバーアイコンの色が変化（緑 → 橙 → 赤）

## Install

### GitHub Releases からダウンロード

1. [Releases](https://github.com/NicoValentine7/claude-code-rate-watcher/releases/latest) からダウンロード
2. 展開してバイナリを `/usr/local/bin/` 等に配置
3. ターミナルから `claude-code-rate-watcher` を実行

### ソースからビルド

```bash
git clone https://github.com/NicoValentine7/claude-code-rate-watcher.git
cd claude-code-rate-watcher
cargo build --release
```

ビルド成果物: `target/release/claude-code-rate-watcher`

## Requirements

- macOS 10.13+
- Rust toolchain（ソースビルドの場合）
- Claude Code がインストール済みであること（`~/.claude/projects/` にセッションデータが必要）

## How it Works

`~/.claude/projects/` 配下の JSONL セッションファイルを監視し、assistant レスポンスのトークン使用量を集計します。

トークンはコスト加重で計算されます：

| トークン種別 | 重み |
|---|---|
| Input tokens | 1x |
| Output tokens | 5x |
| Cache creation | 1x |
| Cache read | 0.1x |

### レート制限の推定値

| ウィンドウ | 推定上限 |
|---|---|
| 5 時間 | 25,000,000 tokens (weighted) |
| 週次 (168h) | 225,000,000 tokens (weighted) |

> これらは Max プランのヒューリスティック推定値です。`src/usage_tracker.rs` の定数を調整して利用してください。

## License

MIT
