# Hermes Model Speed Benchmark — Notes & Gotchas
(measured live on Nous Portal free tier, July 2026)

## Measured: Hy3:free vs Step 3.7 Flash:free (same 300-word MoE prompt)
| Metric | Hy3:free (via Novita) | Step 3.7 Flash:free (OpenRouter -> StepFun) |
| time to first token | 4.95s | 4.69s |
| time to first ANSWER token | 4.95s | 14.59s |
| total to complete answer | 12.4s | 16.0s |
| reasoning tokens before answer | none | ~2,800 |
| content decode speed | ~35 tok/s | ~30 tok/s |

## Takeaways
- Aggregate benchmarks (Artificial Analysis: Step 3.7 Flash = 383 tok/s) do NOT hold on Hermes free tier. The free pool is throttled; observed decode is ~30-35 tok/s for both.
- Step 3.7 Flash defaults to HEAVY reasoning on the `:free` variant. That thinking phase (~2,800 tokens) dominates latency and pushes the first *answer* token to ~14.6s, slower than Hy3's 5s.
- Hy3:free has no separate reasoning stream — it answers immediately. So on Hermes free, Hy3 delivers a complete answer faster in practice.
- The 383 tok/s speed only appears on the PAID tier (stepfun/step-3.7-flash, $0.20/$1.15 per M tokens) with reasoning disabled.

## Routing on free tier (from live probe of /v1/models)
- tencent/hy3:free -> served via Novita (provider field in stream)
- stepfun/step-3.7-flash:free -> routed through OpenRouter, provider "StepFun"
- Both are `synthesizedFreeVariant: true`, pricing 0/0.

## Gotchas for any future benchmark harness
1. Reasoning models emit a `reasoning` field BEFORE `content`. Measure both; "first token" != "first answer".
2. `max_tokens` caps the ENTIRE stream (reasoning + content). Use >=4000 or the answer comes back empty.
3. Skip SSE comment lines starting with `: ` (e.g. `: OPENROUTER PROCESSING`).
4. Single runs on a shared pool have ~+/-20% noise; direction is robust.
5. When the user asks "is model X faster on Hermes?" — measure on Hermes, do NOT cite OpenRouter/Artificial Analysis numbers; free-tier serving is a different hardware/throttle pool.
