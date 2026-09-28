---
name: provider-api-usage
description: Check remaining balance, token quota, plan usage, or spend on a paid API (LLM providers like MiniMax, OpenAI, Anthropic). Covers locating creds safely, probing balance endpoints, proving key liveness when an endpoint 404s, and falling back to console UIs or a local usage tracker.
---

# Provider API Usage & Balance Checks

Use when a user asks to check remaining balance, token quota, plan usage, or spend on a paid API (LLM providers like MiniMax, OpenAI, Anthropic, etc.). Common trigger: "how do I check my <provider> token plan usage" after buying a subscription.

## Workflow
0. **Pick the right track before any HTTP call:**
   - **Hermes itself?** Check the local usage DB FIRST — see `references/hermes-usage-db.md` for the schema. Faster than any provider API and doesn't burn quota.
   - **OpenAI/Codex CLI on a ChatGPT plan?** Live API endpoints are unreachable from CLI (OAuth rejected on `api.openai.com/v1/*`, browser cookies required on `chatgpt.com/backend-api/*`). The canonical source is **on disk**: `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<UUID>.jsonl` has `event_msg.payload.type=="token_count"` events with `last_token_usage` + `rate_limits.primary.{used_percent,window_minutes,resets_at}`. Companion DB `~/.codex/state_5.sqlite → threads.tokens_used` is a cumulative context counter (per-thread running total), not a per-turn sum — use the JSONLs for accurate accounting. Plan tier & renewal come from `~/.codex/auth.json → tokens.id_token` (base64url-decode the JWT payload, no sig check needed for inspection). See `references/codex.md` for the full recipe.
   - **API-key provider (MiniMax, Anthropic key-mode, OpenRouter key-mode, …)?** Continue to step 1.
1. **Locate credentials without leaking them.**
   - `search_files` with `target="files"` for `*.env` — it OMITs results for secret files by default (defense-in-depth).
   - Fall back to `terminal`: `find <root> -maxdepth 4 -name ".env" -type f` to recover the real path.
   - `read_file` on `.env` is BLOCKED. Confirm presence via terminal, masking the value:
     `grep -iE "KEY|TOKEN|GROUP" file.env | sed -E 's/=(.{6}).*/=\1…(masked)/'`
   - For OAuth tokens (Codex `auth.json`, GoA JSON, etc.): prefer `python -c` JSON inspection with explicit redaction in the script; never let the token reach stdout unredacted.
2. **Write a script** that loads creds via `python-dotenv`, pulls the key at runtime, and NEVER prints the raw secret. Run with `uv run --with python-dotenv script.py`.
3. **Try the documented balance/usage endpoint.** 200 ⇒ pretty-print balance fields.
4. **On 404, prove key validity with a known-good call** (e.g. a tiny chat completion). This separates "dead route" from "bad/expired key":
   - 200 on a real call ⇒ key is alive; the balance endpoint is gone/changed.
   - Inspect response headers: some providers return a request-id even on 404 (MiniMax returns `Minimax-Request-Id` / `alb_request_id`), proving the request reached their servers — so it's a routing problem, not auth.
5. **Probe variants before concluding:** with/without `/v1`, `api_key=` query param vs `Authorization: Bearer`, with/without group/project id, and alternate domains (`.io` vs `.chat`). For OAuth bearer providers, also test against the *vendor* endpoint (e.g. `chatgpt.com/backend-api/...`) — the platform API often rejects OAuth tokens that work elsewhere.
6. **If everything 404s/403s**, the provider moved usage to the console UI. Point the user to the login-gated dashboard (e.g. `https://provider.com/console` → Billing/Usage/Token Plan). If Hermes already tracks usage locally, fall back to the Hermes DB path and offer a list-price estimate in USD (clearly labeled as estimate, since the console is the only authoritative source).

## Pitfalls
- Never claim a successful readout when the server 404s — report the real API state. A script that "runs and exits 0 against a live 404" is verified; a balance number that never came back is NOT. State the blocker explicitly.
- Don't ask the user to paste their API key into chat. Load it from `.env`; keep it out of stdout.
- The console is authoritative for subscription token plans; the open API often only exposes credit balance (if anything). A `$N` token plan is usually only visible in the console.
- Don't loop endlessly on one endpoint. After ~3 variants 404 plus a failed SDK check, conclude the endpoint is deprecated and switch to the console/Hermes-DB/local-tracker path.
- **Hermes DB gotchas:** `first_seen` / `last_seen` are **Unix epoch floats (UTC)**, not ISO strings — convert with `datetime.fromtimestamp(t, tz=timezone.utc)` or the day buckets break. `estimated_cost_usd` is **0 with `cost_status='unknown'`** for any `billing_provider` not in Hermes's price table (e.g. `minimax-oauth`, custom providers) — that means Hermes didn't price the call, not that the call was free. To estimate USD against a real plan, apply public list pricing yourself and label it clearly as estimate. Don't conflate "estimated_cost_usd = 0" with "$0 spent".
- OpenCode's per-session log (`~/.local/share/opencode/log/opencode.log`) has a `tokens.input=N tokens.output=N ...` block on every session-created/updated line, but those are **cumulative running totals**, not per-call — and the snapshot at session-create is all zeros. It's only useful for "which model did this session use", not for usage accounting. Prefer the Hermes DB.
- **OAuth bearer ≠ API key.** A ChatGPT/Codex OAuth token in `~/.codex/auth.json` works only against `chatgpt.com/backend-api/codex/*` from a browser session. From a script/CLI it returns `401 invalid_jwt` on every `api.openai.com/v1/*` endpoint and `403` on `chatgpt.com/backend-api/*` (needs session cookie + UA). Don't waste 4-5 round trips discovering this — read the JWT header first: if the `id_token` exists and there's no `OPENAI_API_KEY`, you've got an OAuth-only install and live API checks are off the table. Pivot to on-disk artifacts (`~/.codex/sessions/*/rollout-*.jsonl`).
- **Codex `state_5.sqlite → threads.tokens_used` is a cumulative context counter**, not a per-turn token sum. It keeps growing for the lifetime of a thread (because each turn sees an ever-larger conversation context). Summing it across threads gives "largest context window seen today", not "tokens spent today". Use the rollout JSONLs.
- **Day-bucketing the rollout JSONLs by filename is more reliable than by line timestamp** — rollouts can have warm-up lines that start at the previous day; the filename is when the session opened. `fp.rsplit('/',1)[-1][8:18]` gives `YYYY-MM-DD`.
- **Cache hit rate is wildly inflated** on heavy CLI sessions (system prompt + skills + tool schema is the repeated prefix; 95%+ cache hit on a long session). Useful as "the cache is doing real work", NOT as a proxy for spend.
- **Base-URL override mismatch is a distinct 404 from a dead route.** When a client sends a request-shape the endpoint doesn't serve, you get `404 page not found` even though the key is healthy and quota is fine. Classic case (Hermes `minimax`, 2026-08-02): a provider uses Anthropic-Messages protocol (`api_mode="anthropic_messages"`, default base `https://api.minimax.io/anthropic`) but an env override `MINIMAX_BASE_URL=https://api.minimax.io/v1` (OpenAI base) wins at resolution time (`base_url = env_url or pconfig.inference_base_url`), so Anthropic-shaped POSTs land on `/v1/messages` → 404. The fix is to align the override with the provider's protocol base, then RESTART the client (env is read at startup). To distinguish: (a) reproduce the EXACT request shape the client sends at the suspect base, and (b) at the provider's canonical base — if (a) 404s and (b) 200s with the same key, it's this mismatch, not a dead key. Reusable harness: `scripts/minimax_baseurl_mismatch.py`. This generalizes beyond MiniMax: any provider that auto-derives `base_url_env_var` from its name (e.g. `MINIMAX_BASE_URL`, `OPENAI_BASE_URL`) can be hijacked by a stale/incorrect override.
- **CLI-script traps** when shipping a wrapper that does `python3 - <<'PY' ... ARGS`:
  - `if json_out:` against `sys.argv` value `'false'` is **truthy in Python** — `'false'` is a non-empty string, not `False`. Always coerce: `def as_bool(s): return str(s).strip().lower() in ('1','true','yes','y','on')`. Without this, a `--rate` flag can silently fire the JSON branch every time (caught and fixed in `scripts/codex_usage.sh`).
  - Bash `for arg in "$@"; case "$arg" in --since) shift; VAR=$1; shift;` is fragile — after `shift; VAR=$1`, the next loop iteration sees the value as `$arg` and falls into the default `*)` "unknown arg" branch. Use a `prev=""` state machine: when `prev` is set, capture and `continue`; otherwise set `prev` for long-with-value args. Reference: `scripts/codex_usage.sh` arg loop.

## References
- `references/minimax.md` — MiniMax findings: dead `account/info` 404 (route gone, key alive) AND the distinct `MINIMAX_BASE_URL` override-mismatch 404 (Anthropic-Messages provider routed at `/v1` → 404, fixed by pointing the override at `/anthropic` + restart). Plus console-only usage, GroupId note, do-code vs Hermes-OAuth key split.
- `references/codex.md` — Codex CLI / ChatGPT OAuth-plan usage: rollout-JSONL `token_count` parsing recipe, id_token JWT plan-tier decode, OAuth-bearer rejection on both platform API and `chatgpt.com/backend-api`, real per-day/cache-hit/rate-limit sample from 2026-07-27.
- `references/hermes-usage-db.md` — `~/.hermes/state.db` schema, SQL recipes, and USD-estimate notes for Hermes-local usage accounting.
- `scripts/minimax_cache_probe.py` — live prompt-caching probe: confirms cache hits on YOUR key before trusting the "M3 cache broken" rumors.
- `scripts/minimax_baseurl_mismatch.py` — MiniMax `minimax` provider 404 diagnostic: reads active `MINIMAX_BASE_URL`, derives the effective Anthropic-Messages request path, and reproduces both the broken (`/v1`) and correct (`/anthropic`) paths to confirm a base-URL-override mismatch vs a dead key.
- `scripts/hermes_usage_tally.py` — runnable tally: per-model totals + per-day buckets + public-list-price USD estimate from `~/.hermes/state.db`.
- `scripts/codex_usage.sh` — Codex CLI / ChatGPT OAuth-plan usage: parses `~/.codex/sessions/*/rollout-*.jsonl` for `token_count` events, decodes the id_token JWT for plan tier/renewal. Use `--rate` for the current window percent, `--since 7d` for last week, `--json` for machine output. Companion install at `~/scripts/codex-usage/codex_usage.sh` for users who want it on PATH.

## MiniMax prompt caching — verify it live
MiniMax-M3 supports **passive (automatic) prompt caching**: repeating an identical long
prefix across calls bills the repeated tokens at a discount, surfaced as
`usage.prompt_tokens_details.cached_tokens`. Explicit `cache_control` mode exists too but
is **M2.x only, not M3**. Token plans are just prepaid billing — caching discounts apply.
- June-2026 reports claim M3 caching is "broken" on the direct API (tied to the `thinking`
  toggle). **Verify on the actual key** rather than assume: `python3 scripts/minimax_cache_probe.py --env-file /path/to/.env --repeats 3`.
- Real finding on a live `$20` plan: with a ~12k-token identical prefix, call 1 caches a
  ~128-token warm-up, calls 2–3 cache **~100%** (the whole repeated prefix). Caching works.
- **Minimum-prefix threshold:** short prefixes (<~200 tokens) cache **0** — MiniMax doesn't
  bother. To benefit, keep the repeated prefix (system prompt + tool schema + doc context)
  long and identical between calls. Long-context pricing (per-token jump >512k) includes
  cache-hit tokens.
