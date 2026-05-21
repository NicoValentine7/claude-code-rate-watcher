# Current Maintenance Plan

## Current State

Claude Code Rate Watcher is now a macOS menu bar app that combines reported rate limit data from Claude Code and Codex.

- Claude Code data comes from the Claude usage API/statusline integration.
- Codex data comes from local `rate_limits` snapshots in `~/.codex/sessions/**/*.jsonl` and `~/.codex/archived_sessions/*.jsonl`.
- The menu bar percentage uses the higher 5-hour value between Claude Code and Codex.
- Release packaging is tag-driven through `.github/workflows/release.yml`, producing `claude-code-rate-watcher-macos-universal.tar.gz` and updating the Homebrew tap.

## Maintenance Priorities

1. Keep release infrastructure boring.
   - Release workflow actions should stay on supported runtimes.
   - Release notes should be hand-written for user-visible changes.
   - Homebrew Formula output should be checked after every release.

2. Keep reported usage fresh and explainable.
   - Prefer provider-reported percentages over token estimates.
   - Treat expired reset windows as 0%.
   - Avoid reparsing large local session files unless their metadata changes.

3. Keep docs aligned with shipped behavior.
   - README should describe Claude Code and Codex support together.
   - This file should track current maintenance direction, not old one-off implementation plans.

## Next Good Candidates

- Add tests for file watcher path selection if watcher behavior grows again.
- Consider showing whether the menu bar percentage currently comes from Claude Code or Codex.
- Replace any remaining hard-coded version strings with `CARGO_PKG_VERSION` or release metadata.
