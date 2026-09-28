# Hermes local usage DB — `~/.hermes/state.db`

If the user runs Hermes itself, every chat-completion call is logged into
`~/.hermes/state.db` → table `session_model_usage`. **This is the fastest,
most accurate source of "how much have I used"** — no provider round-trip,
no quota burn, no 404s. Use it before reaching for the provider API.

## Schema (verified 2026-07-21)

Table: `session_model_usage` (one row per `(session_id, model)` pair — a
single session that called two different models has two rows).

```
session_id          text    Hermes session id (e.g. '20260721_153401_1909f1')
model               text    canonical model name (e.g. 'MiniMax-M3')
billing_provider    text    'minimax-oauth', 'openrouter', 'anthropic', ...
billing_base_url    text    last-seen API base URL
billing_mode        text    '' for OAuth plans, otherwise e.g. 'paygo'
task                text    '' currently
api_call_count      int     number of calls in that session for that model
input_tokens        int     cumulative prompt tokens across the session
output_tokens       int     cumulative completion tokens
cache_read_tokens   int     cumulative cached-prompt tokens billed at discount
cache_write_tokens  int     cumulative cache-write tokens
reasoning_tokens    int     cumulative reasoning/thinking tokens (billed as output)
estimated_cost_usd  float   Hermes price-table estimate; 0 when cost_status='unknown'
actual_cost_usd     float   reported by provider when available
cost_status         text    'unknown' | 'estimated' | 'actual'
cost_source         text    'none' | 'price_table' | 'provider_response'
first_seen          float   Unix epoch seconds (UTC) — NOT ISO string
last_seen           float   Unix epoch seconds (UTC) — NOT ISO string
```

`first_seen` / `last_seen` are floats — convert with
`datetime.fromtimestamp(t, tz=timezone.utc)` before slicing on day.

## Gotchas

- **`estimated_cost_usd = 0` does NOT mean $0 spent.** It means Hermes has
  no price table for `billing_provider`. Common offenders: `minimax-oauth`,
  custom OAuth providers, anything new. The fields to trust for *quantity*
  are `input_tokens` / `output_tokens` / `cache_read_tokens` /
  `cache_write_tokens`. The fields to trust for *cost* are the provider's
  console — there is no workaround.
- **`api_call_count` is per (session, model), not per API round-trip within
  a session.** If a session made 4 calls in a row with retries, the count
  includes retries. Sum across rows to get total calls.
- **`cache_read_tokens` is the single biggest number for heavy system-prompt
  users** — it counts every cached-token billing event, so a 12k-token
  system prompt called 100 times contributes 1.2M cache_read_tokens even
  though only 12k unique tokens existed.
- **For USD estimates, apply public list pricing yourself** and label it as
  estimate. Billable input = `max(0, input_tokens - cache_read_tokens)`;
  reasoning tokens bill as output. See `scripts/hermes_usage_tally.py`
  for a runnable tally with default MiniMax-M3 prices
  ($3 / $0.30 / $15 / $3.75 per 1M for input/cache_read/output/cache_write).

## Run the tally

```bash
uv run --with aiosqlite python3 ~/.hermes/skills/mlops/provider-api-usage/scripts/hermes_usage_tally.py
uv run --with aiosqlite python3 ~/.hermes/skills/mlops/provider-api-usage/scripts/hermes_usage_tally.py --model '%openai%' --provider openrouter
uv run --with aiosqlite python3 ~/.hermes/skills/mlops/provider-api-usage/scripts/hermes_usage_tally.py --days 7
```

## What this DB is NOT

- Not a balance/plan readout. Hermes never sees your plan limit; only the
  provider console does.
- Not a per-message breakdown. One row per `(session, model)` — use
  `~/.hermes/sessions/request_dump_*.json` for per-call dumps (but those
  are mostly failed requests, not successful ones with usage blocks).
- Not synced across machines. It's local to the Hermes install that made
  the calls.

## Related tables in `state.db` (FYI)

- `messages` — full conversation history per session (with role, content).
- `messages_fts` — FTS5 index over messages, useful for "find the session
  where I asked about X".
- `sessions` — session-level metadata (created_at, channel, etc).
- `gateway_routing` — which messaging-channel session maps to which agent
  session id. Also mirrored read-only at
  `~/.hermes/sessions/sessions.json`.

## Companion files

- `scripts/hermes_usage_tally.py` — runnable tally with USD estimate.
- The provider-side `references/minimax.md` covers the route-side
  liveness check (why `account/info` 404s and what it means).