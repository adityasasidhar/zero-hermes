# Research Idea Viability Dossier Template

> Canonical template for per-idea dossiers produced by `research-idea-triage` subagents. Paste the relevant sections into each subagent's brief. Each dossier is the unit of triage — one per idea — and the parent agent synthesizes them into a ranked plan.

---

## Dossier Format (13 fields, strict order)

```
=== DOSSIER: #<N> <Idea Name> ===

One-sentence reframed hypothesis:
<One falsifiable sentence. Replace any "how can we…" question with a claim.>

Interpretation / clarification (only if the idea phrase is ambiguous):
<List 2–4 candidate readings of the phrase, then pick one and say why.>

Why this matters now (2026 context):
<What in the literature makes this timely. Reference recent papers, community discussions, or tooling shifts.>

Closest prior work (with arxiv IDs — flag as "verify before citing" if not confirmed):
<5–10 papers max. Group by relevance, not chronologically. State the relationship to the user's idea: "direct competitor", "technique we borrow", "negative result that informs us", etc.>

What the user has already built (or "None"):
<Specific file paths, what's done, what's missing. If the user has a repo, name the model classes, training scripts, datasets, known blockers.>

The honest gap this paper could fill:
<One paragraph. The defensible niche. Why existing work doesn't close it.>

Feasibility on the user's machine (4 GB VRAM, 14 GB RAM):
<Model size, batch, seq-len, what fits in memory, what needs remote (Modal/RunPod). Concrete, not vague.>

Minimum experimental evidence to defend the claim:
<Numbered list of tables/figures the paper needs. State the metric thresholds (e.g. "AUC ≥ 0.75", "≥3 pp gain at matched budget").>

Compute + time estimate:
<GPU-hours, dataset size, wall-clock for the user. State what can run locally vs what needs remote.>

Specific risks (what kills the paper):
<Numbered list. For each risk, say how to detect it early and how to mitigate.>

Sharpened claim options — 2-3 candidate framings:
<Each option: a one-sentence claim, plus "what evidence would defend it". The user picks one.>

Venue fit:
<Conference + workshop. With deadline reality-check ("deadline ~Oct 2026" not "ICLR is open"). Note main-track vs workshop-track.>

Viability score (1-10):
<Number + one-line justification. Anchor against the rubric in the skill body.>

Recommendation:
<PURSUE / SHARPEN-THEN-PURSUE / DEFER / DROP — one word, plus one-line reason.>
```

---

## Scoring Anchors (use consistently across all dossiers)

| Score | Meaning |
|---|---|
| **8–10** | Strong defensible niche, engineering done or near-done, clear conference fit |
| **6–7** | Viable with sharpening — 1–2 weeks of code/pilot work to commit |
| **4–5** | Paper potential exists but crowded or blocker-unknown; sharpen or pilot first |
| **1–3** | Drop or defer — headline finding taken, or no defensible niche |

## Recommendation Anchors

| Verdict | Trigger |
|---|---|
| **PURSUE** | Claim is sharp, evidence achievable, venue open → start RPW Phase 0 |
| **SHARPEN-THEN-PURSUE** | Viable but needs claim refinement, pilot run, or baseline work first |
| **DEFER** | Not viable now (deadlines passed, missing infra, no clear angle) |
| **DROP** | Headline is taken, no defensible sharpening exists |

---

## Subagent Brief Template

Paste this into each subagent's `context` field, customizing the bracketed sections:

```
You are one of [N] parallel subagents producing viability dossiers for a list of research paper ideas. The user (Aditya Sasidhar, VIT New Delhi, AI/ML) wants to know which ideas are worth pursuing as papers and how to sharpen each into a defensible claim.

Machine constraints: RTX 3050 4 GB VRAM, 14 GB RAM, 12 cores. [Optional: He can use Modal for H100 training; RunPod MCP for ad-hoc remote GPUs.]

[CRITICAL CONTEXT — include ONLY when relevant:]
- Read these existing repos first: [list absolute paths + 1-line "what's there" description]
- Read these vault notes first: [list paths + 1-line description]

Your job: produce a structured viability dossier for idea #[N]: "[EXACT PHRASE FROM USER]".

Output format: follow the template in `research-idea-triage/templates/research-viability-dossier.md` exactly. 13 fields in order, no preamble, no postamble.

ID-fidelity rule: do NOT fabricate arxiv IDs. Pull from live web_search (4–6 calls covering different angles of the same topic). If you cannot verify an ID in search results, label it "verify before citing" — the parent will re-verify before any paper citation.

[Idea-specific brief, e.g.:]
"Internal cognition" in 2026 ML research likely refers to: [list candidate readings]. Resolve the ambiguity in the Interpretation field.
OR
"DDR" overlaps with the user's existing repo at /home/arctic/projects/Dynamic-Depth-Recurrence/. Read RESEARCH_DIRECTION.md first to see the prior review. Your job is to UPDATE that review with current literature and propose the next experimental step.
```

---

## Worked Examples

### Good — Sharp, Evidence-Backed

```
=== DOSSIER: #2 Dynamic Depth Recurrence ===

One-sentence reframed hypothesis:
At iso-param and iso-data, a per-token continuous ponder-halting looped transformer
with shared-KV decode beats a fixed-loop baseline on the compute/quality frontier,
but only if the per-token-KL router-collapse bug is fixed before any headline run.

What the user has already built:
- /home/arctic/projects/Dynamic-Depth-Recurrence/dynamic_dense/model.py: SharedKVAdaptiveBlock,
  PonderRouter (W2=0/b2=logit(λ) init), per-token halting with KL-to-geometric-prior
- training/train_dynamic.py:280 — KNOWN BUG: per-token KL penalty (should be batch-mean)
- Frozen 30M-token mixed30m corpus: FineWeb-Edu 0.70 / open-web-math 0.15 / OpenOrca-CoT 0.15
- Zero run artifacts currently exist (curves + bins deleted in 042ce86)

Closest prior work (verify before citing):
- arXiv:2507.10524 — MoR (NeurIPS 2025), discrete top-k, param-saving, can't answer iso-param
- arXiv:2502.05171 — Huginn-3.5B (ICLR 2025), depth-recurrent latent reasoning at 3.5B/800B-tok
- arXiv:2510.25741 — Ouro (Oct 2025), entropy-regularized learned depth (DIRECT design-space match)
- arXiv:2510.24824 — Parallel Loop Transformer (Oct 2025), shares KV-cache-from-loop-1 idea

The honest gap: nobody answers the isolated iso-param question (continuous halting vs fixed-loop
at matched params/data/compute) — MoR can't (discrete + param-saving), PonderLM/AdaPonderLM
sit close but compare to dense at fewer params. Aditya can carve this on the 14L/512/4KV scaffold.

Viability: 6/10 — engineering strong, headline run is $300 of Modal compute, but entry ticket
(router fix + FA3 correctness test) is still unbuilt and MoR/PonderLM-2/Ouro all touched this
axis in the last 12 months.

Recommendation: SHARPEN-THEN-PURSUE — apply batch-mean KL fix (30 min code), add FA3 vs SDPA
correctness test, run 3–5k-step Modal H100 pilot to confirm loops_var rises. If yes: launch
100 H100-hour iso-param sweep. If no: pivot to "shared-KV × halting interaction" mechanistic
study as a focused workshop paper.
```

### Bad — Vague, No Evidence

```
=== DOSSIER: #2 Dynamic Depth Recurrence ===

One-sentence hypothesis:
Dynamic depth recurrence is a cool idea that might work for papers.

Closest prior work:
PonderNet, Universal Transformers, maybe some newer stuff like Huginn.

Viability: 7/10 — could be a good paper.

Recommendation: PURSUE — looks promising.
```

**Why the bad one fails**: no reframing, no specific repos, no verifiable IDs, no compute estimate, no risks, no sharpened framings, no deadline reality-check. This is what happens when the subagent skips the inventory step and doesn't have a strict template.