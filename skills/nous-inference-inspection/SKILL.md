---
name: nous-inference-inspection
description: "Empirically determine Hermes's active inference provider, free-tier model status, zero-cost pricing, and rate-limit situation for the Nous inference backend — via live API probes, never by guessing from docs. Load when the user asks 'how much free usage do I get', 'am I on the free tier', 'why am I hitting 402', or 'what are my rate limits'."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
---

# Nous Inference Inspection

When a Hermes user asks about free-tier usage, limits, or provider status, **verify against the live API instead of guessing from the docs.** The Hermes docs describe Nous Portal as "subscription-based" but publish no explicit free-tier quota, and the free model's metadata carries no `per_request_limits`. Ground truth lives in the API.

This skill is a focused supplement to the bundled `hermes-agent` skill (which covers general config/providers); it captures the *empirical probing technique* for the Nous inference backend specifically.

## Quick status (no secrets, safe to run)
- `hermes portal info` — shows `Auth: ✓ logged in`, the active `provider`, and tool-gateway config. Prints no secrets.
- `hermes config` — shows resolved `model.default`, `provider`, and `base_url` (Nous = `https://inference-api.nousresearch.com/v1`).

## Probe the live model catalog (public, no auth)
`GET https://inference-api.nousresearch.com/v1/models` returns the full catalog as OpenAI-wire JSON.
- Zero-cost models have `"pricing": {"prompt":"0","completion":"0",...}`.
- Free variants are flagged `"synthesizedFreeVariant": true` — they are the free tier of a paid sibling (e.g. `tencent/hy3:free` ← `tencent/hy3`).
- `per_request_limits` is `null` for free models: **no published per-request cap number exists.**
- `max_completion_tokens` and `context_length` are real (hy3:free → 131072 / 262144).
- Run `scripts/probe_free_tier.sh` for a ready-made summary (also prints the auth-status check).

## The 402 / x402 payment gate
An unauthenticated chat request returns `HTTP 402` with header `x-402-accept: solana` and a JSON body whose `accepts[].maxAmountRequired` is `"0"` for free models. Meaning: the free model is genuinely **$0**, but access is gated behind your Nous Portal OAuth (or an x402 handshake). Once `hermes portal info` shows `Auth: ✓ logged in`, the free model works.

## Rate limits
- There is **no documented RPM/quota number** for the free tier.
- Limits are enforced on a shared, throttled pool; free variants are deprioritized under concurrency — parallel subagents / heavy workloads will hit `429`.
- To read your live per-account limits, inspect `x-ratelimit-*` response headers on an **authenticated** request. The JWT is managed by the Portal OAuth flow in `~/.hermes/auth.json`; prefer `hermes portal info` for status rather than hand-extracting the token. Recipe is in `scripts/probe_free_tier.sh` (commented).

## Benchmark response speed & latency (chat endpoint)
Aggregate benchmarks (Artificial Analysis, OpenRouter) **do not** reflect Hermes free-tier serving — the free pool is throttled and routes through different hardware (e.g. Step 3.7 Flash `:free` is served via OpenRouter→StepFun, Hy3 `:free` via Novita). **Measure live before claiming one model is "faster" on Hermes.** When the user asks "is model X faster on Hermes?", benchmark it — don't quote third-party speed numbers.

**Auth token:** `python3 -c "import json;print(json.load(open('/home/arctic/.hermes/auth.json'))['providers']['nous']['access_token'])"`. POST `application/json` to `https://inference-api.nousresearch.com/v1/chat/completions`, header `Authorization: Bearer <token>`, `stream: true`.

**Gotchas that will corrupt your numbers:**
- **Reasoning tokens come first.** Step 3.7 Flash (and other reasoning models) stream a `reasoning` field *before* `content`. Count both; your "time to first answer token" ≠ "time to first token".
- **`max_tokens` caps the WHOLE stream** (reasoning + content). Step's reasoning alone can eat 1500–2800 tokens, so a `max_tokens: 600` budget produces an answer with *zero content*. Use a large budget (e.g. 4000) or disable reasoning for a fair answer-latency test.
- **Skip SSE comments.** Lines starting with `: ` (e.g. `: OPENROUTER PROCESSING`) are keep-alives, not `data:`.
- **Free-tier decode speed is ~30–35 tok/s**, NOT the vendor's 383 tok/s. The speed advantage only appears on paid tiers with reasoning off.

Reusable harness: `scripts/bench_speed.py` (measures TTFT, time-to-first-content, total, reasoning vs content chars for a list of models). Measured results + routing notes: `references/benchmark-notes.md`.

## Pitfalls
- **Never claim a specific "N requests/day" figure** — none is published. State limits are dynamic/shared and only visible via authenticated response headers.
- Free model = **$0 cost, NOT unlimited throughput.** Heavy or parallel work should use a paid model (Nous Portal subscription or OpenRouter credits for priority + higher RPM).
- The `web_search` tool may be absent in some Hermes surfaces; use `browser_navigate` + `browser_console` instead of guessing. (Surface-specific — verify availability before relying on it.)

## Support files
- `scripts/probe_free_tier.sh` — safe, runnable probe: portal auth status + zero-cost model summary + rate-limit header recipe.
- `references/api-notes.md` — condensed JSON facts captured from a live probe (endpoints, record shape, example hy3:free record).
- `scripts/bench_speed.py` — streaming chat benchmark: TTFT, time-to-first-content, total wall-clock, reasoning vs content split. Handles SSE comments + reasoning tokens.
- `references/benchmark-notes.md` — live measured Hy3:free vs Step 3.7 Flash:free results, free-tier routing, and benchmark gotchas.
