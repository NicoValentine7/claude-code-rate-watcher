# Current Maintenance Plan

## Current State

Rate Watcher is a macOS menu bar app that combines reported rate limit data from Claude Code and Codex. The repository and Homebrew package still use `claude-code-rate-watcher`.

- Claude Code data comes from the Claude usage API/statusline integration.
- Codex data comes from local `rate_limits` snapshots in `~/.codex/sessions/**/*.jsonl` and `~/.codex/archived_sessions/*.jsonl`.
- Codex local data is refreshed through file notifications plus a short fallback scan, while Claude API polling stays throttled separately.
- The menu bar percentage uses the higher 5-hour value between Claude Code and Codex, and the popover labels the source driving that value.
- Release packaging is tag-driven through `.github/workflows/release.yml`, producing `claude-code-rate-watcher-macos-universal.tar.gz` and updating the Homebrew tap.
- The Release workflow smoke-tests the generated tarball before publishing by extracting it and checking `ccrw --version` against the tag.

## Maintenance Priorities

1. Keep release infrastructure boring.
   - Release workflow actions should stay on supported runtimes.
   - Release tarballs should stay smoke-tested before publishing.
   - Release notes should be hand-written for user-visible changes.
   - Homebrew Formula output should be checked after every release.

2. Keep reported usage fresh and explainable.
   - Prefer provider-reported percentages.
   - Treat expired reset windows as 0%.
   - Keep Codex local refresh fast without increasing Claude API polling pressure.

3. Keep docs aligned with shipped behavior.
   - README should describe Claude Code and Codex support together.
   - This file should track current maintenance direction, not old one-off implementation plans.

## Next Good Candidates

- Keep release smoke checks aligned with any future artifact layout changes.
- Consider linking from the popover to both Claude and Codex usage/help locations if Codex exposes a stable one.
