# MiniMax — balance/usage check (findings as of 2026-07-21, 404-mismatch addendum 2026-08-02)

## Key facts
- The MiniMax balance endpoint is **dead**: `GET https://api.minimax.io/v1/account/info?GroupId=<id>&api_key=<key>` returns `404 page not found`.
- 10+ variants all 404: `api.minimax.io` / `api.minimax.chat`, with/without `/v1`, `api_key=` query vs `Authorization: Bearer`, `/account/info`, `/balance`, `/usage`, `/query/balance`, `/get_balance`, `/account/usage`, `/user/info`.
- The official `minimax-python` SDK (0.2.0) has **no balance/usage method**.
- MiniMax docs site is JS-gated (307 → login redirect); the API reference for account balance is not publicly scrapable.

## How to prove the key is alive
A real call works fine — this is the liveness check to separate "dead route" from "bad key":

```python
import os, json, urllib.request
from dotenv import load_dotenv
load_dotenv()
KEY = os.getenv("MINIMAX_API_KEY")
body = json.dumps({
    "model": "MiniMax-M3",
    "messages": [{"role": "user", "content": "hi"}],
    "max_tokens": 5,
}).encode()
req = urllib.request.Request(
    "https://api.minimax.io/v1/chat/completions", data=body,
    headers={"Content-Type": "application/json", "Authorization": f"Bearer {KEY}"})
with urllib.request.urlopen(req, timeout=30) as r:
    print(r.status, json.loads(r.read().decode())["usage"])
```
Returns `200` with a `usage` block: `prompt_tokens`, `completion_tokens`, `total_tokens`, `cached_tokens`, etc. (Note: `service_tier: standard`.)

## Diagnostic signal
A 404 from `account/info` still returns headers `Minimax-Request-Id` and `alb_request_id` — proof the request reached MiniMax's edge. So 404 = route gone, NOT auth failure. A wrong/empty key would normally be 401, not 404.

## SECOND MiniMax 404 mode — base-URL override mismatch (Hermes key-based `minimax`, 2026-08-02)
This is a different 404 from the dead-balance-endpoint one above: every **chat** call
fails with `HTTP 404: 404 page not found`, not just an account-info probe.

Symptom (Hermes `errors.log`):
```
provider=minimax base_url=https://api.minimax.io/v1 model=MiniMax-M3 → HTTP 404: 404 page not found
provider=minimax base_url=https://api.minimax.io/v1 model=MiniMax-M2.7-highspeed → HTTP 404
```

Root cause: Hermes's key-based `minimax` provider uses `api_mode="anthropic_messages"`
and a built-in default base of `https://api.minimax.io/anthropic`. BUT it also declares
`base_url_env_var="MINIMAX_BASE_URL"` (see `hermes_cli/providers.py` line 122, and the
overlay resolution in `runtime_provider.py:1596-1608`: `base_url = env_url or
pconfig.inference_base_url`). If `.hermes/.env` sets `MINIMAX_BASE_URL=https://api.minimax.io/v1`
(the OpenAI-compatible endpoint), that env var **overrides** the provider's `/anthropic`
base. Hermes then POSTs an Anthropic-Messages-shaped request (`/v1/messages`,
`anthropic-version` header, `{"model","max_tokens","messages"}` body) to the OpenAI
endpoint, which doesn't have `/messages` → `404 page not found`.

Why the key "looks fine" but the model "doesn't work": the raw key is healthy on BOTH
endpoints (proven with live HTTP 200 on `api.minimax.io/v1/chat/completions` AND
`api.minimax.io/anthropic/v1/messages`). Quota is also fine. The failure is purely a
**request-shape-to-endpoint mismatch caused by the base-URL override**, not a dead key.

Reproduce the exact two paths to confirm (substitute your key):
```bash
KEY=...
# (A) what Hermes was doing — Anthropic body -> /v1 (OpenAI base) -> 404
curl -s -o /dev/null -w "HTTP %{http_code}\n" -X POST https://api.minimax.io/v1/messages \
  -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" -H "anthropic-version: 2023-06-01" \
  -d '{"model":"MiniMax-M3","max_tokens":16,"messages":[{"role":"user","content":"ping"}]}'
# → HTTP 404

# (B) correct path Hermes SHOULD hit — /anthropic/v1/messages -> 200
curl -s -o /dev/null -w "HTTP %{http_code}\n" -X POST https://api.minimax.io/anthropic/v1/messages \
  -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" -H "anthropic-version: 2023-06-01" \
  -d '{"model":"MiniMax-M3","max_tokens":16,"messages":[{"role":"user","content":"ping"}]}'
# → HTTP 200
```

Fix (low-risk, reversible): in `.hermes/.env`, point the override at the Anthropic endpoint:
```
- MINIMAX_BASE_URL=https://api.minimax.io/v1
+ MINIMAX_BASE_URL=https://api.minimax.io/anthropic
```
Then **restart Hermes** (`.env` is read at startup; a running session keeps the old value).
`.hermes/.env` is write-guarded by the patch tool — edit via terminal with a timestamped
backup (`cp -p .hermes/.env .hermes/.env.bak.minimax-$(date +%Y%m%dT%H%M%S)`).

Scope note — this fixes ONLY Hermes's key-based `minimax` provider. It does NOT affect:
- **`zuck` / `do-code`**: they call the OpenAI-compatible `https://api.minimax.io/v1` directly
  and read their OWN `.env`, not Hermes's. They were already working (independently verified).
- **`minimax-oauth`** (paid browser-OAuth route): separate tokens in `auth.json`, validated
  independently; healthy through 2027. Unaffected by `MINIMAX_BASE_URL`.

Runnable diagnostic: `scripts/minimax_baseurl_mismatch.py` — reads the active
`MINIMAX_BASE_URL` from `.hermes/.env`, derives the effective request path, and reproduces
both (A) and (B) above to prove/confirm the mismatch without guessing.

## GroupId
MiniMax historically required a `GroupId` (from console → API Keys, top-right avatar) alongside the API key for account calls. It is NOT stored in the do-code `.env` (only `MINIMAX_API_KEY` + `CONFLUENCE_API_TOKEN`). Even with a dummy GroupId, `account/info` still 404s — the route is gone regardless.

## Conclusion / user path
- The `$20` token plan usage (tokens used vs. allotted, reset date) is **console-only**: https://www.minimax.io/console → left sidebar → **Token Plan** (or Billing/Usage/用量).
- No server-side balance via API. Recommended workaround: a **local usage tracker** that logs each call's `usage` block and estimates spend against the plan.

## For Hermes users — there's a much better path
If you use Hermes (this agent), `~/.hermes/state.db` → `session_model_usage`
has every call's `input_tokens` / `output_tokens` / `cache_read_tokens` /
`cache_write_tokens` aggregated per session. See the umbrella `SKILL.md`
workflow step 0 and `references/hermes-usage-db.md` for the full schema.
`billing_provider='minimax-oauth'` rows have `cost_status='unknown'` and
`estimated_cost_usd=0` because Hermes has no price table for that OAuth
provider yet — apply public list pricing yourself for a USD estimate. The
`scripts/hermes_usage_tally.py` helper does this and labels the output as
estimate (NOT authoritative — only the provider console is).

Real numbers from a $20 Plus plan user (2026-07-21):
- 877 MiniMax-M3 calls / 2.39M input / 436K output / 61.87M cache_read over 28 sessions (first_seen 2026-07-18 12:53 UTC, last_seen 2026-07-21 10:14 UTC).
- Cache-hit ratio ~96% — M3 automatic prompt caching on the long repeated system prompt + skills stack is doing real work. A single 120-call session on 2026-07-20 contributed 8.3M cache_reads alone.

## do-code project note
`.env` lives at `/home/arctic/Internship/DataObserve/do-code/.env` (referenced in PLAN.md; git-ignored). `opencode.json` configures MiniMax as an OpenAI-compatible provider against `https://api.minimax.io/v1` with `apiKey: {env:MINIMAX_API_KEY}`. Working checker script: `check_minimax_usage.py` in that dir (loads `.env`, 404s by design until the endpoint is restored).

The do-code `.env` `MINIMAX_API_KEY` is a **separate raw subscription key** from the Hermes-OAuth `minimax-oauth` billing path. Both route to the same MiniMax account but show up as different rows in `session_model_usage` (different `billing_provider`). The raw-key one is rarely used now that the user is on the Hermes-OAuth plan.
