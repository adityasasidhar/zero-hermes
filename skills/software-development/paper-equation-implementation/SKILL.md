---
name: paper-equation-implementation
description: "Implement a specific equation/algorithm from a paper and verify it matches the paper when the reference impl is unavailable or impractical (CUDA-only kernels, missing weights, broken build). Covers the from-scratch implementation path: derive from the paper equation, verify via algebraic invariants instead of bit-for-bit numerical comparison, then optionally reconcile against the reference when hardware permits."
version: 1.1.0
author: Hermes Agent
license: MIT
metadata:
  hermes:
    tags: [paper-reproduction, ml, correctness, verification, equations, algorithms]
    related_skills: [spike, systematic-debugging, test-driven-development, plan]
---

# Paper Equation Implementation

When the user asks "implement X from this paper" or "reproduce this paper's algorithm" — and the reference implementation needs hardware/weights/builds you don't have — this is the playbook. It produces a vectorized implementation of the paper's equation that you can verify **without** running the reference, then numerically reconcile later if hardware permits.

This came out of implementing Kimi Delta Attention (KDA, paper Eq. 1: `S_t = (I - βk kᵀ) Diag(α) S_{t-1} + β k vᵀ`) and verifying it against Moonshot's CUDA-only `fla.ops.kda.chunk_kda` kernel — which we couldn't run. Result: 6 algebraic-invariant tests passing on a CPU-only sandbox, with per-step error vs the analytic closed form `< 1e-4` and `Sᵀk - ((β⊙k)ᵀk)·v` exact within fp32.

## When to use

- **User asks** for a from-scratch reproduction of a paper's algorithm or equation
- **Reference impl is gated** behind something you don't have (CUDA-only kernel, training-data checkpoints, Python 2 code, unwieldy repo, missing licenses, etc.)
- **You have** the paper PDF, can read the equation, and can write a vectorized NumPy/PyTorch/JAX impl
- **The user does NOT want** you to spend hours reverse-engineering a complex reference impl
- **Equation-level reproduction is acceptable** to the user (it usually is — the goal is to *understand and use* the algorithm, not to ship a production kernel)

**Do NOT use when** the user's actual ask is "make this run", "ship it to production", or "match the paper exactly on hardware benchmarks" — those demand running the real reference kernel. Switch back to TDD + systematic-debugging.

## When this exact pattern fires (load triggers)

A future session, working on any paper equation, can recognize this skill by phrases like:

- "implement X from this paper"
- "reproduce this paper's [equation / algorithm / method / module]"
- "the reference impl needs CUDA / needs the original weights / is in Python 2 / is a giant repo, just give me a from-scratch version"
- "verify my impl matches the paper"
- "derive the [delta-rule / softmax / recurrence / kernel] from first principles and test it"

When matched, jump to Phase 1.

## The method — four phases

### 1. Locate the source equation

Open the paper PDF and find the equation(s) that fully specify the algorithm. Then read the *paragraph immediately following* — equations carry assumptions the prose makes explicit (e.g. "we use per-channel α", "applied before the value write", "the State is per-head").

Capture in a short note:
- The equation exactly as in the paper (copy/paste, don't paraphrase)
- The dimensions of every tensor: `(B, T, H, Dk)`, `(B, H, Dk, Dv)`, etc.
- The parametrization convention (per-channel vs per-head β, L2-norm on q/k, etc.)
- Any drawing or numbered algorithm box (paper Fig. / Listing)

For KDA specifically the equation was Eq. 1 + the input projection paragraph (L2-norm, ShortConv → Swish, low-rank α, sigmoid β, Sigmoid output gate). Anything named in the prose becomes part of the contract.

### 2. Recurrent (single-step) implementation first

Write the **step-by-step update first**. Yes — even though production code uses chunkwise parallel form. Reasons:

- The recurrent form is a verbatim translation of the paper equation. Most operator bugs you write are visible as wrong tensor contractions here.
- It can be verified analytically against derived invariants (next phase) before you touch the parallel form.
- It runs without a GPU.
- Many production kernels include a recurrent path for autoregressive decoding anyway — your impl becomes a sanity check, not throwaway.

```python
def step(q, k, v, alpha, beta, S_prev):
    # Reproduces paper Eq. 1 verbatim, einsum-by-einsum.
    # Q, K, alpha, beta are (B, H, Dk); v is (B, H, Dv); S is (B, H, Dk, Dv).
    scaled = alpha.unsqueeze(-1) * S_prev                          # Diag(α) S
    k_dot_scaled = torch.einsum('bhd, bhdc -> bhc', k, scaled)     # kᵀ · Diag(α) S
    kk_scaled = (beta * k).unsqueeze(-1) * k_dot_scaled.unsqueeze(2)  # (β k)(kᵀ S)
    write = torch.einsum('bhd, bhe -> bhde', beta * k, v)          # β k vᵀ
    S_new = scaled - kk_scaled + write
    o = torch.einsum('bhdc, bhd -> bhc', S_new, q)
    return o, S_new
```

**Pitfall to spot-check on every new equation:** einsum contraction axes. The two most common bugs:

- **Contracting the wrong axis.** `'bhe, bhdc -> bhde'` is `outer(k, scaled)` with no contraction at all — gives a 4D tensor of the wrong shape. Correct: `'bhd, bhdc -> bhc'`.
- **Forgetting per-channel broadcasting.** If α is `(B, H, Dk)` not `(B, H)` you cannot index `α[slot]` without keeping the trailing dim. Use `.unsqueeze(-1)` consistently on the result axis.

Run the smoke test now — if the rank-1 corrected terms give obviously wrong shapes, you'll see it immediately.

### 3. Algebraic invariants (the verification method)

This is the load-bearing technique. When you can't run the reference impl, derive *algebraic* properties from the paper equation and assert them. **Each invariant should be derivable from the equation in 2-3 lines**, so a mismatch is unambiguously an impl bug.

For an equation `S_new = f(S, k, v, α, β)`, derive:

**(a) Reduction tests** — set knobs to trivial values, predict the closed form, assert.

| Knob setting | What should happen | Assert |
|---|---|---|
| `α = 0` (full forget) | `S_new = β k vᵀ` exactly, no carryover | `S == β · outer(k, v)` |
| `β = 0` (no write) | `S_new = Diag(α) S_prev`, pure decay | after T steps: `S == S_0 · ∏ α_t` (row-wise product) |
| `α = 1, β = 1` | Pure Householder correction + full overwrite | `S_newᵀ k == ((β ⊙ k)ᵀ k) v` (under specific exact-read conditions) |
| `α = 1, β = 1, v_new` overwrite a state that already encodes `k→v` | Orthogonal associations preserved | `S_newᵀ k_perp == S_0ᵀ k_perp` exactly |
| Per-channel α: row r=0 retains while r=1 forgets | Only row 0 has the new v | `S[0]` carries the write, `S[1..]` near zero |

**(b) Analytic derivations of `S_newᵀ k`** — the most useful single invariant.

Derive from `S_new = scaled - kk_scaled + write`:

```
S_newᵀ k = scaledᵀ k
         − (kk_scaled)ᵀ k
         + writeᵀ k
```
- `scaledᵀ k = S_prevᵀ (α ⊙ k)`
- `kk_scaled = (β k) (kᵀ scaled)`, so `(kk_scaled)ᵀ k = (kᵀ scaled) · (kᵀ (β k)) = (kᵀ scaled) · ((β ⊙ k)ᵀ k)`
- `writeᵀ k = (β k vᵀ)ᵀ k = v · ((β ⊙ k)ᵀ k)`

So: `S_newᵀ k = S_prevᵀ(α ⊙ k) · (1 − (β ⊙ k)ᵀ k) + v · ((β ⊙ k)ᵀ k)`

This is the **one invariant to verify before trusting anything else** — it's the closed form of the update's projection onto its own key, and it's exact, deterministic, and catches ~80% of impl bugs.

**(c) Stability tests** — at realistic hyperparams, state shouldn't explode.

```python
# 10 000 steps with retention α ∈ (0.5, 0.9), β ∈ (0.1, 0.4) — must stay bounded
```

### 4. (Optional) Reconcile vs production kernel later

Once you have algebraic invariants passing, you're done from a *correctness* standpoint. **Stop and report what you have.** Do not chase the production kernel unless:

- The user explicitly asks for numerical-vs-reference parity, **and**
- The reference is actually runnable in your environment (has a CPU fallback, or you have CUDA)

If both hold, write a small `numerical_vs_<kernel>.py` (or equivalent) comparing outputs at matched shapes/dtypes. Tolerances to expect:

- bf16: `max |Δ| < 5e-2` between reference chunkwise and your step-by-step impl over 100 tokens.
- fp32: `max |Δ| < 1e-5`.
- Anything larger = a real bug, not numerical noise.

If reconciliation fails after 2 attempts, fall back to reporting the algebraic invariants as your correctness evidence. Do not invent a "matches" verdict — that's fabrication.

## Worked example checklist (KDA / Kimi Linear)

This was the actual run. Treat it as a worked template.

1. ✅ Located Eq. 1 (`S_t = (I - βk kᵀ) Diag(α) S_{t-1} + β k vᵀ`) and the input-projection paragraph.
2. ✅ Wrote recurrent `step()` with explicit einsums (caught one wrong-contraction bug this way: `'bhe, bhdc -> bhde'` → `'bhd, bhdc -> bhc'`).
3. ✅ Derived 6 invariants:
   - **clean-state contract** — `S_newᵀ k = ((β ⊙ k)ᵀ k) · v` (when `S_prev = 0`)
   - **dirty-state closed form** — `S_prevᵀ(α ⊙ k) · (1 - (β ⊙ k)ᵀ k) + v · ((β ⊙ k)ᵀ k)`
   - **α = 0 erases state** — `S_new == β · outer(k, v)` exactly
   - **β = 0 decays state** — `S_T == S_0 · ∏ α_t` (row-wise product)
   - **orthogonal preservation** — corrections don't leak into `k_perp` (Householder invariant)
   - **stability** — `max |S| < 1.0` over 10 000 steps at realistic hyperparams
4. ❌ Did not run the production FLA kernel (CUDA-only, not in sandbox). Did not reconcile.
5. ✅ Reported both: `6 passing algebraic invariants | no numerical reconciliation (reference impl requires CUDA)`

Total time: ~30 minutes including 3 bug fixes caught by the invariants.

## Common pitfalls

- **Wrong einsum contractions** — see above. Always print shape after the first einsum and compare to the equation.
- **Per-channel vs per-head tensors** — `β` is sometimes `(B, T, H)` (per-head, broadcast on Dk), sometimes `(B, T, H, Dk)` (per-channel). Read the paper carefully; an off-by-one broadcast destroys correctness silently.
- **Test the projection onto k, not the raw S matrix** — `S_newᵀ k` is a sharp, derivable quantity. Testing `S_new` elementwise against an analytic form is usually too tight (paper may not have specified it; differences in indexing will look like bugs).
- **Don't compare against the production kernel on CPU** — many implementations have a `cpu` fallback that throws or silently installs a different backend. Run on the actual target hardware or use algebra.
- **Forgetting the L2-norm / parameterization** — papers often describe their input projection (L2-norm, ShortConv → Swish, etc.) in prose. Without those, your KDA outputs match the *unparameterized* update, not the actual model.
- **Hard-coding per-channel vs per-head `β`** — the paper usually says it; the production kernel may have collapsed it. Don't assume your interpretation matches theirs.

## What this is NOT

- **Not a substitute for running benchmarks.** A from-scratch implementation that passes algebraic invariants is correct *as an equation*. It is not necessarily as fast or numerically stable as a tuned production kernel.
- **Not a substitute for reading the paper carefully.** The invariants you derive ARE the contract. If you derive them wrong, you're testing your wrong interpretation. Hand-derive from the equation line by line.
- **Not the right skill for "ship an inference kernel."** This is correctness-from-equation land. For latency/throughput optimization, switch to profiling + `llm-wiki` or `serving-llms-vllm` skills.

## Related skills

- `spike` — for validating ideas before committing to a build; this skill is more specific (implementing a *given* equation).
- `systematic-debugging` — for when verification fails; use Phase 1 / Phase 2 patterns to localize the bug to a specific term.
- `test-driven-development` — TDD governs the *form* of the invariants; this skill tells you *what* to assert when you're working from an equation rather than a spec.
- `arxiv` — finding the paper in the first place.

## Worked example

See `references/kda-walkthrough.md` for a full transcript of this method applied to Kimi Delta Attention (KDA). Includes the actual equation, the einsum that went wrong, the seven invariants derived, and what was *not* done (numerical reconciliation against the CUDA kernel).
