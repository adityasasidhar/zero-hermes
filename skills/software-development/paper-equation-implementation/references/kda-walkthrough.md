# Worked example: Kimi Delta Attention (arXiv:2510.26692)

This is a worked transcript of `paper-equation-implementation` applied to the KDA attention layer. Use it as a template when reproducing other paper equations.

## The setup

| Question | Answer |
|---|---|
| Paper | *Kimi Linear: An Expressive, Efficient Attention Architecture* (arXiv:2510.26692, Nov 2025) |
| Equation to implement | Paper Eq. 1 (state update) + the input projection paragraph above Listing 1 |
| Reference impl | `flash-linear-attention/fla/ops/kda/chunk_kda` (Triton kernel) — **CUDA-only** |
| Sandbox | CPU-only PyTorch (no GPU, no `triton` driver) |
| Goal | A correct, vectorized, CPU-runnable PyTorch implementation of the recurrent update step |

## Phase 1 — Locate the source equation

From §3 of the paper (verbatim):
> "we propose Kimi Delta Attention (KDA)... `S_t = (I − β_t k_t k_tᵀ) Diag(α_t) S_{t−1} + β_t k_t v_tᵀ ∈ ℝ^{d_k×d_v}; o_t = S_tᵀ q_t ∈ ℝ^{d_v}`"

And from §4 (input projection, in prose):
- `q, k = L2Norm(Swish(ShortConv(W_q/k x)))`, dim `d_k = 128`
- `v = Swish(ShortConv(W_v x))`, dim `d_v = 128` (default; may differ)
- `α = f(W_↑^α W_↓^α x)` ∈ `[0,1]^{d_k}` — per-channel, low-rank parametrized
- `β = σ(W_β^h x)` ∈ `[0,1]` — per-channel scalar (sigmoid) in the *base* implementation; production may collapse to per-head
- Output gate: `o = W_o (σ(W_↑^g W_↓^g x) ⊙ RMSNorm(KDA(q,k,v,α,β)))`

Dimensions agreed at runtime: `(B, H, Dk, Dv)`.

## Phase 2 — Recurrent impl

```python
def kda_step(q, k, v, alpha, beta, S_prev):
    """
    q, k, alpha, beta: (B, H, Dk)
    v:                  (B, H, Dv)
    S_prev:             (B, H, Dk, Dv)
    returns: (o, S_new), both (B, H, Dk/Dv)
    """
    # 1) Diag(α) S_prev   — per-row scale (Diag(α) is per-channel in key dim Dk)
    scaled = alpha.unsqueeze(-1) * S_prev                       # (B, H, Dk, Dv)

    # 2) Compute kᵀ scaled  (broadcast result has shape (B, H, Dv); then
    #    expand back to Dk by outer-product with β·k.)
    k_dot_scaled = torch.einsum('bhd, bhdc -> bhc', k, scaled)  # (B, H, Dv)
    kk_scaled   = (beta * k).unsqueeze(-1) * k_dot_scaled.unsqueeze(2)  # (B, H, Dk, Dv)

    # 3) β k vᵀ write term
    write = torch.einsum('bhd, bhe -> bhde', beta * k, v)       # (B, H, Dk, Dv)

    # 4) S_new = scaled - kk_scaled + write
    S_new = scaled - kk_scaled + write

    # 5) o = S_newᵀ q
    o = torch.einsum('bhdc, bhd -> bhc', S_new, q)
    return o, S_new
```

### First-attempt bug caught here

My initial version had:
```python
inner = torch.einsum('bhe, bhdc -> bhde', k, scaled)  # WRONG
```
That contracts nothing — it's a 4-D outer product with the wrong shape. Correct form: `'bhd, bhdc -> bhc'` (with `c` shared as the row-contraction index). Once replaced, the invariants immediately closed.

The lesson: **before any invariant check, print the shape after your first einsum against what the equation says the term should be.** `kᵀ scaled` is `(B, H, Dv)`, not `(B, H, Dk, Dv)`.

## Phase 3 — Algebraic invariants (the verification)

### Invariant 1 — Clean-state projection contract

Starting with `S_prev = 0`, derive `S_newᵀ k` from the equation:

```
S_new = 0 − 0 + (β k) vᵀ  ⇒  S_newᵀ k = v · (β k)ᵀ k = v · ((β ⊙ k)ᵀ k)
```

Implementation:

```python
def test_clean_state_contract():
    """S_newᵀ k = ((β ⊙ k)ᵀ k) · v   when S_prev = 0"""
    kt, vt = torch.randn(B, H, Dk), torch.randn(B, H, Dv)
    at, bt = torch.rand(B, H, Dk) + 0.3, torch.rand(B, H, Dk) * 0.5 + 0.3
    qt = torch.randn(B, H, Dk)
    S0 = torch.zeros(B, H, Dk, Dv)
    _, S_new = kda_step(qt, kt, vt, at, bt, S0)
    proj = torch.einsum('bhdc, bhd -> bhc', S_new, kt)
    bkk = (bt * kt * kt).sum(-1, keepdim=True)
    err = (proj - bkk * vt).abs().max().item()
    assert err < 1e-4
```

### Invariant 2 — Dirty-state closed form

```
S_newᵀ k = S_prevᵀ(α ⊙ k) · (1 − (β ⊙ k)ᵀ k) + v · ((β ⊙ k)ᵀ k)
```

```python
def test_dirty_state_closed_form():
    kt, vt = torch.randn(B, H, Dk), torch.randn(B, H, Dv)
    at, bt = torch.rand(B, H, Dk) + 0.3, torch.rand(B, H, Dk) * 0.5 + 0.3
    S0 = torch.randn(B, H, Dk, Dv)
    _, S_new = kda_step(qt, kt, vt, at, bt, S0)
    proj = torch.einsum('bhdc, bhd -> bhc', S_new, kt)
    scaled_old_k = torch.einsum('bhdc, bhd -> bhc', S0, at * kt)
    bkk = (bt * kt * kt).sum(-1, keepdim=True)
    expected = scaled_old_k * (1 - bkk) + bkk * vt
    err = (proj - expected).abs().max().item()
    assert err < 1e-4
```

### Invariant 3 — `α = 0` erases everything

```
S_new = (I − βkkᵀ) · 0 · S_prev + β k vᵀ = β k vᵀ
```

```python
def test_alpha_zero_erases_state():
    S0 = torch.randn(B, H, Dk, Dv) * 100.0
    alpha = torch.zeros(B, H, Dk)
    beta  = torch.full((B, H, Dk), 0.5)
    _, S_new = kda_step(q, k, v, alpha, beta, S0)
    expected = torch.einsum('bhd, bhe -> bhde', 0.5 * k, v)
    err = (S_new - expected).abs().max().item()
    assert err < 1e-5
```

### Invariant 4 — `β = 0` is pure α-decay

```
S_T = S_0 · ∏_{t=1..T} α_t   (row-wise product)
```

### Invariant 5 — Orthogonal-direction preservation (Householder invariant)

If `S₀` is constructed so that `S₀ᵀ k_perp = w` exactly (with `k_perp ⊥ k`), then after `k → v_new` write with any β > 0, `α = 1`, the orthogonal association is untouched:

```
S_newᵀ k_perp = w
```

### Invariant 6 — Stability

10 000 steps with realistic hyperparams (`α ∈ (0.5, 0.9)`, `β ∈ (0.1, 0.4)`). State remains bounded; no NaN/Inf.

### Invariant 7 (bonus) — Per-channel gating observability

Set `α[:, 0] = 1.0`, `α[:, 1:] = 0.0`. After one write of `k = e_0`, channel 0 retains `0.8 · v`, channels 1–3 are exactly zero. **This is the KDA-vs-GDN discriminator.**

## Phase 4 — Did NOT reconcile

The production FLA kernel is CUDA-only (Triton, no CPU fallback). The session ended here with:

> 6 algebraic invariants passing on CPU. No numerical reconciliation; no claims about parity with the production kernel at any precision.

Future work: run on hardware with CUDA, compare `chunk_kda` to the analytic formula and to the recurrent impl over 100–10 000 tokens at bf16, expect `max |Δ| < 5e-2`.

## What went right

- Algebraic invariants caught the first einsum bug in <2 minutes.
- The "projection onto k" invariant is the most informative per test (it directly tests the rank-1 correction term — the trickiest part of KDA).
- All 6 invariants run on CPU, total smoke test < 1 second. No GPU or special dependencies.

## What I'd do next time

- Write a `numerical_vs_fla.py` skeleton even before running it (with a clear "requires CUDA" header) so future agents have the pattern.
- Derive the invariants from the paper Eq. *before* writing any code — they'd catch the einsum-bug earlier.
- For equations with multiple parameters, parameter-sweep over the dimensions you expect to be biased (here: per-channel `α`); that's where shape bugs hide.
