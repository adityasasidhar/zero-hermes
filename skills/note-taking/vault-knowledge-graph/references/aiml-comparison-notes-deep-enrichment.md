---
name: aiml-comparison-notes-deep-enrichment
description: "Comparison-note variant of the aiml-wiki deep-enrichment pattern. Covers the 9-section Setup/Design/Hypothesis/Methodology/Results framing/Trade-off table/Open questions/Related work/References layout specified by comparison briefs. Distinct from raw/articles and concept/entity notes; the parent file's 11-section research schema does NOT apply."
---

# Aiml wiki — comparison-note deep enrichment

A variant of the aiml-wiki deep-enrichment pattern covered in
`references/aiml-wiki-deep-enrichment.md`. Comparison briefs specify
a **9-section layout**, not the 11-section research schema.

## The 9-section comparison schema

In order, every comparison-note deep-enrichment uses:

1. **Setup** — what each side is, and the comparison axis. Usually one
   short paragraph + 2 bullets defining each side.
2. **Design** — 2-3 subsections each describing one side's mechanism
   (e.g. "execution lever" for the attention side, "operator lever" for
   the recurrent side, plus a "hybrid framing" subsection). Math
   equations appear here.
3. **Hypothesis** — 2 numbered sub-claims, each ≤ 3 lines.
4. **Methodology** — 5-6 inline bullets covering experimental-design
   constraints: parameter matching, budget envelope, compute accounting,
   KV-cache accounting, positional-mechanism consistency.
5. **Results framing** — results organized by granularity
   (operator / hybrid / production), each at 1-2 lines with explicit
   "this does not close the comparison" caveats.
6. **Trade-off table** — 10-12 rows × 2 columns comparing axes head-to-head.
7. **Open questions** — 6 bullets, one per claim being deferred.
8. **Related work** — one bullet per required cross-link target (8 bullets
   if the brief specified 8 cross-links).
9. **References** — both `^[raw/articles/...]` citation markers and
   `[[wiki/...]]` links for sources outside wikilink scope.

## Observed byte-budget (recurrent-state-vs-attention wave14, 12,404-byte attempt)

The 10K ceiling attempt landed 2,404 bytes over. Section-level
distribution from the actual file:

| Section                | Bytes  | Risk   |
| ---------------------- | -----: | ------ |
| Frontmatter            | ~300   | low    |
| Tagline + intro        | ~700   | low    |
| Setup                  | ~700   | low    |
| Design (3 subsections) | ~2500  | HIGH   |
| Hypothesis             | ~850   | low    |
| Methodology            | ~1300  | med    |
| Results framing        | ~1000  | med    |
| Trade-off table        | ~1700  | HIGH   |
| Open questions         | ~1100  | med    |
| Related work           | ~500   | low    |
| References             | ~850   | low    |

Sum: ~11,500 bytes for body before the PASS/FAIL line — already 1,500
bytes over the 10,000-byte ceiling. Body content alone needs to drop to
~9,500 bytes to land in range on the first draft.

## Specific lessons (this session)

### 1. Design section is the highest-risk blowout

Math equations in operator-mechanism write-ups take 200-400 bytes per
display equation (`$$...$$`) and 100-150 bytes per inline statement
(`$...$`). If Design threatens to run >1800 bytes, pre-trim by:

- **Inlining one display equation** — move `$$...$$` into running text
  as single-`$...$`. Same math, ~3× the byte efficiency.
- **Dropping one of the two operator write-ups** if the comparison can
  be made with one (e.g. cite the second operator by name and link to
  the source).
- **Replacing verbose math derivations** with plain-prose
  characterisations of the operator's behaviour.

### 2. Trade-off table is the second-highest risk

Each row in the comparison table takes 150-200 bytes. With 12 rows the
table alone is ~2000 bytes. If the table runs >1500 bytes:

- **Collapse row content from full sentences to phrases**
  (e.g. `FFN routing (orthogonal)` instead of "FFN routing changes
  which parameters activate for a token").
- **Drop a row or two by merging axes** (e.g. "Execution lever" +
  "Arithmetic cost" → single row if both refer to the same underlying
  lever).
- **Use parenthetical context** instead of full sentences.

### 3. Convergence-pattern stop signal for FAIL

The parent file's "Byte-budget discipline" section sets a
switch-to-FAIL threshold at "<300 bytes over AND 2+ consecutive
<100-byte iter cuts". **Extension from this session**: if you are
still >300 bytes over but have **3+ consecutive trim iterations each
moving <100 bytes**, you are in the same regime — content-fit mismatch
with the brief ceiling, not loose bytes. Switch to FAIL rather than
continue patching at <100 bytes per iter.

Session trajectory hit this pattern: 18165 → 16476 → 15978 → 15746 →
15336 → 15233 → 14862 → 14441 → 14342 → 13095 → 12761 → 12678 →
12657 → 12544 → 12464 → 12350 → 12323 → 12295 → 12404 (with FAIL line).
The trim delta dropped from 1200+ bytes (early rewrites) to <100 bytes
(later iterations), and the file could not converge without sacrificing
required content (math equations, trade-off table cells, citation
markers, 9 outbound wikilinks).

Honest response: `FAIL: <bytes> over <ceiling> byte ceiling after
<N> rewrites, parent should re-run with pre-budgeted section
constraints`. The parent then re-dispatches either with tighter
per-section caps or accepts the file as-is and applies it.

## Cross-link budget

Aiml-wiki briefs typically list 8 required cross-link targets. All
must resolve in the aiml wiki (concepts/ entities/ comparisons/
raw/articles/). The "Related work" section is the natural home for
these — one bullet per target. The body should also include at least
5 cross-links to satisfy the BRIEF-TEMPLATE ≥5 rule (the brief itself
typically sets a higher count via the `LIST_OF_WIKILINK_TARGETS` field).

The `[[flashattention]]` raw/articles soft-exception is permitted:
even though only `raw/articles/flashattention.md` exists, the basename
match is tolerated by the resolver. The recommended response is to use
the wikilink AND flag in a `## Broken links in this note` footer:
"`[[flashattention]]` — wikilink target only exists as
`raw/articles/flashattention.md`; remediation: create
`concepts/flashattention.md` and link from there."

## Wikilink count vs citation count

The PASS-line "outbound links" count excludes source refs in the form
`[[Research/Papers/...]]`. Required cross-link bullets that resolve in
the aiml wiki (`[[long-context]]`, `[[gated-deltanet]]`, etc.) ARE
counted. Don't conflate the two — common mistake when reading the
final file.

## Sources & references

- `references/aiml-wiki-deep-enrichment.md` — the parent playbook
  (covers raw/articles and concept/entity notes, with 11-section
  research schema).
- `BRIEF-TEMPLATE.md` — the parent agent's brief contract.
- Worked-example session: recurrent-state-vs-attention-for-long-context
  comparison note, wave14, output at
  `/home/arctic/projects/aiml-burn-tmp/wave14/recurrent-state-vs-attention-for-long-context.md`
  (12,404 bytes, FAIL shipped for byte ceiling; reported with honest
  reason: "parent should re-run with pre-budgeted section constraints").
