# Implementation Plan: Real Rate Limit Data via API

## Goal
Replace estimated token-based usage calculation with real rate limit data from Anthropic's API.

## Architecture

### Auth Token Acquisition (new module: `auth.rs`)
1. Read `cookie.claude` from macOS Keychain via `security` CLI
2. Parse JSON → extract `cookieHeader` (e.g., `sessionKey=sk-ant-sid01-...`)
3. Fallback: read `Claude Code-credentials` keychain → parse `claudeAiOauth.accessToken` for Bearer token

### API Client (new module: `api_client.rs`)
1. **Primary**: `GET /api/oauth/usage` with Cookie auth
   - Returns `{"five_hour": {"utilization": 6.0, "resets_at": "..."}, "seven_day": {...}}`
   - Utilization is 0-100 percentage
2. **Fallback** (if Bearer token available): Haiku Probe
   - `POST /v1/messages` with `model=claude-haiku-4-5-20251001, max_tokens=1, messages=[{role:user,content:"h"}]`
   - Read `anthropic-ratelimit-unified-5h-utilization` and `7d-utilization` from response headers
   - Values are 0.0-1.0 floats
3. Poll interval: 60 seconds (with cache)
4. On 429/error: keep last known values, retry with exponential backoff

### Data Flow Changes
- `UsageSummary` gets new fields: `api_5h_percent: Option<u32>`, `api_7d_percent: Option<u32>`
- UI displays API percentages when available, falls back to local JSONL estimate
- Badge shows data source: "Live" (API) vs "Est." (local)

### Dependencies
- Add `ureq` (lightweight sync HTTP client) to Cargo.toml

### Files to Modify
1. `Cargo.toml` — add `ureq`
2. New `src/auth.rs` — keychain token extraction
3. New `src/api_client.rs` — usage API + haiku probe
4. `src/usage_tracker.rs` — add API fields to UsageSummary/UsagePayload
5. `src/main.rs` — spawn API polling thread, merge data
6. `src/popover.html` — show "Live" badge, display API data

### UI Changes
- When API data available: show actual % from API (green "Live" badge)
- When API unavailable: show local estimate (gray "Est." badge)
- Both 5h and 7d sections update from API data
