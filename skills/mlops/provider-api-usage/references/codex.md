# Codex CLI (ChatGPT / Plus / Pro / Business) — usage check findings (2026-07-27)

## Key facts
- Codex CLI authenticates via OAuth against `chatgpt.com/backend-api/codex/*`, NOT against `api.openai.com/v1/*`. The OAuth bearer **does not work on the platform API** — every dashboard-style endpoint (`/v1/dashboard/billing/credit_grants`, `/v1/dashboard/billing/usage`, `/v1/dashboard/billing/subscription`, `/v1/me`) returns `401 invalid_jwt` with the exact error `Could not parse your authentication token`.
- Even on the Codex backend, **non-browser requests are blocked**: `chatgpt.com/backend-api/codex/usage`, `/rate_limits`, `/credits`, `/account/usage`, `/accounts/check` all return `403` to a plain `Authorization: Bearer <token>` (likely needs a session cookie + UA). So there is **no live API path** for a usage readout.
- The canonical usage source is **on disk** at `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<UUID>.jsonl`. These files contain `event_msg.payload.type == "token_count"` events with everything you need:
  - `last_token_usage.input_tokens / output_tokens / cached_input_tokens / reasoning_output_tokens` — per-turn deltas.
  - `total_token_usage.*` — cumulative-per-session running total.
  - **`rate_limits.primary.used_percent`** — the actual consumption-percentage tracking you want.
  - `rate_limits.primary.window_minutes` — typically `10080` (7 days) on Plus.
  - `rate_limits.primary.resets_at` — Unix epoch seconds (UTC) when the bucket flips.
  - `rate_limits.credits.{unlimited, has_credits}` and `rate_limits.limit_id` — tier signal.
- Companion local DB `~/.codex/state_5.sqlite` → table `threads` has `tokens_used` per thread (running total, grows with context). It's quick for "how many threads ran today" / "what models did I use" but does NOT carry the per-turn usage breakdown — use the rollout JSONLs for that.
- The id_token JWT in `~/.codex/auth.json` → `tokens.id_token` decodes (base64url, no signature check needed for inspection) and reveals the plan tier, renewal date, account id, user id, organizations. **Do this read locally** — it costs nothing and is fully authoritative for "which plan am I on".

## Plan tier & renewal — from the id_token
The `id_token` payload has an `https://api.openai.com/auth` object with:
- `chatgpt_plan_type` (`plus`, `pro`, `team`, `enterprise`, …)
- `chatgpt_subscription_active_start`, `chatgpt_subscription_active_until` (ISO 8601)
- `chatgpt_subscription_last_checked`
- `chatgpt_account_id`, `chatgpt_user_id`
- `organizations[]` (`id`, `title`, `role`, `is_default`)

Decoding recipe:
```python
import json, base64
hdr, pld, sig = json.load(open('/home/arctic/.codex/auth.json'))['tokens']['id_token'].split('.')
b64 = lambda s: json.loads(base64.urlsafe_b64decode(s + '=' * (4 - len(s)%4)))
claims = b64(pld)
print(claims['https://api.openai.com/auth']['chatgpt_plan_type'])         # 'plus'
print(claims['https://api.openai.com/auth']['chatgpt_subscription_active_until'])
```

## Live diagnostic recipe (what worked 2026-07-27)

```python
import json, glob, datetime as dt
from collections import defaultdict

day = defaultdict(lambda: {'turns':0,'in':0,'out':0,'cache_in':0,'reason':0})
latest = {}

for fp in sorted(glob.glob('/home/arctic/.codex/sessions/**/rollout-*.jsonl', recursive=True)):
    d = fp.rsplit('/',1)[-1][8:18]            # '2026-07-27'
    with open(fp) as f:
        for line in f:
            try: o = json.loads(line)
            except: continue
            if o.get('type') != 'event_msg': continue
            p = o.get('payload') or {}
            if p.get('type') != 'token_count': continue
            tu = (p.get('info') or {}).get('last_token_usage') or {}
            rl = p.get('rate_limits') or {}
            try:
                it = int(tu.get('input_tokens') or 0)
                ot = int(tu.get('output_tokens') or 0)
                ci = int(tu.get('cached_input_tokens') or 0)
                rr = int(tu.get('reasoning_output_tokens') or 0)
            except: continue
            if it+ot == 0: continue
            r = day[d]
            r['turns']+=1; r['in']+=it; r['out']+=ot
            r['cache_in']+=ci; r['reason']+=rr
            prim = (rl.get('primary') or {})
            if prim.get('used_percent') is not None:
                latest[d] = (
                    prim['used_percent'],
                    prim.get('window_minutes'),
                    prim.get('resets_at'),
                    (rl.get('credits') or {}).get('unlimited'),
                    (rl.get('credits') or {}).get('has_credits'),
                    rl.get('limit_id'),
                )
```

That gives you (1) per-day turn count + per-turn token deltas, (2) cache hit rate, (3) the last rate-limit snapshot seen that day (which is the one you want to surface as "current").

## Real snapshot (verified on this machine, 2026-07-27)

- Plan: **ChatGPT Plus**, renewal 2026-11-04 (from id_token `chatgpt_subscription_active_until`)
- Window: **10080 min (7 days)**, current reset **2026-07-28 17:03 UTC** (~1 day out)
- **Used now: 38.0%** (peak 99% on 2026-07-21)
- `unlimited: false`, `has_credits: false`, `limit_id: codex`
- Last Codex rollout on disk ends 2026-07-23 21:04 IST; nothing newer.
- 72 rollouts total; 3,017 token_count events; cache-hit rate ~95%.
- Big burn days (peak used_percent that day): 2026-07-20 = 100%, 2026-07-21 = 99%. Lighter weeks sit at 30–47% peak.

## Pitfalls
- **`tokens_used` in `state_5.sqlite.threads` is a cumulative context counter**, not a per-turn total — it's the size of the conversation, which keeps growing as the thread continues. Do NOT sum it across threads thinking you got "tokens spent". Use the rollout JSONLs.
- **`last_token_usage` per turn is the delta for that turn only**, but the `token_count` event fires on every incremental snapshot Codex takes (often several per turn). Summing them over a session gives you "tokens consumed in this turn" because each event reports the latest usage — but if you sum across `total_token_usage`, you get cumulative-per-session, which you should NOT also sum across turns. Pick one of `last_token_usage` (sum) or `total_token_usage` (last value).
- **`reasoning_output_tokens`** is the right key on Codex rollouts, not `reasoning_tokens`. The OTel logs in `~/.codex/logs_2.sqlite` use `codex.turn.reasoning_tokens` (no `_output_`). Different naming conventions in different places.
- **Day bucketing via filename is more reliable than via line timestamp** — rollouts can have warm-up lines that start at the previous day, while the filename tells you when the session opened.
- **`auth.json` may have OPENAI_API_KEY = None** when auth is ChatGPT-OAuth. Don't try to read a key that isn't there.
- **Cache hit rate is wildly inflated** because Codex's system prompt + skills stack is the repeated prefix; on a heavy day 95%+ of input tokens are cache_reads. Useful as a "the cache is doing real work" signal, NOT as a proxy for spend.
- **No live API for usage means you cannot read this from a different machine** — `auth.json` and the rollouts are local. If the user wants a remote view, the only authoritative source is `chatgpt.com/codex` in the browser (where the OAuth cookie is valid).
- **Don't confuse `window_minutes=10080` (Plus) with `43200` (Pro/Team — 30 days)**. The window-minutes value is itself the tier signal if you can't decode the JWT.

## What to tell the user
1. Lead with the percent-used + days-to-reset, not the raw token numbers.
2. Always label the source: "from local Codex rollout files — same `token_count` events the Codex UI uses, since neither the OpenAI platform API nor `chatgpt.com/backend-api` accept the CLI's OAuth bearer from outside a browser."
3. Mention the peak that week so they know how close they came to the cap.
4. If they want a remote/console view: `https://chatgpt.com/codex/settings/usage` (or `Settings → Codex → Usage` in the ChatGPT web app).