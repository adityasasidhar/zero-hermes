# Deep enrichment of aiml-wiki concept/entity notes

A sibling to `deep-enrichment.md` (which covers GitHub-repo project notes).
This file covers the **aiml-wiki research-note** deep-enrichment pattern:
taking a single concept or entity note in `/home/arctic/Documents/fun/wiki/aiml/`
from a ~1–2 KB stub to a 6,000–10,000 byte research-grade note using the
11-section research schema, real source materials, and the no-fabrication
discipline that the aiml wiki enforces.

Distinguishing this from `deep-enrichment.md`:

| | GitHub-repo (`deep-enrichment.md`) | aiml-wiki (this file) |
|---|---|---|
| Note class | Project / repo card | Research concept or entity |
| Schema | Status, Inside the Codebase, Key Features, Stats, … | Overview, Mechanism, Math, Variants, … (see schema below) |
| Primary source | `gh api`, README, cloned source | `raw/articles/*.md` (paper summaries in the wiki) |
| Citation marker | n/a | `^[raw/articles/<source>.md]` on paragraphs that synthesize ≥3 sources |
| Output dir | `/tmp/enrich-waveN/` | `/home/arctic/projects/aiml-burn-tmp/waveN/` (durable disk) |
| Wikilink target | repo notes, themes | concept / entity / Research-paper notes in the aiml wiki |
| Frontmatter `sources:` | `raw/papers/...` | `Research/Papers/.../...md` (preserve verbatim) |
| Hard rule | "Don't fabricate versions" | **No fabrication, period** — every claim traces to a source |

## When to use

Use this playbook when the user/parent agent asks to deep-enrich a specific
`wiki/aiml/concepts/<X>.md` or `wiki/aiml/entities/<X>.md` note, typically
via a BRIEF that lives at `/home/arctic/projects/aiml-burn-tmp/BRIEF-TEMPLATE.md`
or a per-wave copy. The brief lists `<NOTE_PATH>`, `<basename>`, `<waveN>`,
`<BYTES>`, related raw articles, and required wikilink targets.

If the task is to enrich the **whole aiml wiki** from a hand-written vault
(scratch pipeline, paper notes), use `cross-vault-wiki-enrichment.md` instead.
If the task is to enrich a **GitHub repo** note, use `deep-enrichment.md`.
This file is for the single-note, single-brief leaf-subagent dispatch.

## Brief contract (from BRIEF-TEMPLATE)

The parent fills these in the brief; the leaf reads and follows:

- `<NOTE_PATH>` — relative path under `/home/arctic/Documents/fun/wiki/aiml/`
  (e.g. `concepts/multi-token-prediction.md`).
- `<basename>` — filename without `.md` (e.g. `multi-token-prediction`).
- `<waveN>` — current wave number (`wave2`, `wave7`, …).
- `<BYTES>` — current size of the note in bytes (so the agent knows the
  starting point).
- `<LIST_OF_RELATED_RAW_ARTICLES>` — newline-separated `raw/articles/*.md`
  paths the agent should read as primary sources.
- `<LIST_OF_WIKILINK_TARGETS>` — newline-separated concept/entity basenames
  the agent should cross-link to (≥5 required, all must resolve).

Hard rules from the brief:

1. **Output path is `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`**.
   Durable disk (`/tmp/` is volatile and has been wiped mid-burn before).
2. **Preserve existing frontmatter verbatim** except bump `updated:` to
   today's date. Do not edit `title`, `created`, `type`, `tags`, `sources`,
   `confidence`. Source ref format `Research/Papers/.../...md` is sacred.
3. **Use the 11-section research schema** (below). Do not use the
   project-notes schema from `deep-enrichment.md`.
4. **Citation markers** (`^[raw/articles/<source>.md]`) on any paragraph
   that synthesizes ≥3 sources or traces to a specific paper.
5. **Wikilinks must resolve in the aiml wiki.** Before writing, run
   `python3 /home/arctic/Documents/fun/wiki/build_index.py --check` to
   confirm target basenames exist. **Important**: `build_index.py` lives
   in the parent `/wiki/` directory, not in `wiki/aiml/`. The brief's
   command path is correct; the wrong path silently fails.
6. **No fabrication.** If you can't find a source for a claim, drop it or
   mark it as unsourced. The original note's `confidence: medium` (or
   whatever) is preserved; do not upgrade.
7. **Last line is `PASS: <bytes> bytes, <outbound_wikilink_count> outbound links`**
   or `FAIL: <one-line reason>`. The parent agent greps for this.

## The 11-section research schema

In order, every deep-enriched aiml-wiki concept note uses:

1. **Overview** — 2–4 sentences: what it is, who's behind it, what makes
   it interesting. Carries the citation that establishes the topic.
2. **History / Motivation** — origin, why it exists, what problem it
   solves. Often co-introduces the *previous* state of the art being
   generalized.
3. **Mechanism** — how it works at training and inference time, with
   the architectural pattern(s) it comes in.
4. **Math** — loss formulations, operator definitions, complexity. Use
   LaTeX in `$...$` (and `$$...$$` for display). Mark anything not in
   the cited sources as "not recorded in this vault's sources" rather
   than making it up.
5. **Implementation** — practical wiring, hyperparameters, micro-opt
   notes, inference-time algorithm sketch.
6. **Variants** — parallel vs. chained forms, paper-A vs. paper-B
   differences, how the technique composes with adjacent techniques.
7. **Use cases** — 3–5 concrete scenarios, with citations where the
   source actually mentions the use case.
8. **Trade-offs** — honest cost/benefit, including memory, compute, and
   the "what you give up" axis.
9. **Connections** — bullet list of `[[wikilinks]]` to the cross-link
   targets in the brief, each with a one-line role statement.
10. **Open questions** — what's *not* in the vault's sources and would
    need verification. The parent agent uses this to gauge the risk of
    applying the enrichment to the real note.
11. **References** — `[[Research/Papers/...|alias]]` lines for each
    cited paper, plus the `^[raw/articles/...]` markers used inline.

Sections 2, 4, 10, and 11 are the ones most often under-written.
A common failure is a 9-section note with a thin Math section and an
empty Open questions — those are the lowest-confidence spots and the
parent agent's verifier checks them.

## Wikilink resolution rules for this vault

A wikilink `[[name]]` (or `[[name|display]]`) resolves in the aiml wiki
if **any** of these files exists:

- `wiki/aiml/concepts/<name>.md`
- `wiki/aiml/entities/<name>.md`
- `wiki/aiml/comparisons/<name>.md`
- `wiki/aiml/queries/<name>.md`
- `Research/Papers/.../<name>.md` (or any path that ends with `/<name>.md`)

**It does NOT resolve if:**

- `<name>` is only a tag in the taxonomy (e.g. `speculative-decoding`,
  `kv-cache`). Tags live in `SCHEMA.md` under "Tag Taxonomy"; they are
  not notes. This is a real failure mode — `[[speculative-decoding]]`
  looks plausible because the tag is in the schema, but it produces a
  broken wikilink.
- `<name>` is only a raw article (`raw/articles/<name>.md`). Cite raw
  articles via `^[raw/articles/<name>.md]`, not via `[[...]]`. There is
  one soft exception: `[[flashattention]]` is used as a wikilink in
  `concepts/gated-deltanet.md` even though only the raw article exists;
  this is a fuzzy convention, not a hard rule, and the build_index
  resolver still tolerates it because the basename matches.

**When the BRIEF itself lists a non-resolving target as a required
cross-link, use the wikilink AND flag it in a `## Broken links in this
note` footer.** Session-evidenced 2026-07-29 (long-context wave7):
the brief's `LIST_OF_WIKILINK_TARGETS` listed `[[flashattention]]` as
one of 8 required cross-link targets even though
`concepts/flashattention.md` does not exist (only
`raw/articles/flashattention.md` does). The correct response is to use
`[[flashattention]]` in the body (the brief required it), then add a
`## Broken links in this note` footer noting the mismatch and
recommending `concepts/flashattention.md` be created. Silent omission
violates the brief; bare inclusion without a footer hides the issue
from the parent agent. The footer format:
`\`[[<target>]]\` — <one-line description of why it doesn't resolve and
recommended remediation>`. This footer is part of the body, not the
PASS line.
- The target is a stub (0–50 bytes). Don't bridge to stubs — see
  `vault-knowledge-graph` SKILL.md "Stub files are sacred".
- Folder-only path (`[[Foo/Bar]]` where `Foo/Bar/` is a folder, not a
  note). Use the bare basename or the full path with filename.

`build_index.py --check` is authoritative for resolution; the count is
real, the line numbers are often off by one — re-derive them with
`open(p).read().count('\n', 0, m.start()) + 1`.

## Frontmatter preservation discipline

The existing note's frontmatter is the contract with the rest of the
vault. Things to preserve verbatim:

- `title:` (the display name)
- `created:` (the original creation date)
- `type:` (entity | concept | comparison | query | summary)
- `tags:` (from the existing taxonomy, even if a tag looks unfamiliar
  to you)
- `sources:` (the source-ref list — including `Research/Papers/...`
  paths, which are NOT `raw/articles/...` and are NOT shortened)
- `confidence:` (high | medium | low — do not upgrade based on the
  enrichment work you did)
- `contradictions:` / `contested:` (rare; preserve as-is)

The only field you change is `updated:`, which you bump to today's
date. Some notes also have an `arxiv_id:` (optional but recommended
for papers); if it's present, leave it. If it's missing and the brief
gives you one, adding it is fine — adding new frontmatter fields is OK,
modifying existing ones is not.

### raw/articles sub-pattern (different frontmatter schema)

Notes at `wiki/aiml/raw/articles/<paper>.md` are **ingested paper
summaries** with a different frontmatter schema than concept/entity
notes. They have:

```yaml
---
source_url: https://arxiv.org/abs/<id>
ingested: YYYY-MM-DD
sha256: <hex digest of the raw body>
arxiv_id: <id>
---
```

There is **no** `title:`, `created:`, `type:`, `tags:`, `sources:`, or
`confidence:`. The body is plain prose — the agent-ingested summary of
the paper. When the brief says *"This is a raw/articles note (different
frontmatter: source_url, ingested, sha256, arxiv_id)"*, preserve those
four fields verbatim, bump `updated:` to today's date (or add it if it
was missing — `updated:` is one new frontmatter field that adding is
acceptable per the rule above), and apply the same 11-section body
schema. Do NOT retrofit a `title:`, `created:`, `type:`, or `tags:` —
the parent agent already chose the raw/articles form deliberately, and
the vault's `build_index.py` indexes these notes by sha256, not by
`title:`.

Cross-link rules are the same: target basenames must resolve in the aiml
wiki (concept/entity/raw-article). Citation markers are the same:
`^[raw/articles/<source>.md]` for paragraphs that synthesize ≥3 sources
or trace to a specific paper.

**Byte-budget gotcha specific to raw/articles notes**: because they
often summarise high-information papers (multiple innovations, large
benchmarks, dense math), the first draft routinely runs 30–50% over
the ceiling. Apply the section-budget discipline from "Byte-budget
discipline" *especially* for raw/articles notes — pre-budget the Math
section to ≤4 display equations, the Mechanism section to ≤3, and use
list-form for Variants/Use cases/Trade-offs to keep prose compact. If
your first `wc -c` after the initial `write_file` is over 18 KB,
the safest recovery is a full re-draft with the section budgets in
place, not a multi-attempt patch chain — session evidence shows that
patch chains from 18+ KB rarely converge below the 10 KB ceiling
within the tool-budget a leaf agent has.

**Observed per-section byte-budget for a 9,990-byte raw/articles note
(2026-07-29, RoFormer/RoPE wave10):** useful as a planning baseline.
Over-budget the Math and Mechanism sections, lean into prose everywhere
else, keep Connections and References tight.

| Section                  | Bytes | Share | Notes                                                     |
| ------------------------ | ----- | ----- | --------------------------------------------------------- |
| Frontmatter (4 fields + `updated:`) | ~280 | 3%   | Stays verbatim; one new line.                             |
| Overview                 | ~1100 | 11%  | 2 short paragraphs max. Citation on the source-tracing line. |
| History / Motivation     | ~900  | 9%   | 2 paragraphs: limits of prior scheme + Su et al.'s pivot. |
| Mechanism                | ~1100 | 11%  | One display equation (2-D rotation matrix).               |
| Math                     | ~1300 | 13%  | Two display equations (identity + per-pair expansion).    |
| Implementation           | ~900  | 9%   | 3-stage numbered list + 1 paragraph.                      |
| Variants                 | ~1200 | 12%  | Inline-bullet form (linear / NTK / YaRN / ABF / ALiBi / NoPE). |
| Use cases                | ~1000 | 10%  | One paragraph + Models list with parenthetical context.   |
| Trade-offs               | ~1000 | 10%  | 5 inline bullets.                                         |
| Connections              | ~900  | 9%   | 8 bullets (one per required cross-link target).           |
| Open questions           | ~750  | 8%   | 4 bullets.                                                |
| References + PASS line   | ~450  | 5%   | 2 source refs + `PASS: <bytes> bytes, <k> outbound links`. |

The Math section is the most expensive and the most likely to blow the
budget — it ate 13% of total bytes despite being only 1% of the words.
If your first draft has Math > 1,500 bytes, plan to either collapse
display equations to inline or move derivations to "see also" in
`[[rope-positional-encoding]]` style references. Connection bullets at
~110 bytes each (8 required cross-links = ~880 bytes) hold near the budget
floor; don't pad them.

**Verifying every trim round with the script, not by hand:** session
evidence (RoPE wave10) shows the manual `wc -c` + `grep` + `tail`
sequence for the byte-budget dance costs 4–6 tool calls per round. Use
`scripts/verify_aiml_wiki_output.py` (described in "After the leaf
writes" below) as a single-call gate after every `patch` / `write_file`.
The script catches the three failure modes that bit the RoPE run:
"section accidentally deleted by over-broad patch", "PASS line byte
count drifted from file size", "PASS line format drifted from
`PASS: <N> bytes, <K> outbound links$`". Each round costs 1 tool call
instead of 5+.

## Byte-budget discipline (the most common pitfall)

Briefs in this wave pattern set byte targets (e.g. 6,000–10,000). The
trap is to write a "full" first draft that comes in at 12,000+ bytes and
then have to trim aggressively. Real session evidence: a draft of the
aiml `multi-token-prediction` note came in at 12,723 bytes on the first
pass, 10,480 on the second, 9,988 on the third (in range). Each trim
required 3–4 targeted patches.

**Iteration-cost evidence (2026-07-29, cot-faithfulness deep-enrichment):**
a first draft came in at 17,139 bytes (~70% over the 10 KB ceiling).
Compressing it via `patch` chains took 10+ iterations:

```
write_file:        17,139 bytes
write_file:        15,411  (-1,728)   full rewrite
write_file:        13,372  (-2,039)   full rewrite
write_file:        13,131  (-241)     full rewrite
write_file:        12,311  (-820)     full rewrite
write_file:        12,180  (-131)     full rewrite (over-trim)
write_file:        11,703  (-477)     full rewrite
write_file:        11,180  (-523)     full rewrite
write_file:        10,996  (-184)     full rewrite
write_file:        10,267  (-729)     full rewrite
write_file:        10,106  (-161)     full rewrite (final)
```

**Every successful cut above 200 bytes was a full `write_file`, not a
`patch`.** Patch chains that ran alongside the rewrites each moved
the count by 5–200 bytes — the wrong tool for the job once the file
isn't already close to range. The lesson: as soon as a `wc -c` shows the
file is >2 KB above the ceiling, switch from `patch` chains to `write_file`
full rewrites with the section budget in mind, rather than burning
tool calls on patch chains that converge slowly. The "3–4 targeted
patches" estimate from the multi-token-prediction session was
optimistic; in practice, once you're >30% over the ceiling, plan for
5–8 full rewrites.

**Second-iteration-cost evidence (2026-07-29, alignment-faking
deep-enrichment):** a first draft came in at 16,445 bytes (~64%
over the 10 KB ceiling). The trajectory:

```
write_file:        16,445 bytes
write_file:        14,710  (-1,735)   full rewrite
write_file:        13,682  (-1,028)   full rewrite
write_file:        13,327  (-355)     full rewrite (slowing)
write_file:        13,017  (-310)     full rewrite
write_file:        12,474  (-543)     full rewrite
write_file:        11,766  (-708)     full rewrite
write_file:        11,550  (-216)     full rewrite
write_file:        11,428  (-122)     full rewrite
write_file:        11,209  (-219)     full rewrite
write_file:        11,093  (-116)     full rewrite
write_file:        10,948  (-145)     full rewrite
write_file:        10,911  (-37)      full rewrite (stalling)
write_file:        10,593  (-318)     full rewrite
write_file:        10,429  (-164)     full rewrite
write_file:        10,380  (-49)      full rewrite (tool budget exhausted)
```

After 14 full rewrites the file was 380 bytes over the ceiling and
the leaf agent's tool-call budget was exhausted. The lesson: even
*full rewrites* converge slowly once you're inside ~10% of the
ceiling — each rewrite moved the count by 100–300 bytes. By the
last 3 rewrites the marginal gain was 49–164 bytes per rewrite,
which means **for 380 bytes, the next rewrite was the only one that
could possibly land it in range** — but the budget was gone. The
correct action when tool budget runs out with the file still over
the ceiling is `FAIL: <bytes> over <ceiling> byte ceiling after
<N> rewrites, parent should re-run with pre-budgeted section
constraints`, NOT to ship a wrong byte count in a placeholder
PASS line. The parent can either re-run the leaf with a tighter
brief (e.g. pre-budgeted per-section caps) or apply the file
manually after a single targeted trim.

**Concrete switch-to-FAIL threshold:** when the file is <300 bytes
over the ceiling AND you've completed at least 2 consecutive
trim iterations each moving the count by <100 bytes, switch to
FAIL immediately. Do not attempt a third or fourth micro-trim —
the next iteration is overwhelmingly likely to either leave you
in the same place or overshoot downward into an under-ceiling
file that drops required content. Session-evidenced 2026-07-29
(long-context wave7): the trajectory ended at 10,039 bytes
(39 over a 10,000 ceiling) after multiple small Python-trim
passes. The correct response was `FAIL: 10039 bytes over 10000
byte ceiling after N rewrites, parent should re-run with
pre-budgeted section constraints`. Shipping the file at
10,039 bytes with `PASS: 10003 bytes, 8 outbound links` (PASS
measured before appending, real size 10,039) is the exact
"placeholder PASS line is a LIE" failure mode the earlier pitfall
warns against. The PASS line must be re-measured *after* appending
itself, and if the resulting count exceeds the ceiling, the
file does not pass — write FAIL instead.

**Two more concrete lessons from the alignment-faking run:**

1. **Pre-budgeted per-section caps save tool calls.** The
   alignment-faking note's Math section (1,609 bytes after the
   3rd rewrite) was always going to need a 30%+ cut to hit the
   ceiling — but the section-budget discipline from this playbook
   (Math ≤ ~900 bytes) would have prevented the initial blowout.
   When you know the section you're about to write could plausibly
   be 1,500+ bytes, write the shorter version first and expand
   only if budget remains.

2. **Source-ref wikilinks are NOT counted in outbound link totals.**
   The `[[Research/Papers/Alignment-Faking-in-LLMs/alignment-faking-in-llms|...]]`
   line in `## References` is a citation, not a cross-link target,
   and the parent's verifier excludes it from the outbound-link
   count. The 8 required cross-links in the brief (concept/entity
   basenames like `[[cot-faithfulness]]`) ARE counted. Easy mistake
   when grep-counting: include the source ref and over-report;
   exclude it correctly and under-report by 1.

Practical recipe to land in range on the first try:

1. **Estimate sections before writing.** Overview + History + Mechanism
   + Math + Implementation + Variants + Use cases + Trade-offs +
   Connections + Open questions + References ≈ 11 sections. Budget
   600–900 bytes per section for a 6,500–10,000 byte note. The Math
   and Mechanism sections are usually 1,000–1,500; References and
   Connections are usually 200–400.
2. **Write the Connections and References sections first** (they're
   deterministic from the brief's wikilink list and source list). This
   fixes the outbound-link count and the citation baseline.
3. **Then write Overview, History, Mechanism** — the prose-heavy
   sections. Keep each paragraph ≤ 4 sentences; favour density over
   breath.
4. **Then Math, Implementation, Variants, Use cases, Trade-offs** — these
   expand the note. Watch the count.
5. **Then Open questions** — explicit list of what's not in the
   sources. Often under-written; aim for 4–6 bullets.
6. **Final pass: `wc -c`.** If over the ceiling, trim the prose-heavy
   sections first (Mechanism, Implementation), not the structural
   ones (Connections, References, Open questions).

## Common pitfalls (session-evidenced)

- **Byte ceiling overshoot.** See "Byte-budget discipline" above.
  Tendency: aiml concept notes are easy to over-write because they
  have rich math and many cross-link targets. Default ceiling in
  recent briefs is 10,000; treat that as a hard cap.
- **Non-resolving `[[...]]` for taxonomy tags.** `[[speculative-decoding]]`
  looks plausible because it's in the SCHEMA tag taxonomy; it does
  not resolve. Tags ≠ notes. Verify each wikilink target resolves
  via `build_index.py --check` (or by `find` if `build_index.py` is
  unavailable) before using it.
- **Wrong `build_index.py` path.** The script lives at
  `/home/arctic/Documents/fun/wiki/build_index.py`, NOT in
  `wiki/aiml/`. The brief's command is correct; the naive
  `cd wiki/aiml && python3 build_index.py --check` silently fails
  with `No such file or directory`.
- **Confusing raw articles with concept notes.** When the brief says
  "read `raw/articles/<paper>.md`", that's a source file, not a
  wikilink target. Cite it via `^[raw/articles/<paper>.md]`. The
  one soft exception is `flashattention` (see Wikilink Resolution).
- **Conflating `Research/Papers/...` (in frontmatter `sources:`) with
  `raw/articles/...` (in citation markers).** They are different
  paths serving different roles:
    - `sources:` in frontmatter lists which papers the note draws
      from. Preserved verbatim; uses `Research/Papers/...` paths.
    - `^[raw/articles/...]` markers cite specific paragraphs that
      trace to the vault's summary of a paper. Adds new content
      based on what the agent read.
  Don't mix them.
- **Upgrading `confidence:` based on the work you did.** The brief
  asks for an enrichment, not a verification. If the original note
  said `confidence: medium`, leave it `medium` — your work may have
  added context, but until the V3 report is re-read at the level of
  detail your prose implies, the underlying confidence is unchanged.
- **Writing past the ceiling in the final draft.** The first draft
  *will* be 20–30% over the ceiling if you write it normally. Build
  the trim pass into the workflow, not as an afterthought.
- **Multi-section `patch` chains break structural notes.** Concept notes
  have bullets nested inside sections; `patch` find/replace on list
  items can accidentally indent continuation lines (turning prose into
  a code block), delete the heading of the next section (e.g. a
  `## Connections` heading removed because it was the line after a
  `Trade-offs` body), or merge two sections into one. When the next
  edit is more than ~150 bytes OR crosses a section boundary, prefer
  a full `write_file` rewrite over a chained `patch`. The byte budget
  is wasted on broken-structure revisions that ratchet the file *up*
  (because re-indented/re-broken content re-expands the structure),
  not down. Session-evidenced: a 7-attempt `patch` chain on an aiml
  MoE note went 11,773 → 11,813 → 11,687 → 11,489 → 11,380 → 11,209
  → 11,282 — every "fix" that broke section structure added 100–200
  bytes back, and two rewrites had to rebuild the `## Connections` and
  `## References` headings that patches had accidentally absorbed into
  neighbouring prose. Real cuts need explicit content removal (drop a
  bullet, shorten a paragraph), not whitespace surgery.

  **Concrete failure pattern (2026-07-29, cot-faithfulness):** the
  `patch` `old_string` for a Math→Implementation boundary edit was
  anchored on the last line of Math ("becomes an explanation, not a
  process.") plus the `## Implementation` heading plus the first line
  of Implementation. The `new_string` was meant to drop one trailing
  blank line — but because the patch tool's fuzzy match resolved the
  `## Implementation` heading as part of the *Math* section's tail
  rather than its own section, the heading got eaten, leaving two
  sections merged into one. The fix-and-recover sequence took 3 extra
  patches and a final `write_file`. **Mitigation**: when patching
  across a section boundary, include the next-section heading in
  *both* `old_string` (to anchor) and `new_string` (to preserve).
  Test the patch conceptually before sending: "does my `new_string`
  re-emit every heading it consumes?" If a heading appears in
  `old_string` but not in `new_string`, the heading will be deleted.
- **The PASS line is a LIE if you write it before trimming stops.**
  Common failure: write the full note with a PASS line estimated
  from mental character count (e.g. "I have ~10K of content, so
  PASS: 10000 bytes"), then patch-trim three times to fit under
  the ceiling — and never go back to update the PASS line. The
  parent's `wc -c` then reports e.g. 11,353 bytes while the PASS
  line still claims 9,562. **Treat PASS-line update as atomic
  with the final `wc -c`**: the very last `write_file` / `patch`
  must be followed by `wc -c <path>` (capture the number), then a
  `patch` that replaces the PASS line with that exact number. If
  you cannot do all three, FAIL rather than ship a wrong PASS line.
- **A placeholder PASS line is WORSE than a wrong number.** Writing
  literal `PASS: <bytes> bytes, <outbound_links> outbound links` with
  angle-bracket placeholders is the worst variant: the parent's
  `grep -c 'PASS:'` matches the literal string "PASS:" regardless
  of what follows, so a placeholder sneaks past grep-based verifiers
  and only fails when a human runs `tail -1 <file>` and sees the
  angle brackets — which the parent often does not. If you cannot
  compute the exact final byte count and outbound link count before
  writing the file (e.g. you ran out of budget mid-trim), write
  `FAIL: could not compute final PASS line` instead. The parent will
  re-run you with a tighter brief. A placeholder PASS line in the
  output is a *worse* failure than oversize content, because it
  implies the leaf finished cleanly when it didn't.
- **Mis-estimated PASS numbers exhaust budget silently.** Session-evidenced:
  an agent wrote `PASS: 9212 bytes, 8 outbound links` after producing an
  11,489-byte file and then made four more trim attempts. None of the
  four re-read the in-file PASS line to update it. The final file
  shipped at 11,353 bytes with the placeholder version
  (`PASS: <bytes> bytes, <outbound_links> outbound links`), making the
  PASS line both wrong and unparseable. The lesson: when the byte
  count is in flux, defer the PASS line edit to the very last tool
  call before printing your summary. Don't write it speculatively.
- **"Trimming" that doesn't trim.** Session-evidenced failure:
  re-`write_file` the same content three times while believing
  each pass is shorter, only to discover `wc -c` returns the
  identical byte count each time (because the content is in fact
  the same). Each "trim" iteration must change the *content*
  measurably. Rule: if your patch diff shows the line count
  unchanged and the byte delta is <200 bytes, you're not trimming
  — go re-draft with explicit cuts (drop a bullet, shorten a
  paragraph) rather than re-save the same text. Better yet, budget
  per-section in advance (see "Byte-budget discipline") so the
  final draft lands in range on the first try.
- **Patching overcounts (or undercounts) bytes.** Patch tool
  diffs sometimes leave behind differing whitespace than you
  expect; a single-line "tighten" can shrink by 5 bytes or grow
  by 200 depending on line-break handling. After every patch
  trim, re-run `wc -c` — don't trust the diff to tell you the
  net delta. Multiple trims without re-checking is how a note
  finishes at 11,353 bytes with the agent believing it's at
  9,500.
- **Failing the `PASS:` line.** The parent agent greps the last
  line of the output. If you forget to update `<bytes>` after a
  trim, or the count is wrong, the parent flags a FAIL. Run
  `wc -c <path>` and re-count unique outbound wikilinks (excluding
  `Research/Papers/...` source refs) immediately before writing the
  PASS line.
- **The PASS line is itself part of the byte count it reports.**
  PASS reads `PASS: <N> bytes, <K> outbound links`, and the file's
  final `wc -c` *includes* this line. So `N` must equal the file
  size *with* the PASS line appended. Naive calculation `N =
  len(rest_of_file)` is wrong by however many bytes the PASS line
  contributes. Solve the fixed point: iterate 2–3 times, each time
  setting `candidate_pass = f"PASS: {len(rest + candidate_pass + newline)} bytes, {K} outbound links"`,
  re-measuring. Converges in 2 iterations in practice (digits of N
  rarely change between iterations). Session-evidenced 2026-07-29
  (agent-loop): the file stripped of PASS was 11,777 bytes; first
  estimate `N=11777` produced a 50-byte PASS line → real size
  11,827; re-solving with `N=11827` produced a 50-byte PASS line
  → real size 11,827. If you can't iterate, fall back to a
  `terminal` heredoc — see the next pitfall for the regex trap.
- **`execute_code` regex literals get over-escaped through the
  tool's sandbox compiler.** When computing `re.findall` for
  outbound wikilinks via `hermes_tools.read_file/write_file` in
  `execute_code`, raw patterns like `r'\[\[([^\]|]+)\]\]'` may
  raise `re.error: unbalanced parenthesis at position 15` because
  the sandboxed compile consumes backslash escapes twice. Two
  reliable workarounds: (1) `terminal(command="python3 -c \"import
  re,os; ...\"")` with a heredoc — the shell pass escapes
  backslashes predictably; (2) build the pattern from raw strings
  with `re.compile('\\[' + '\\[' + '([^\\]|]+)' + '\\]' + '\\]')`
  to avoid the literal-bracket trap. The terminal path is faster;
  the regex-from-strings path is safer when `terminal` is blocked.
  Session-evidenced 2026-07-29 (agent-loop): burned 2 tool calls
  on a wrong read_file shape + the regex compile. One `terminal`
  heredoc would have done both.
- **`patch` rejects identical-string hunks and aborts the *whole*
  multi-hunk patch.** A V4A patch where any single hunk has
  `old_string == new_string` returns `hunk N not found — old_string
  and new_string are identical` and **rolls back all the other
  hunks that already succeeded in the same call** (the patch tool
  validates the whole patch before applying any hunk). Symptom:
  you burn a tool call on a 10-hunk patch, get back the
  identical-string error, and discover 9 perfectly good edits got
  dropped. Fix: before sending a multi-hunk patch, deduplicate
  hunks whose `old_string == new_string` (just remove them).
  Session-evidenced 2026-07-29 (agent-loop): a 10-hunk patch
  dropped 8 successful edits because one hunk was a no-op;
  recovered with a follow-up 9-hunk patch after dropping the
  no-op.
- **`hermes_tools.read_file` in `execute_code` returns dict
  shape `{"content": "..."}` — `result["content"]` works;
  `result.content` raises `KeyError: 'content'`.** Always confirm
  the shape with `print(type(result), list(result.keys()))` on
  the first call when debugging.
- **PDF as primary source → use `pymupdf`.** When the brief's
  primary source is a paper PDF (e.g. `Research/Papers/Kimi-Audio/
  Kimi-Audio Technical Report.pdf`) and there is no
  `raw/articles/<paper>.md` summary, the verbatim PDF is the
  substrate. Install `pymupdf` if not present
  (`pip install pymupdf`), then `pymupdf.open(<pdf>)[i].get_text()`
  per page. Search for the section headings the brief cares
  about (here, "Audio Detokenizer" or "flow matching") to skip
  ahead. Quote exact section numbers and numbers verbatim — the
  parent's verifier checks that mechanism claims trace to the
  report, not to general background knowledge. The Kimi-Audio
  session in this playbook's history used this technique for
  chunk size (1s), look-ahead width (n=4), upsampling factor
  (4x), training stages (3), and the Qwen2.5-7B initialization.
- **Citation marker paths can be `^[Research/Papers/.../...md]`
  too, not only `^[raw/articles/...md]`.** The brief's example
  uses `raw/articles/`, but the existing aiml-wiki notes
  (multi-head-latent-attention, kimi-delta-attention,
  multi-token-prediction) routinely use the `Research/Papers/...` form
  when the source is the paper's own vault note rather than a
  `raw/articles/` summary. Both forms are accepted; pick the one
  matching the frontmatter `sources:` list. The hard rule is
  *consistent* citation: every paragraph that synthesizes ≥3 sources
  or traces to a specific paper gets *some* marker pointing
  at the same path style used in the frontmatter.
- **Citation markers use single brackets, not double.** The marker
  syntax is `^[raw/articles/<source>.md]` (one `[` and one `]`),
  NOT `^[[raw/articles/<source>.md]]` (two `[` and two `]`). The
  latter is a malformed citation that the wikilink regex
  `\[\[([^\]|]+)\]\]` will accidentally match as a real outbound
  link, inflating the link count and leaving a stray `[[...]]`
  inside the citation marker. Symptom: `re.findall(r'\[\[([^\]|]+)\]\]', text)`
  returns the source path as one of the targets. Session-evidenced
  2026-07-29 (babylm-2026 raw/articles wave10): the regex flagged
  `raw/articles/babylm-2026.md` as a 9th outbound link, but the
  baseline was 8. The fix is to re-read every `^[[` in the file and
  collapse each to `^[` — don't try to strip the inner brackets
  alone, just rewrite the whole marker.
- **Python `\a` `\b` `\t` `\n` `\r` `\f` `\v` escape sequences
  corrupt LaTeX commands during `execute_code` string substitutions.**
  When the brief's content includes LaTeX like `\alpha`, `\beta`,
  `\top`, `\times`, `\eta`, `\to`, `\theta`, etc., and you use
  `execute_code` to do bulk text replacement (e.g. iterative byte
  trims), the Python string literal containing `\alpha` is parsed by
  Python's escape handler — `\a` becomes ASCII bell (0x07),
  `\b` becomes backspace (0x08), `\t` becomes tab (0x09), `\n` becomes
  newline (0x0A), `\r` becomes CR (0x0D), `\f` becomes form feed
  (0x0C), `\v` becomes vertical tab (0x0B). When the substitution is
  written back, the file has `\x07lpha`, `\x08eta`, `\x09top`,
  `\x09times` instead of `\alpha`, `\beta`, `\top`, `\times`. The byte
  count barely changes (one byte swapped for one byte), so the trim
  dance looks like a no-op rather than a corruption. Three defences:

  1. **Raw strings for patterns:** `content.replace(r'\alpha', r'\alpha')`
     — the `r` prefix disables Python's escape interpretation, so
     `\a` stays as the two characters `\` and `a`.
  2. **Double-escape in regular strings:** `content.replace('\\alpha', '\\alpha')`
     — in a regular string, `\\a` is the two characters `\` and `a`.
  3. **Read the file as bytes and patch with bytes:** `data = open(path, 'rb').read(); data = data.replace(b'\x07lpha', b'\\alpha'); open(path, 'wb').write(data)`.
     This is the most reliable **detector**: after the substitution,
     scan `data` for any byte in `[0x07, 0x08, 0x09, 0x0B, 0x0C, 0x0D]`
     (skip 0x0A = newline, expected) and confirm none remain inside
     LaTeX/math sections. Session-evidenced 2026-07-29 (moonshot-ai
     wave12): the leaf detected `\x07lpha` (bell) and `\x08eta`
     (backspace) via `data.count(b'\x07')` after a trim round, fixed
     via byte replacement, but left four `\x09top` / `\x09times` (tab)
     sequences in the file because the math-line anchors used by
     subsequent `replace` calls did not match the corrupted bytes
     position-for-position. The leaf reported PASS at 9,966 bytes but
     the LaTeX in the file was still corrupt.

  **Mandatory pre-ship check for any note with math:** after the
  final `wc -c`, run `data = open(path, 'rb').read(); assert not any(b in data for b in [0x07, 0x08, 0x09, 0x0B, 0x0C, 0x0D])`.
  If any of those bytes appear inside the Math / Mechanism sections,
  the file is corrupt and the leaf must either (a) byte-patch the
  bad sequences back to their intended `\command` form, or (b) re-draft
  the affected lines via `write_file` so the patterns are written
  fresh without going through `execute_code`'s escape parser.

## Dispatch mode and pool capacity (parent-side orchestration)

The leaf subagent's contract is unchanged — it writes one file, one PASS
line, then exits. What is NOT in the leaf contract is *how* the parent agent
should dispatch 10+ such leaves in a row without losing hours of work to a
mid-burn crash. Session-evidence from the 2026-07-29 aiml-wiki burn (50
notes, 41 dispatched subagents, ~4.5 hours wall-clock):

### Three rules for the parent agent running this burn

1. **Output to durable disk, not `/tmp/`.** `/tmp/` is tmpfs on this host
   and was wiped mid-burn by a snap/systemd recovery event (23:26 IST
   2026-07-28, observed). Round 1 of the burn lost 9 of 12 wave outputs
   because they sat in `/tmp/aiml-burn/`. Round 2 of the burn switched to
   `/home/arctic/projects/aiml-burn-tmp/` and survived a parallel
   snap-recovery fine. Default for any future aiml-wiki burn:
   `mkdir -p /home/arctic/projects/aiml-burn-tmp/wave{0..15}` and tell the
   brief to use that path verbatim. This is in addition to the parent-side
   rule under SKILL.md → Idempotency + safety → "Stub files are sacred /
   Output dir must be durable / Single-task dispatch / Poll-and-apply /
   Sync-pool fallback / HTTP 429 recovery".

2. **Single-task `delegate_task(goal=...)`, not batched `delegate_task(tasks=[...])`.**
   The current Hermes parent-side batch wrapper crashes with
   `"Delegation owner exited before recording a terminal result;
   outcome unknown"` on every batched dispatch of 3+ tasks — even when
   every subagent finished and wrote its file. Verified by 41 single-task
   dispatches (all returned inline cleanly) vs the prior 12 batched
   dispatches (all errored the same way, with subagent outputs already on
   disk). The pool-level message *"ran SYNCHRONOUSLY: The background
   delegation pool was at capacity"* is unrelated and *not* a problem —
   it just means the subagent ran inline rather than queued. Slow
   (~6–10 s overhead per single-task dispatch) but reliable.

3. **Poll-and-apply, not end-of-wave apply.** Parent loops `ls /home/arctic/
   projects/aiml-burn-tmp/waveN/` every 60–90 seconds and `cp` each
   finished file to the real wiki path immediately. This makes the burn
   resumable at any point: at any moment, the only in-progress work is
   whatever's currently in flight; everything on disk has already landed
   in the real vault. Re-running `build_index.py --check` after each
   apply gives a near-live `broken=0` baseline that catches drift
   (e.g. subagent writing a `[[Concepts/RoPE (Rotary Position
   Embeddings)]]` cross-vault bridge) within minutes, not after 41
   subagents have all finished.

### Throughput expectations

- 41 subagents × ~10 min/subagent ≈ 7 hours of subagent work
- Effective wall-clock for the burn: 4.5 hours (single-task mode + poll-
  and-apply + occasional HTTP 429 pauses)
- Token spend estimate: ~37M tokens total (subagent input/output + parent
  orchestration); per-subagent ≈ 600K–1.3M input + 16–45K output
- Aggressive parallelism (10+ concurrent) does NOT help when the
  subagent pool reports "ran SYNCHRONOUSLY" — effective throughput is
  ~1 subagent at a time regardless of advertised concurrency when the
  pool is full. Plan wall-clock assuming single-track subagent work, not
  the `max_concurrent_children` number.

### HTTP 429 recovery

When a subagent hits the rate limit mid-trim, its transcript ends with
`exit_reason: max_iterations` and the PASS line is missing or stale.
Two things to check on disk:

- **File exists with content?** Apply it as-is. The 10–22 KB outputs
  are slightly over the 6–10 KB target range but are schema-correct.
- **File missing or a stub?** Re-dispatch the same brief with no changes.
  Rate limit windows typically clear in 60–90 seconds; a single retry
  succeeds almost every time.

Don't refund tool calls on a 429 hit. The leaf's transcript already
captured most of the working reasoning — re-dispatching a fresh leaf
costs the same as one more dispatch in the batch.

## Worked example shape (multi-token-prediction)

Skeleton for an aiml-wiki concept deep-enrichment (illustrative, not a
copy of the actual output):

```
---
title: <Concept Name>
created: <YYYY-MM-DD>           # preserved
updated: 2026-07-29             # only field changed
type: concept                   # preserved
tags: [...]                     # preserved
sources:
  - Research/Papers/...md       # preserved verbatim
confidence: medium              # preserved
---

# <Concept Name>

> <one-line tagline blockquote>

## Overview
[2-4 sentences, with `^[raw/articles/...]` on the source-tracing line]

## History / Motivation
[origin, why, what it generalizes]

## Mechanism
[how it works at training and inference]

## Math
$$\mathcal{L} = ...$$
[mark "not in vault sources" where applicable]

## Implementation
[practical steps, hyperparams, micro-opts]

## Variants
[parallel vs. chained, etc.]

## Use cases
[3-5 bullets with citations]

## Trade-offs
[honest cost/benefit]

## Connections
- [[wikilink-1]] — one-line role
- [[wikilink-2]] — one-line role
...

## Open questions
- <what's not in the sources>

## References
- [[Research/Papers/...|alias]]
- ^[raw/articles/...md]

> Confidence medium: <what the vault actually records, restated>

PASS: <bytes> bytes, <count> outbound links
```

## After the leaf writes

The parent agent runs:

- `wc -c <output>` to confirm size
- `tail -1 <output>` to read the PASS/FAIL line
- `grep -oE '\[\[[^]]+\]\]' <output> | sort -u` to count outbound
  links (excluding the `Research/Papers/...` source ref)
- A `build_index.py --check` re-run to confirm no new broken links
  were introduced (the leaf is supposed to verify this *before*
  writing, but the parent re-checks)
- A read-through of the Open questions and References sections to
  gauge the note's reliability before applying to the real vault

The leaf does NOT apply the enrichment to the real note. The parent
does that after verification.

**Don't run all five checks by hand.** `scripts/verify_aiml_wiki_output.py`
collapses them into a single CLI call that prints `[PASS]/[FAIL]` per check
and exits non-zero on any FAIL:

```
python3 scripts/verify_aiml_wiki_output.py /home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md
python3 scripts/verify_aiml_wiki_output.py /home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md --baseline-broken 0
```

Use it after EVERY trim round during the byte-budget dance; that's how
you catch the "accidentally deleted a section because the patch's
`old_string` was broader than its `new_string`" failure mode without
reading the whole file back. Pass `--baseline-broken N` to enforce the
no-regression rule (current `broken=` from `build_index.py --check` must
not exceed the pre-write count).