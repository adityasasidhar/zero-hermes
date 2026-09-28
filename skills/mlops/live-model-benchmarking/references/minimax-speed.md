# MiniMax model speed — first-party Token-Plan reality (verified 2026-07-18)

## The wrong-name trap
- There is **no** `MiniMax-M2.7-Fast`. The low-latency variant is **`MiniMax-M2.7-HighSpeed`**.
- Requesting the wrong name returns `HTTP 400 {"type":"error","error":{"type":"bad_request_error","message":"invalid params, unknown model 'minimax-m2.7-fast' (2013)","http_code":"400"}}`.
- Other real first-party names seen: `MiniMax-M3`, `MiniMax-M2.7` (standard),
  `MiniMax-M2.5`, `MiniMax-M2.1`, `MiniMax-M2` (cached-fields differ per family).

## Live measurement on THIS user's $20 Token-Plan (Plus) — first-party endpoint
Method: 400-tok `max_tokens` completion, same ~210-tok prompt, `urllib` wall-clock
(includes prefill). Single cold samples — direction, not precision.

| Model | tok/s | cached prompt tok | note |
|---|---|---|---|
| MiniMax-M3 | **67.1** | 128 / 210 | fast prefill (cache hit) |
| MiniMax-M2.7-HighSpeed | **26.2** | 0 / 75 | cold, no cache |

**Takeaway:** on first-party MiniMax infra, M3 was ~2.5x faster than M2.7-HighSpeed.
The "HighSpeed" naming did NOT deliver speed over M3 here.

## Why reseller "100 tok/s" claims are misleading for this user
- The "~100 tok/s" / "~60 tok/s standard" figures come from **AI/ML API's blog** and
  Artificial Analysis' *third-party-host* benchmarks (Together AI, Fireworks,
  Novita-FP8, SambaNova). Those are optimized hosts.
- MiniMax's own **Token-Plan endpoint is the slow lane** — multiple reports (incl. a
  Reddit `r/MiniMax_AI` thread) say first-party "HighSpeed" feels false-advertised.
- Artificial Analysis' MiniMax-provider page: M3 is itself ~86 tok/s on good infra and
  is one of their *fastest* first-party models — so M2.7-HighSpeed has little headroom
  over M3 even in ideal conditions.
- To get M2.7-HighSpeed's advertised throughput you must route via a **third-party host**
  (OpenRouter / Fireworks). That is PAY-AS-YOU-GO billing, NOT the Token-Plan quota.

## Measurement caveats to state when reporting
- Single cold samples vary +/-20% on shared pools; report direction, not precision.
- `urllib` wall-clock includes prefill — a cached prefix (M3's 61% hit) gives a big
  prefill head start that inflates apparent decode speed. To compare decode speed
  fairly, warm the cache or report prefill vs decode separately.
- Never write the probe into a project repo — use `/home/arctic/hermes`.
