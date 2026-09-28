# Algebraic Invariants — Reference Patterns

When verifying a paper algorithm in environments without GPU/CUDA, use **algebraic
invariants** derived from the equation itself instead of numerical comparison against
a production kernel.

## Recipe (worked through: Kimi Linear KDA Eq. 1)

Paper Eq. 1 (KDA update):

```
S_t = (I − β k kᵀ) · Diag(α) · S_{t-1}  +  β k vᵀ
o_t = S_tᵀ q_t
```

Six invariants that fully characterize correctness, each runnable on CPU with `torch` only:

| # | Invariant | What it checks | Test pattern |
|---|-----------|----------------|--------------|
| 1 | Clean-state write: `S₀=0` ⇒ `S_post = β k vᵀ`, so `S_postᵀ k = ((β⊙k)ᵀk)·v` | einsum contract shape; per-channel β interpretation | assert `|| S^T k − (β⊙k)·k · v || < 1e-4` |
| 2 | Dirty-state full expansion: `S_postᵀk = S_oldᵀ(α⊙k) − [S_oldᵀ(α⊙k)·((β⊙k)ᵀk)] + [v·((β⊙k)ᵀk)]` | sign on the `(I − βkkᵀ)` term; α-vs-α⊙k distinction | assert exact closed form |
| 3 | α=0 ⇒ `S_post = β k vᵀ` (state fully erased each step) | the α gate actually applies to the carry term | assert equality to β k vᵀ exactly |
| 4 | β=0 ⇒ `S_T = S_0 · ∏α_t` (pure carry-over with decay) | β gate actually protects decayed state | row-wise product check |
| 5 | α=1 orthogonal preservation: write `(k, v_new)` does not leak into any `k⊥` direction previously encoded in `S₀` | the `(I − βkkᵀ)` correction only touches the k direction | encode `w_old` under `k⊥`, write `v_new` under `k`, assert `S^T k⊥ = w_old` |
| 6 | State magnitude bounded over 10K steps with stable hyperparams | no runaway amplification from contract not summing | `max(\|S\|) < threshold` |

All six pass ⇒ the implementation matches paper Eq. 1 to floating-point precision,
**without ever running the GPU kernel.**

A 7th, weaker but tasty:

| 7 | Per-channel gating visualised: encode (k, v) where k = e₀; set α = [1, 0, 0, 0]; expect exactly one row of S to be nonzero after the step | proves the per-channel α is the source of the expressivity gain over GDN | assert `S[1:, :]` ≈ 0, `S[0, :]` ≈ 0.8·v |

## Production-kernel API gotcha (`fla.ops.kda.chunk_kda`)

The signature looks like:
```python
chunk_kda(q, k, v, g, beta, ..., A_log=None, use_gate_in_kernel=False, ...)
```
Three pitfalls that cost tool calls to discover:

1. **`g` is the log-forget-gate** (i.e. `log α`, not `α`). It's used when
   `use_gate_in_kernel=False`. When `use_gate_in_kernel=True`, you must also
   pass `A_log` — and the value convention there is **the negative**:
   `A_log = -log(α) = g · (-1)`. Mixing these silently produces wrong outputs
   that "look" correct.

2. **`beta` is per-head `(B, T, H)`** in FLA's convention, NOT per-channel
   `(B, T, H, Dk)`. Your reference impl that uses per-channel β will need
   to broadcast `beta.unsqueeze(-1)` before passing in.

3. **Triton requires CUDA + `pip install triton`**. The CPU fallback path
   in `fla/ops/backends/__init__.py` errors with `RuntimeError: 0 active
   drivers ([]). There should only be one.` — verify your impl on CPU via
   invariants, then run numerical-vs-FLA on a GPU box.

## When this technique generalizes

Any time you have:
- a paper algorithm with explicit equations (update rules, recurrence, forward pass)
- an environment where the production kernel can't run (no GPU, missing dep, sandbox)
- you can derive closed-form expressions for special cases by setting some inputs to 0/1

…write the invariants as a `verify.py` smoke test before trying the heavy install.
You will catch:

- **Shape bugs** (the most common — wrong einsum axes or unsqueezes)
- **Sign errors** in update rules (especially around `(I - X)` correction terms)
- **Per-channel vs. per-head broadcasting** mistakes
- **Missing terms** in formulas (especially when copy-pasting from pseudocode that drops
  the bias or activation path)
- **Conjugate transposes** mistakes (real vs complex)

## Workflow

1. Read the paper's equations section (typically §3 in attention / SSM papers).
2. Hand-derive closed-form behavior for: clean state, all gates closed, ±∞ limit cases.
3. Implement naïvely, write the invariants.
4. Iterate until 6+ invariants pass.
5. **Then** and only then: install the production kernel and run numerical agreement.

A spike for an equation-implementation that doesn't ship a `verify.py` is incomplete.
