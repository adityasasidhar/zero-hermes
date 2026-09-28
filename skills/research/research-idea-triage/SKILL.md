---
name: research-idea-triage
description: "Rank a list of research ideas into a viability plan."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Research, Ideas, Triage, Viability, Papers, Planning, Brainstorm, ML]
    category: research
    related_skills: [research-paper-writing, arxiv, delegation-first, plan]
---

# Research Idea Triage

## Overview

When the user dumps a list of half-formed research ideas ("here are 6 things I'm thinking about, give me a plan"), this skill produces a **ranked viability plan** — one structured dossier per idea, then a consolidated ranking with concrete next steps for the top 2–3.

This is **upstream of `research-paper-writing`**: before you run Phase 0 (Project Setup) on any paper, you need to know which paper is worth pursuing. Triage answers that question. The skill composes with `delegation-first` (fan out per-idea dossiers in parallel) and `plan` (write the consolidated plan to disk).

## When to Use

Trigger conditions:

- The user pastes/drops a list of research paper ideas (3+ items) and asks for a "plan", "which one should I do", "viability check", or similar
- The user has a vault note titled something like `Research paper ideas.md`, `Brainstorm.md`, or `Things to try.md` containing multiple candidate directions
- The user asks "I have a bunch of ideas, which is worth pursuing?" without naming a specific paper
- A user proposal mentions multiple candidate topics and the goal is to **choose**, not to **execute**

Do NOT use when:

- The user has already chosen a paper and wants to execute — use `research-paper-writing`
- There's only one idea — proceed with `research-paper-writing` Phase 0 directly
- The ideas are not research-related (product features, side projects) — use `plan` instead

## Workflow

### Step 1: Inventory Existing Context

Before dispatching any subagents, check what the user already has:

```bash
# Vault searches (parallel)
search_files(path="/home/arctic/Documents/fun", pattern="<idea keyword>", target="content")
search_files(path="/home/arctic/projects", pattern="<idea keyword>", target="files")

# Existing repo reads — these become per-dossier context blocks
ls /home/arctic/projects/<relevant-repo>/
read_file("/home/arctic/projects/<repo>/CLAUDE.md")
read_file("/home/arctic/projects/<repo>/RESEARCH_DIRECTION.md")
```

**Why this matters**: A triage dossier that doesn't know the user already has `Dynamic-Depth-Recurrence/` will recommend building from scratch. A triage dossier that doesn't know there's a `wiki/aiml/concepts/long2short-rl.md` won't see that #6 GRPO is already half-codified. Always inventory first; pass the context into each subagent's brief.

### Step 2: Fan Out Dossiers in Parallel

Dispatch **one subagent per idea** in a single `delegate_task` batch call (not serially). Each subagent gets:

1. **Goal**: produce a structured viability dossier for this specific idea
2. **Context**: machine constraints (VRAM, RAM), existing repos to read, the rubric below
3. **Output format**: a strict template (see `templates/research-viability-dossier.md`)
4. **Hard rule**: do NOT fabricate arxiv IDs — pull them from real web_search and label candidates as "verify before citing"

**Rubric (the 13 dossier fields, in order):**

| Field | What it captures |
|---|---|
| One-sentence reframed hypothesis | A falsifiable claim, not a question |
| Interpretation / clarification | If the idea phrase is ambiguous, list the candidate readings and pick one |
| Why this matters now (2026 context) | What's happening in the literature that makes this timely |
| Closest prior work | With arxiv IDs from real search, labeled "verify before citing" if not confirmed |
| What the user has already built | Specific files, what's done, what's missing — empty if none |
| The honest gap the paper could fill | The defensible niche; why existing work doesn't close it |
| Feasibility on the user's machine | Concrete, not vague — model size, batch, seq-len, fit-in-memory |
| Minimum experimental evidence | What tables/figures the paper needs to defend the claim |
| Compute + time estimate | GPU-hours, dataset size, wall-clock — concrete numbers |
| Specific risks | What kills the paper; how to mitigate each |
| Sharpened claim options | 2–3 candidate framings the user could pick from |
| Venue fit | Conference + workshop, with deadline reality check |
| Viability score (1–10) | + one-line justification |
| Recommendation | PURSUE / SHARPEN-THEN-PURSUE / DEFER / DROP |

**Scoring anchors** (use these consistently):

- **8–10**: strong defensible niche, engineering is done or near-done, conference-fit is clear
- **6–7**: viable with sharpening; needs 1–2 weeks of code/pilot work to commit
- **4–5**: paper potential exists but crowded or blocker-unknown; sharpen or pilot first
- **1–3**: drop or defer; headline finding is taken, or no defensible niche

**Recommendation anchors:**

- **PURSUE**: claim is sharp, evidence is achievable, venue is open → start Phase 0 of `research-paper-writing`
- **SHARPEN-THEN-PURSUE**: viable but needs claim refinement, pilot run, or baseline work first
- **DEFER**: not viable now (deadlines passed, missing infra, no clear angle) — re-evaluate next cycle
- **DROP**: headline is taken and no defensible sharpening exists

**Brief template** (paste into each subagent's context, customizing for the specific idea):

```
You are one of N parallel subagents producing viability dossiers for a list of research paper ideas. The user (Aditya Sasidhar, VIT New Delhi, AI/ML) wants to know which ideas are worth pursuing as papers and how to sharpen each into a defensible claim.

Machine constraints: RTX 3050 4 GB VRAM, 14 GB RAM, 12 cores. He has repos at <list of relevant repos — read these first>.

Output format: a strict 13-field dossier per the template in templates/research-viability-dossier.md. Return ONLY the dossier, with no preamble.

ID-fidelity rule: do NOT invent arxiv IDs. Pull from live web_search; if you can't verify an ID in the search results, label it "verify before citing".
```

### Step 3: Synthesize the Consolidated Plan

In the parent agent, do **NOT** concatenate the dossiers — synthesize:

1. **Ranked summary table**: idea, score, recommendation, headline differentiator
2. **Mermaid flow**: ideas grouped into "main track candidates" vs "workshop-tier only"
3. **Recommended sequencing**: which idea to start with, what's the cheapest 1–2 week pilot, what's the longest-horizon bet
4. **Per-idea summary** with the 3–4 most actionable lines (not the full dossier — link to the full one)
5. **References-to-verify list** — every arxiv ID pulled from subagent search that wasn't 100% confirmed

**Output destination**: write the plan to the vault note the user gave you (e.g. `Reserach paper ideas.md`) AND link it from the relevant MOCs (typically `Projects.md` and `Research.md`).

### Step 4: Identify the Single Next Move

End the triage with **one** concrete, executable next action — the cheapest pilot that decides whether the top-ranked idea is real. Examples:

- "Apply the batch-mean KL fix in `Dynamic-Depth-Recurrence/train_dynamic.py:280` (30 min) → run 3–5k step Phase 0 on Modal H100 → decision in 1 day"
- "Operationalize the novelty score `η` in `the-deep-field/Learning_with_confidence/main.py` (gzip-length proxy, ~20 LOC) → run 1 seed on Llama-3.2-3B → decision in 2 days"

The user wants to start running, not to keep planning. Close with an offer to execute the next move.

## Common Pitfalls

1. **Skipping the inventory step.** Dossiers that don't know about the user's existing repos recommend wasted build-from-scratch work.
2. **Fabricating arxiv IDs.** Subagents under deadline pressure will invent plausible IDs. The brief MUST include the "verify before citing" rule and the parent MUST skim the ID list and flag suspicious ones.
3. **Concatenating instead of synthesizing.** Stacking 6 dossiers is not a plan. The parent must rank, sequence, and pick one next move.
4. **Sequential subagent dispatch.** Dispatch all per-idea dossiers in one `delegate_task` batch — they're independent.
5. **Letting the consolidated plan become a wishlist.** End with ONE next move, not a "you could also try X" epilogue.
6. **Confusing triage with research-paper-writing.** Triage ranks; RPW executes. Don't start drafting a paper until triage recommends PURSUE on a specific idea.
7. **Re-running triage on the same idea list.** If the vault note is unchanged, just re-read the existing plan — don't burn subagents again.

## Verification Checklist

- [ ] Inventoried existing repos and vault notes before dispatching
- [ ] Dispatched all per-idea dossiers in a single `delegate_task` batch
- [ ] Each subagent brief includes: machine constraints, repos to read, dossier template, ID-fidelity rule
- [ ] Consolidated output is a ranked plan, not concatenated dossiers
- [ ] Plan is persisted to the vault note the user provided
- [ ] Plan is linked from relevant MOCs (Projects.md, Research.md)
- [ ] Output ends with one concrete next-move offer
- [ ] All arxiv IDs flagged for re-verification before any paper citation

## Reference Files

| File | Contents |
|---|---|
| [`templates/research-viability-dossier.md`](templates/research-viability-dossier.md) | Canonical 13-field dossier template + subagent brief template + good/bad worked examples |

## Composes Well With

- `research-paper-writing` — this skill picks the paper; that skill writes it
- `delegation-first` — fan-out pattern for the per-idea subagents
- `arxiv` — subagents use arxiv search to verify and discover prior work
- `plan` — the consolidated output is a plan; can be filed to `.hermes/plans/`
- `obsidian` — the per-idea dossiers + consolidated plan land in vault notes