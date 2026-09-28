---
name: nous-inference-probe
description: "Probe the Nous Portal inference endpoint (inference-api.nousresearch.com/v1) to check live rate limits, model pricing/context, free vs paid status, and compare models — without burning quota. For Hermes users on the nous provider."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos]
tags: [nous, hermes, rate-limit, free-tier, model-comparison, inference-api]
---

# Nous Inference API Probing

Check your live rate limits, model metadata, and free/paid status on the Nous
inference endpoint — read-only where possible, negligible cost otherwise. Use it
to answer "how much free usage do I get?", to compare two models before
switching, or to diagnose a 429 throttle / 404 "requires available credits".

## When to use
- "How much free usage do I get?" — live rate limits on `tencent/hy3:free` or any free model.
- Compare two models (pricing, context, benchmarks) before switching.
- Diagnose a 404 `requires available credits` or a 429 throttle.
- Verify a Nous Portal paid plan is actually active for a given model.

## Prerequisites
- Logged into Nous Portal: `~/.hermes/auth.json` has a `nous` provider entry
  (`hermes portal info` shows `Auth: ✓ logged in`).
- Token at `providers.nous.access_token` (or `agent_key`, the scoped
  `inference:invoke` JWT). Read it **locally from the file**; never echo secrets to chat.

## Method

### 1. Model metadata (read-only, $0)
GET `/v1/models` returns the full catalog with `pricing`, `context_length`,
`top_provider.max_completion_tokens`, `reasoning`, and `synthesizedFreeVariant`.
No auth needed in practice, but send the Bearer token to be safe.
```bash
curl -s https://inference-api.nousresearch.com/v1/models -H "Authorization: Bearer $TOK" -o models.json
```
Free models are flagged `"synthesizedFreeVariant": true` with `"pricing": {"prompt":"0","completion":"0"}`.

### 2. Live rate-limit headers (1 token, $0 on free model)
**GET `/v1/models` does NOT return rate-limit headers.** You must hit a
generation endpoint. Use a 1-token POST so it costs essentially nothing:
```bash
curl -s -D - -o /dev/null https://inference-api.nousresearch.com/v1/chat/completions \
  -H "Authorization: Bearer $TOK" -H "Content-Type: application/json" \
  -d '{"model":"tencent/hy3:free","messages":[{"role":"user","content":"ping"}],"max_tokens":1}'
```

### 3. Read the headers
- `x-ratelimit-limit-requests` / `-1h`, `x-ratelimit-limit-tokens` / `-1h`
- `x-ratelimit-remaining-*` (left in the current window)
- `x-ratelimit-reset-*` (seconds until the short-window counter resets)
- `x-nous-credits-*` — the **paid** credit envelope: `paid-access`,
  `remaining-usd`, `subscription-limit-usd`, `disabled-reason`.

## Key facts & pitfalls
- **Rate-limit headers are account/provider-scoped, NOT per-model.** Probing
  `tencent/hy3:free` and `stepfun/step-3.7-flash:free` returned the *identical*
  bucket (50 req/40s, 2100/hr) — you cannot double free usage by alternating
  free models; they draw from one shared pool.
- **A 404 `Model '...' requires available credits. Your account balance is too
  low...`** means the paid credit envelope is empty for that model even if you
  believe you have a plan. Check `x-nous-credits-paid-access` and
  `x-nous-credits-remaining-usd`. The $0.10 starter credit envelope shows
  `disabled-reason: out_of_credits` on the free tier.
- **Free tier is throttled, not quota'd per se** — the `x-ratelimit-*` table is
  the real ceiling (~75 req/min burst, ~35/min sustained, ~6M tokens/hr on the
  free pool). Heavy parallel subagent sweeps WILL 429.
- **Don't persist one-off probe scripts** unless the user asks for a reusable
  artifact. Deliver the answer inline; the technique is three copy-paste curl
  lines. (User explicitly had a probe script removed this session — answer
  first, ship a tool only on request.)

## References
- `references/nous-rate-limit-headers.md` — observed header set, free-tier
  numbers, model comparison table (hy3:free vs hy3 vs minimax-m3 vs
  step-3.7-flash:free), and the 404 message text.
