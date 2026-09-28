---
name: vault-knowledge-burn
description: "Run multi-wave bulk-enrichment on an Obsidian sub-wiki."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux]
---

# Vault Knowledge Burn

Bulk-enrich an Obsidian vault sub-wiki (50–200 notes) to research-grade depth with math, citations, and cross-links. Validated on the aiml wiki (50 notes, 6.5× byte growth) on 2026-07-29.

## When to use

- User asks to "burn tokens" / "deep-enrich a wiki" / "rewrite short notes into research-grade pages"
- Working with an Obsidian sub-wiki (aiml/, personal/, etc.) that has many short (1–2 KB) notes that need deepening
- Want 30+ note rewrites overnight / over several hours

## When NOT to use

- Single note edit (use `read_file` + `write_file`/`patch` directly)
- Non-Obsidian markdown vaults (different schemas)
- Vault that doesn't have a `SCHEMA.md` and a tag taxonomy (can't validate schema without it)

## Hard rules (learned from round-1 failure)

1. **Scratch dir on durable disk.** `/home/arctic/projects/<name>-tmp/`, NEVER `/tmp/`. `/tmp` is tmpfs and gets wiped by snap/systemd recovery events (verified 2026-07-28 — 12 subagent outputs lost). Round 2 used durable disk and survived a second recovery cleanly.

2. **Single-task `delegate_task` calls only.** The multi-task batched form `delegate_task(tasks=[...])` crashes the parent-side wrapper on EVERY batch — verified across 8 separate batches in round 1, all 8 crashed with "Delegation owner exited before recording a terminal result" even when subagents completed fine. Use one `delegate_task(goal=...)` call per subagent task.

3. **Poll-and-apply per file.** Parent checks `/home/arctic/projects/<name>-tmp/waveN/*.md` after each expected completion and `cp`s the file to the real wiki immediately. Don't wait for end-of-wave — single subagent delays or crashes shouldn't block later work.

4. **Validate each temp file before cp.** Check frontmatter preserved (`updated:` bumped, `Part of [[...]]` line preserved, `counterpart:` preserved), size in 6–10 KB target range (accept 11–18 KB with caveat), file ended with `PASS:` or `FAIL:` line. Use `grep -E "^PASS:|^FAIL:"` for the contract line.

5. **Schema gate after apply.** `cd <vault_root> && python3 wiki/build_index.py --check` reports `broken=0` after each wave. If broken count rises, halt and patch before continuing.

## BRIEF-TEMPLATE structure

The subagent brief must include:
- NOTE_PATH under the wiki root
- Existing size in bytes
- 11-section schema (or comparison-/entity-specific sections)
- Source materials to read (2–3 sibling notes)
- Cross-link targets that resolve in the wiki (parent has verified these exist)
- Frontmatter constraints (preserve verbatim, bump `updated:`)
- Output path on durable disk (NEVER `/tmp/`)
- Last-line contract: `PASS: <bytes> bytes, <count> outbound links` or `FAIL: <reason>`
- Anti-fabrication: drop unsourced claims, mark them, don't make up

**Wave scaling rules-of-thumb:**
- Per subagent: 600K–1.3M input tokens + 16–45K output
- Wall-clock per task: 5–15 minutes (single-task delegation runs synchronously when pool is full)
- Effective concurrency ≈ 1 (sync pool) regardless of `max_concurrent_children`
- 30 notes takes ~6 hours wall-clock
- Rate limits (HTTP 429) hit at unpredictable intervals — re-dispatch failed files cleanly

## Post-burn verification

After every wave:
```bash
cd <vault_root>
python3 wiki/build_index.py --check  # confirms broken=0
find <wiki>/<subdir> -name "*.md" -type f -exec wc -c {} + 2>/dev/null | tail -1
```

Generate a final report at `/home/arctic/projects/<name>-results-<date>.md` with:
- Before/after byte counts (per subdir)
- Per-wave pass/fail table
- Token spend estimate
- Top over-sized notes (if any > 15 KB)
- Schema compliance status

Append a dated entry to `<wiki>/log.md` (per the wiki's own convention; check log.md for format).

## Failure-mode catalog (validated 2026-07-29)

| Failure | Symptom | Recovery |
|---|---|---|
| `/tmp` wipe | Subagent outputs disappeared between dispatch and parent apply | Migrate scratch dir to durable disk |
| Batched `delegate_task` crashes wrapper | "Delegation owner exited before recording a terminal result" | Switch to single-task `delegate_task(goal=...)` |
| HTTP 429 rate limit | Subagent exits with `max_iterations` after 50 API calls | Re-dispatch with same goal; usually succeeds on retry |
| Subagent over-shoots byte cap (11–22 KB) | File exceeds 10 KB target | Apply as-is; content is sound, just denser than spec |
| Subagent writes broken wikilink (cross-vault bridge that doesn't resolve) | `build_index.py --check` shows broken > 0 | `patch` to retarget to resolvable basename |
| Subagent forgets to update PASS line after final trim | File complete, PASS line stale | `wc -c` and patch the PASS line, OR apply file and ignore stale PASS |

## Sample dispatch (single-task format)

```python
delegate_task(
    goal="""Deep-enrich /path/to/wiki/concepts/rmsnorm.md to 6,000-10,000 bytes.
Read BRIEF-TEMPLATE at /home/arctic/projects/aiml-burn-tmp/BRIEF-TEMPLATE.md first.
Existing size: 1052 bytes. Use 11-section schema.
Source materials: read X.md, Y.md, Z.md.
Cross-link targets: [[A]], [[B]], [[C]], ...
Write output to /home/arctic/projects/aiml-burn-tmp/wave1/rmsnorm.md.
Preserve frontmatter except bump updated to <TODAY>.
Final line: PASS: <bytes> bytes, <count> outbound links.
NO FABRICATION. Math in $...$ LaTeX where it helps.""",
    role="leaf"
)
```
