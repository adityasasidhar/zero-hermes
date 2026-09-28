---
name: bulk-vault-enrichment
description: "Burn-deep-enrich vault notes by subagents."
version: 0.1.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Vault, Wiki, Enrichment, Bulk, Subagents, Obsidian]
    related: [delegation-first, vault-knowledge-graph]
---

# Bulk Vault Enrichment — Polled-and-Applied Subagent Burn

Use this skill when the user asks to enrich, expand, or "burn through" many notes in a personal-vault wiki (Obsidian-style with `[[wikilinks]]`, MOCs, frontmatter). The shape is N notes of ~1–2 KB that need to become M notes of 6–10 KB with real math, mechanism, citations, and cross-links.

## When to use

- User says "enrich", "deep-enrich", "burn [tokens]", "bulk-update", "expand [a wiki/notes]" and the target is a vault of knowledge notes
- There's a clear depth gap (existing notes are stubs or placeholders) and a rich source tree (raw articles, paper notes, parent concepts) to draw from
- Subagent fan-out is the right tool — each note enrichment is a self-contained reasoning task drawing on 2–3 source notes

## When NOT to use

- A single note needs editing → just edit it
- The vault has no source material to draw from → enrich by hand or stop
- The user just wants index/structure work without content deepening → use `vault-knowledge-graph` instead

## Workflow (Polled-and-Applied)

This is the production-tested pattern. Three phases, each with a hard rule.

### Phase 1 — Setup (parent-side, ~10 min)

1. **Pick the durable scratch dir.** NOT `/tmp` (tmpfs, observed wiped mid-burn by snap recovery on 2026-07-28 at 23:26 IST). Use `<user-projects>/<task>-tmp/` (e.g. `/home/arctic/projects/aiml-burn-tmp/`).
2. **Write the brief template** at `<scratch-dir>/BRIEF-TEMPLATE.md` with the exact spec a subagent must follow: schema, frontmatter-preservation rule, byte target, citation-marker rule, output path, PASS-line contract. See `templates/brief-template.md` in this skill.
3. **Capture pre-burn metrics** to `<scratch-dir>/baseline.json` (note count, total bytes, broken-link count, orphan count, byte-size histogram). This is your morning-delta snapshot.
4. **Choose concurrency.** Begin at `min(target_concurrency, max_concurrent_children)` (typically 3–10). Plan to drop to 1–2 if the multi-task batch wrapper starts failing.

### Phase 2 — Dispatch

**Decision: batch vs single-task.** Start with multi-task batches for parallelism. The instant you see one of:
- Dispatch returns `"synchronous"` on a result (pool saturated)
- Wrapper errors with `"Delegation owner exited before recording a terminal result"`
- File appears in the scratch dir long before the wrapper acknowledges completion

…switch to **single-task `delegate_task` calls** (one task per call). Slower wall-clock, but bypasses the broken wrapper. Per-leaf burn estimate: 600K–1.3M input + 16–45K output tokens (~1.5M average).

**Verify-before-apply brief invariant.** Subagents must NEVER edit the real vault note. They always write to `<scratch-dir>/waveN/<basename>.md` and append a final `PASS:` line. The parent copies to the real note only after verifying frontmatter is preserved, `updated:` bumped, and byte count is in target.

### Phase 3 — Poll-and-apply (parent-side, continuous)

This is the critical discipline. Run a loop:

```
watch <scratch-dir>/waveN/
for each finished file:
  cp to the real vault note
  log the byte delta
```

Don't wait for:
- ✗ The wrapper's batch completion signal (unreliable on long runs — see Pitfall 9 in `delegation-first`)
- ✗ End-of-wave aggregation
- ✗ Subagent's final message text

**Speed rule:** poll every ~30s with terminal+ls or execute_code+os.listdir. Each finished file gets `cp`'d to the real wiki the moment it lands.

**What to verify before cp**:
- Frontmatter intact (original fields preserved, only `updated:` bumped)
- No schema break (YAML still parses)
- Wikilinks resolve (run `python3 wiki/build_index.py --check` once per wave; tolerate 0–5 NEW broken per wave — patch the worst offenders, don't block)
- `counterpart:`, `Part of [[...]]`, `arxiv_id:`, `status:` fields preserved

**What to reject**: notes with NEW broken wikilinks, schema-broken YAML, dropped frontmatter fields. Either patch via parent-side `patch` or mark `FAIL:` at bottom and skip.

### Phase 4 — Closeout

1. **Final diagnostic** — re-run `build_index.py --check` and `graph_stats.py <subwiki>`. Confirm `broken=0` (or accept the small drift if it's safer than mass-rewriting).
2. **Log entry** — append a dated entry to `<subwiki>/log.md` summarizing the burn: what was deepened, what was broken, token spend.
3. **Morning report** — write a summary at `<user-projects>/<task>-results-<DATE>.md` with: before/after byte counts per subdir, broken-link delta, over-cap notes flagged for trim later, token spend (rough estimate based on subagent `input`+`output` tokens from each transcript).

## Quick-decision table

| Symptom | Diagnosis | Fix |
|---|---|---|
| Wrapper returns `Delegation owner exited before recording a terminal result` | Multi-task batch wrapper crash | Switch to single-task delegation; trust the file on disk, not the wrapper |
| File in scratch dir long before wrapper says "completed" | Wrapper lag, not subagent failure | Apply the file; subagent did its work |
| Subagent exits with `HTTP 429 ... Token Plan usage limit` | Rate limit hit on final API call | Check if file was written before the rate limit; if yes, apply as-is |
| Subagent shows 0 output bytes | Genuine failure | Re-dispatch with a fresh subagent, or skip |
| Several subagents over-shoot the byte target | Subagent "research-grade" interpretation differs from the cap | Accept the over-shoot for now; flag for a future trim pass |
| Subagent writes `Concepts/RoPE ...` paths-style wikilinks that don't resolve | Cross-vault bridge format leaked into body | Patch to bare-basename wikilink (`[[rope-positional-encoding]]`) |

## Anti-patterns

1. **Don't batch if the wrapper is mid-glitch.** Stay in single-task mode until you've seen 5+ clean completions in a row.
2. **Don't write to `/tmp`.** It's tmpfs and will get wiped.
3. **Don't cp to the real wiki inside the subagent's brief.** The parent owns the apply step — gives you a clean rollback point if the subagent output is bad.
4. **Don't run the parent-side `find` loop until end-of-wave.** Each finished file is a chance to keep the burn moving — copy immediately.
5. **Don't skip the `broken=0` check.** A subagent introducing 1 broken link that didn't exist before costs you more trust than the whole burn earned.

## Token budgeting

Per subagent: ~1M–2M tokens of input+output. Plan target:

| Vault size | Notes | Subagent tasks | Estimated burn |
|---|---|---|---|
| 50 notes | ~84 KB | 50 | ~75–100M |
| 200 notes | ~300 KB | 200 | ~300–400M |
| 500 notes | ~750 KB | 500 | ~750M-1B |

These are upper bounds. In practice many subagents finish at the low end (~1M each). Wall-clock is the binding constraint: each subagent takes 5–15 min, so 50 notes on 3 concurrent ≈ 2.5–4 hrs of single-track work.

## Support files

- `templates/brief-template.md` — copy-paste brief template for any leaf subagent enrichment task. The shape the parent fills in with goal/path/cross-link-targets/source-materials.
- `references/observed-failure-modes.md` — concrete transcripts and examples of the wrapper crash, the /tmp wipe, and the 429 tail-error scenarios, with the diagnostic commands that confirmed each.
