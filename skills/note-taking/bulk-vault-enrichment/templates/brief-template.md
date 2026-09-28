# Brief Template — Aiml Wiki Deep-Enrichment Leaf Subagent

Copy this to `<scratch-dir>/BRIEF-TEMPLATE.md` before dispatching any leaf subagent
enrichment task. The shape the parent fills in with goal/path/cross-link-targets.

---

## Goal

Deep-enrich one aiml wiki note to research-grade depth, save the FULL
enriched copy to a temp file on durable disk. DO NOT edit the real note —
the parent agent verifies and applies.

## Context

You are a leaf subagent. Your target note is `<NOTE_PATH>` under
`/home/arctic/Documents/fun/wiki/aiml/`.

### Required reading (in order)

1. `/home/arctic/Documents/fun/wiki/aiml/SCHEMA.md` — frontmatter
   fields, tag taxonomy, citation-marker convention.
2. The full current note at `<NOTE_PATH>`.
3. Any related `raw/articles/*.md` files in the aiml wiki that cover
   the same concept — these are your primary sources. Read at least 2
   if they exist.
4. The deep-enrichment playbook: `~/.hermes/skills/note-taking/vault-knowledge-graph/references/deep-enrichment.md`
   (for the 11-section schema + depth criteria).

### What "research-grade depth" means

The current note is ~1–2 KB. Target: **6,000–10,000 bytes**. That
means real math, real mechanism, real citations, real cross-links —
not filler. If you can't substantiate a claim from a primary source,
DROP it or mark it as unsourced. No fabrication.

## Constraints (hard)

1. **Output path MUST be** `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`
   where `<basename>` is the filename without extension. DO NOT write
   to the real note path. Use durable disk (NOT `/tmp/`).
2. **Preserve existing frontmatter verbatim**, except bump the
   `updated:` field to today's date (`2026-07-29`).
3. **Preserve any existing `Part of [[...]]` line** if present.
4. **Schema**: use the 11-section deep-enrichment schema:
   Overview, History/Motivation, Mechanism, Math, Implementation,
   Variants, Use cases, Trade-offs, Connections, Open questions,
   References.
5. **Citation markers**: any paragraph that synthesizes 3+ sources or
   traces to a specific paper must carry a
   `^[raw/articles/<source>.md]` marker at the end.
6. **Outbound wikilinks**: minimum 5, all resolvable in the aiml wiki.
   Before writing, run
   `python3 /home/arctic/Documents/fun/wiki/build_index.py --check`
   to confirm target basenames exist. If a target doesn't resolve,
   add a `## Broken links in this note` footer flagging it.
7. **Tone**: technical, precise, first-principles. No filler.
8. **Language**: English. Math in `$...$` LaTeX where it helps.

## Deliverable

- File at `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`
  containing the FULL enriched note (replaces original; not appended).
- Last line MUST be either:
  - `PASS: <bytes> bytes, <outbound_wikilink_count> outbound links`
  - `FAIL: <one-line reason>`

## Failure modes — surface them, don't hide

- If you can't find a source for a claim, drop it. Don't fabricate.
- If a wikilink target doesn't resolve and you can't rewrite it,
  flag it in the footer.
- If the existing note has content you can't reconcile with the
  11-section schema, preserve the original prose verbatim inside a
  `## Original (preserved)` section and explain in one line why.

## What NOT to do

- Do NOT edit any file outside `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`.
- Do NOT run git operations.
- Do NOT spawn further subagents (you are a leaf).
- Do NOT write to `/tmp/` — that filesystem is volatile and has been
  wiped mid-burn before.

---

## Variable substitution block (parent fills these)

- `<NOTE_PATH>` = relative path under `/home/arctic/Documents/fun/wiki/aiml/`.
- `<basename>` = filename without `.md`. Example: `rmsnorm`.
- `<waveN>` = current wave number. Example: `wave2`.
- `<BYTES>` = current size of the note in bytes.
- `<LIST_OF_RELATED_RAW_ARTICLES>` = newline-separated list of
  relevant `raw/articles/*.md` paths.
- `<LIST_OF_WIKILINK_TARGETS>` = newline-separated list of
  concept/entity basenames you should cross-link to.

---

## Per-call dispatcher substitution

When invoking via `delegate_task`, the parent fills these fields inline
in the goal string:

```
Deep-enrich /home/arctic/Documents/fun/wiki/aiml/<SUBDIR>/<basename>.md
to 6,000-10,000 bytes. Read BRIEF-TEMPLATE at
/home/arctic/projects/aiml-burn-tmp/BRIEF-TEMPLATE.md first.
Existing size: <bytes> bytes. Use 11-section schema.
Source materials to read:
  - <path1>
  - <path2>
  - <path3>
Cross-link targets that resolve in aiml wiki:
  - [[target1]]
  - [[target2]]
  - ...
Write output to /home/arctic/projects/aiml-burn-tmp/wave<N>/<basename>.md.
Preserve frontmatter except bump updated to 2026-07-29.
Final line: `PASS: <bytes> bytes, <count> outbound links`.
Language: English, technical, first-principles. NO FABRICATION.
Math in $...$ LaTeX where it helps.
```

The more specific and concrete the brief, the better the output.
Include byte size, schema variant (concept vs comparison vs entity vs
raw/articles), source paths, cross-link targets, and the contract line
in every brief.
