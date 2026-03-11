# Token Auto-Refresh & Error Visibility

## Problem

When the OAuth access token expires, both API methods (`/api/oauth/usage` and Haiku probe) fail silently. The UI shows "Connecting to API..." indefinitely with no indication of what went wrong. The `refreshToken` stored in Keychain is never used.

## Design

### 1. OAuth Token Auto-Refresh (`auth.rs`)

Add `refresh_token()` that:
- Reads `refreshToken` from the existing Keychain entry (`Claude Code-credentials`)
- POSTs to `https://console.anthropic.com/v1/oauth/token` with `grant_type=refresh_token`
- On success: updates the Keychain entry with the new `accessToken` (and new `refreshToken` if returned)
- On failure: returns an error (refresh token itself may be expired → user needs `claude login`)

`get_credential()` stays unchanged — it just reads the current token. Refresh is triggered by `api_client.rs` when it detects a 401.

### 2. Error Tracking (`api_client.rs`)

Add to `ApiRateLimitData`:
- `error_message: Option<String>` — short user-facing message ("Token expired", "Rate limited", "Network error")
- `error_detail: Option<String>` — technical detail (HTTP status, response body excerpt)
- `retry_count: u32` — how many consecutive failures

Flow in `poll()`:
1. Get credential → if missing, set `auth_missing: true`, return
2. Try `/api/oauth/usage` → if 401, attempt refresh → retry once
3. If still failing, try Haiku probe → if 401, attempt refresh → retry once
4. On any failure: set `error_message` and `error_detail`, increment `retry_count`
5. On 429: set longer cache TTL (5 min) to avoid hammering the API
6. On success: clear error state, reset `retry_count`
7. `eprintln!` all errors for debug log visibility

### 3. UI Error Display (`popover.html`)

State 2 (not live) changes from just "Connecting to API..." to:
- Default: "Connecting to API..." (first attempt, no error yet)
- With error: show short error message below spinner (e.g. "Token expired — refreshing...")
- Clickable "Show details" text toggles a detail section with `error_detail` and `retry_count`
- If refresh fails and token is truly dead: show "Login Required" state instead of connecting forever

### 4. Retry Strategy

- Token expired (401): refresh → immediate retry → if fails, show error + retry on next 30s tick
- Rate limited (429): cache TTL = 5 min, show "Rate limited" message
- Network error: retry on next 30s tick, show "Network error"
- Refresh token expired: transition to `auth_missing` state (show Login button)

## Files Changed

- `src/auth.rs` — add `refresh_token()`, `save_credential()`
- `src/api_client.rs` — add error fields, refresh-on-401 logic, 429 backoff
- `src/usage_tracker.rs` — pass new error fields through `UsagePayload`
- `src/popover.html` — error message display, expandable details
