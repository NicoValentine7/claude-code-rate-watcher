# Claude Code Rate Watcher UI Mockups

`ui-concepts.html` には、Swift ネイティブ版ポップオーバーの方向性を比べるための 3 案をまとめています。

## Concepts

- `Concept A / Command Center`
  - 一番実装しやすい案
  - 5h の利用率を最優先で見せる
  - 情報量は保ちつつ、視線移動を減らす

- `Concept B / Native Glass`
  - macOS らしい軽さと材質感を重視
  - 補助情報を脇に逃がして中央を静かに保つ
  - 長時間開いていても疲れにくい方向

- `Concept C / Alert Radar`
  - 監視ツールとしての緊張感を前面に出す
  - しきい値、余力、次の行動を強く見せる
  - 危険域の見落としを減らしやすい

## Best-Practice Notes

- メニューバー系ユーティリティは、最初の視線で主要状態がわかることを優先したい
- 材質表現は使いどころを絞り、常時すべてをガラス化しないほうが読みやすい
- `MenuBarExtra` には依然として制御上の制約があるため、現行の `NSStatusItem + NSPopover` 方針は妥当
- 監視アプリでは、主指標と補助情報の階層差をはっきり出したほうが判断が速い

## Open

ローカルで確認する場合:

```bash
open /Users/nico/projects/claude-code-rate-watcher/docs/mockups/ui-concepts.html
```
