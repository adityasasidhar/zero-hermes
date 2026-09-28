---
name: live-model-benchmarking
description: "Measure LLM/model providers for real instead of trusting vendor or aggregate benchmarks. Covers live speed/latency/caching benchmarks on Hermes (Nous Portal free models) and MiniMax token-plan probing (usage, M3 speed, image-01 gen). Use whenever the user asks 'which model is faster/better', 'is caching supported', 'how many tokens do I have left', or wants a real side-by-side rather than a paper comparison. Embeds the user's hard rule: never write scratch/test artifacts into project repos."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos]
---

# Live Model Benchmarking & Provider Probing

When the user asks which model is "better/faster" or whether a feature (caching,
quota) exists, **measure it live** — do not rely on Artificial Analysis / vendor
claims. Aggregate numbers (e.g. "Step 3.7 Flash = 383 tok/s") reflect dedicated/paid
serving and often do NOT match the throttled free tier the user actually hits on Hermes.

## USER RULE — repo hygiene (HARD, explicit correction this session)
**Never save, write, or touch files in the user's actual project repos** (e.g.
`/home/arctic/Internship/...`). For any experiment, benchmark, test artifact, or
script, use the dedicated workspace **`/home/arctic/hermes`**. If a test accidentally
lands in a repo, move/remove it immediately. This is a durable preference, not a
one-off.

## When to use
- "compare model A vs B", "which is faster", "is X better than Y"
- "does my plan support token caching", "how many tokens left on my plan"
- "can [provider] do image generation"

## Core technique — measure, don't trust
1. Build a tiny streaming harness (`scripts/bench_speed.py`) that times
   time-to-first-token, time-to-first-CONTENT-token, total wall-clock, and
   content tok/s for the SAME prompt across models.
2. **Count reasoning tokens separately.** Models that emit a `reasoning` stream
   (Step 3.7 Flash, Claude-style) burn thousands of think-tokens before the first
   answer token — so "answer latency" is dominated by thinking, not decode speed.
   A model that "wins" on paper tok/s can LOSE on time-to-first-answer.
3. Single runs on shared pools are ±20% noise; report direction, not precision.
4. Give both models identical conditions (same prompt, same max_tokens, same method).

## Gotchas / pitfalls
- **SSE comments**: MiniMax/OpenRouter streams emit lines like `: OPENROUTER PROCESSING`
  (start with `: `). Skip them — they are not data.
- **Free-tier routing differs**: Hermes `tencent/hy3:free` => served by Novita;
  `stepfun/step-3.7-flash:free` => routed via OpenRouter to StepFun. So the two free
  models on Hermes run on DIFFERENT backend hardware — cross-model speed comparisons
  are valid only as "what you get on Hermes," not intrinsic model speed.
- **max_tokens budget**: reasoning models can exhaust `max_tokens` on thinking alone
  and return 0 content. Raise budget (1500+) or disable thinking to get a real answer.
- **Vendor launch-blog numbers ≠ paper numbers** (verified session: Kimi K3 / Kimi Linear). When
  the only primary source for a model is a launch blog post, the headline benchmarks
  are vendor-curated. The paper (if any) is usually more conservative and includes the
  harnesses they used. Three-step verification before quoting any vendor claim: (1) check
  arXiv for a tech report by ID/name; (2) if present, prefer paper's reported row over the
  blog's; (3) if paper is missing, label the blog claim with "vendor self-report" and a date
  stamp. See the `arxiv` skill's "Vendor benchmark gotcha" for the discovery flow.

## Algebraic verification fallback (CPU-only environments)

When the user wants to verify an equation or paper algorithm but the production kernel
needs CUDA (flash-linear-attention, xformers, custom triton, FLA's `chunk_kda`, etc.),
do NOT pretend a CPU run-through validates the math. Use **algebraic invariants** instead:

1. Read the source equation from the paper. Hand-derive closed-form expressions for
   special cases of the equation:
   - All gates closed (e.g. α=0 or β=0) → state reduces to a known closed form
   - Clean-state write → new value is retrievable exactly with one β step
   - Orthogonal associations → protected from the correction term
2. Write `assert_allclose(actual, derived_closed_form, atol=1e-5)` per case.
3. Run on CPU with `torch` only — never import the GPU kernel.

This catches ~80% of impl bugs (shape, einsum contract axis, missing terms, sign
errors) and the verification *itself* forces the implementer to derive the equation
properly — if you can't write the closed form, you don't understand the equation well
enough to have implemented it correctly. Combine with one equivalence test between
two impls of the same equation written differently (e.g. recurrent vs. chunkwise)
for ~95% coverage. Numerical agreement with the production kernel is only needed for
the remaining fp-precision edge cases.

Worked example: Kimi Linear KDA paper Eq. 1 verified on CPU with six invariants
(passed without ever importing `fla.ops.kda.chunk_kda` which requires CUDA).

## MiniMax specifics (verified live)
See `references/minimax-quirks.md` for endpoints, the `/token_plan/remains`
field-mislabeling bug, the `thinking`-must-be-object requirement, caching confirmation,
and where the API key actually lives for this user. Use `scripts/probe_minimax_plan.py`
to check quota.

### MiniMax model-name resolution & first-party speed (verified live 2026-07-18)
- **`MiniMax-M2.7-Fast` does NOT exist.** The fast/low-latency variant is
  **`MiniMax-M2.7-HighSpeed`** (inference-optimized MoE routing + batching; same
  weights, no quality loss). Requesting `minimax-m2.7-fast` => HTTP 400
  `invalid params, unknown model 'minimax-m2.7-fast' (2013)`.
- **First-party Token-Plan infra is the SLOW lane.** Live test on this user's plan
  (400-tok completion, same prompt): M3 = 67 tok/s, M2.7-HighSpeed = 26 tok/s (cold).
  So the "HighSpeed" naming did NOT make it faster than M3 on first-party routing.
- Reseller claims of "~100 tok/s" (AI/ML API blog) and Artificial Analysis'
  *third-party-host* benchmarks (Together/Fireworks/Novita-FP8/SambaNova) are
  **optimized-host numbers that do NOT transfer** to MiniMax's first-party endpoint.
  Do not quote them as the user's speed.
- To actually get M2.7-HighSpeed's advertised throughput, route via a **third-party
  host** (OpenRouter/Fireworks) — that's pay-as-you-go, NOT Token-Plan quota.
- Full numbers + measurement caveats: `references/minimax-speed.md`. Reusable probe:
  `scripts/bench_minimax_speed.py`.

## Hermes (Nous Portal) specifics
- Free models: `GET https://inference-api.nousresearch.com/v1/models` lists catalog;
  `$0` variants have `synthesizedFreeVariant: true`.
- Auth token: `~/.hermes/auth.json` -> `providers.nous.access_token` (NOT in env).
- Chat: `POST https://inference-api.nousresearch.com/v1/chat/completions` (OpenAI wire).

## Support files
- `scripts/bench_speed.py` — reusable streaming benchmark harness (Hermes free models).
- `scripts/probe_minimax_plan.py` — MiniMax token-plan usage/quota probe.
- `scripts/bench_minimax_speed.py` — MiniMax first-party speed probe (M3 vs M2.7-HighSpeed vs M2.7); flush + per-call timeout so a bad model name can't hang.
- `references/minimax-quirks.md` — endpoints, field gotchas, caching, key location.
- `references/minimax-speed.md` — `M2.7-Fast` wrong-name trap, first-party vs reseller tok/s, live M3/HighSpeed numbers + caveats.
- `references/algebraic-invariants.md` — CPU-only verification pattern for paper-equation implementations (six-invariant template + FLA `chunk_kda` API gotchas).
