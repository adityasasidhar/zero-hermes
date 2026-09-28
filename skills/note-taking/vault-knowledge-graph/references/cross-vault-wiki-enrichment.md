# Enriching an llm-wiki destination from a personal vault

Cross-pollination pattern: the user has an llm-wiki at one path (e.g.
`/home/arctic/Documents/fun/wiki/aiml/`) and a personal vault at the
same root (`/home/arctic/Documents/fun/`). They want the wiki's
research-side material to be a canonical mirror of what the vault
holds. This file captures the verified workflow.

## Why this needs its own playbook

The default `llm-wiki` skill (Karpathy's pattern, ingest-from-source) is
the right schema/structure reference for the **destination** wiki. The
vault-knowledge-graph skill covers the **origin** vault. This reference
sits between them: it is what you actually run when the user asks you
to "enrich the aiml wiki from the vault".

The work consists of:

1. Picking 4-6 high-impact items from `Research/Papers/`.
2. Writing each as a condensed raw article in the wiki (paraphrasing the
   vault note — never copying).
3. Creating 3-5 concept pages backed by ≥2 sources each.
4. Cross-linking the existing entity pages into those new concepts.
5. Adding one comparison page if (and only if) a justified concept pair
   exists.
6. Updating `index.md` and `log.md`.

Steps are validated after each one, not at the end (an end-only
validation can hide per-step regressions).

## Hard rule

**Never invent facts.** Every claim in a new wiki page must trace to one of:
(a) an existing `raw/articles/*.md` in the wiki, (b) a vault paper note at
`/home/arctic/Documents/fun/Research/Papers/<X>/<slug>.md`, or (c) a vault
repo note at `/home/arctic/Documents/fun/Github/Repos/<repo>.md`.

If a claim has no traceable source, do not write it. Wiki pages read like
synthesized knowledge — it is easy to slip in "facts you know" about the
topic that didn't actually come from any source. Don't do that.

## The five steps (run after orient)

Step 0 is the standard llm-wiki orientation: read the destination's
`SCHEMA.md`, `index.md`, recent `log.md`, and run `find … -name '*.md'`.
Steps 1-5 are the enrichment itself.

### Step 1: Pick sources and write raw articles

Choose 4-6 high-impact items from `Research/Papers/` (canon entries +
recent moves). For each:

- Read the corresponding vault paper note first
  (`Research/Papers/<X>/<slug>.md`) to see what claims are actually
  supported.
- Write a `raw/articles/<slug>.md` with the schema-mandated raw
  frontmatter: `source_url`, `ingested`, `sha256` (of body only),
  optional `arxiv_id`.
- Body = a condensed paraphrase of the source's key claims. **Do not
  copy** — treat the raw article as your own re-statement. 200-400 words.
- Cite the vault note path at the bottom so a future reader can trace
  back.

**Digests must be real.** Compute `sha256: <hex>` over the body
(everything after the closing `---`), not placeholder text. A validator
that recomputes the digest catches the placeholder-`PENDING` case and
forces the real hash before counting a raw article as complete.

### Step 2: Add concepts that source ≥2 raw articles (or 1 + vault note)

Gate on real backing. Required:

- Full schema frontmatter (`title`, `created`, `updated`, `type: concept`,
  `tags` from SCHEMA.md taxonomy, `sources`, `confidence`).
- Blurb 100-200 words (validated by word count).
- ≥2 outbound `[[wikilinks]]` to existing concept pages in the wiki.
- Optionally 1-2 `^[raw/articles/foo.md]` provenance markers per major
  paragraph (per SCHEMA.md "Provenance markers" rule).

A concept that only one raw article supports is a list-of-candidates
item, not a concept page yet. Wait for the second source.

### Step 3: Cross-link the entities

For each entity page in `entities/`, ensure it links to ≥2 concept pages
that already exist. Read the entity body, justify each concept link from
the entity's content, then add the link. **Do not** link to a concept
that hasn't been written — broken links accumulate.

Tag the entity's `updated:` date too — every modification through this
skill must bump the date even if the body change is link-only.

### Step 4: Add comparisons only when a justified pair exists

A comparison page is justified when two concept pages can be placed side
by side on real dimensions AND the available sources support that
comparison. For example, `[[recurrent-state]]` vs `[[attention]]` for
long-context mechanism is supportable (FlashAttention + RoPE + BabyLM
GDN source).

If you can't find 2 such pages, **skip the step** — leaving
`comparisons/` untouched is correct. A forced comparison without
sources is worse than no comparison because it launders implied
authority.

### Step 5: Update `index.md` and `log.md`

- `index.md` header: bump `Total pages: N → M`, `Last updated:` to today.
- `index.md` body: add each new page to the right section with a one-line
  summary.
- `log.md`: append `## [YYYY-MM-DD] enrich | Vault cross-pollination
  batch` with two lists — "Files created (N):" and "Files updated (M):"
  — naming every path you touched. A reader grepping `log.md` to find
  where path `X` was written depends on every path being there.

## Validate after each step, not just at the end

A single end-of-session validation can hide per-step regressions.
Validate incrementally:

- **After step 1:** each new raw article has required frontmatter
  (`source_url:`, `ingested:`, `sha256:`), the sha256 recomputes to the
  body, and the body has substantive content (≥180 words for a
  meaningful re-statement).
- **After step 2:** schema frontmatter complete, blurb length in
  `[100, 200]`, tags all in the SCHEMA.md taxonomy, no broken outbound
  wikilinks (target basename resolves on disk), ≥2 outbound links.
- **After step 3:** for each entity, the new concept references resolve
  to existing concept pages; each entity has ≥2 concept links; updated
  dates are bumped.
- **After step 4:** schema frontmatter required fields, ≥2 outbound
  links, no broken links.
- **After step 5:** index links equal the set of content pages, index's
  `Total pages` matches the page count, log contains the batch entry
  listing every touched file.

## Wikilink-resolution checklist (run at the end)

A full-walk broken-link check catches what per-step checks miss:

1. Build the set of all `.md` basenames in the wiki (recursively).
2. For every `.md` (including SCHEMA.md and index.md), extract every
   `[[wikilink]]` (strip pipe aliases and `#anchors`), look up its
   basename.
3. Classify broken links against a known-pre-existing set; the budget
   is *zero new broken links*.
4. Report `Pre-existing broken (unchanged): N, Newly broken: M`.
   M must be 0.

**Pre-existing broken baselines to track.** A given wiki will have
unrelated broken links from before this session (doc-example
`[[wikilinks]]` in SCHEMA.md, references to upcoming concepts in
captured pieces). Track them explicitly so a per-session check can
confirm only zero *new* broken links were introduced.

## Pitfalls specific to this workflow

- **Placeholder `sha256:` values are not real digests.** Raw article
  templates sometimes ship with `sha256: PENDING` and rely on the
  writer to fill it in. Forgetting to fill it in leaves an inert
  field that is a lie on re-ingest. Compute
  `hashlib.sha256(body.encode()).hexdigest()` and substitute *before*
  the raw article is complete.

- **Tag typos break the validator.** A tag not in SCHEMA.md's taxonomy
  (e.g. `training` instead of the canonical `pretraining`) makes
  frontmatter non-conformant even if the page reads correctly. When
  the taxonomy is strict, scan tags against the allowed set extracted
  from `SCHEMA.md` (the lines beginning `- \`<tag>\``) before declaring
  step 2 complete.

- **The "skip if no pair" gate is real — don't pad comparisons.** A
  forced comparison without sources is worse than no comparison
  because it launders implied authority. If step 4 cannot justify a
  pair from the available sources, log "step 4 skipped — no justified
  concept pair" and move on.

- **Don't link to concepts that haven't been written yet.** Writing 5
  concept pages in one batch requires writing them in the right order:
  resolve all sibling-`[[wikilinks]]` before writing any. The pattern
  (`pending_concepts` set + `pending_targets_for[name]` set + only call
  `write_file` after every target is known) lives in
  `references/graph-growth-levers.md` → "See also round-trips across
  a batch".

- **Compare blurb word counts against SCHEMA.md's page thresholds, not
  against your own taste.** SCHEMA.md may specify range sizes for
  concept pages; treat those ranges as binding even if the page reads
  shorter to you. Validation: `100 <= words <= 200` for blurb-on-concept.

- **Log entry must list every file.** The log entry is a record of
  disk operations, not a summary of intent. List every path you
  created or modified. A future session grepping the log to answer
  "when did we introduce `X`?" depends on every path being there.

- **Don't break the prose-only paraphrase rule on raw articles.** The
  raw article and the vault note can have identical structure but
  different wording. Treat the raw article as your re-statement — the
  whole point of having a `raw/` layer is that the vault note is the
  source of truth and the wiki rewrites it in the wiki's voice.

- **Validated broken-link budget must be zero for *new* broken links,
  not zero total.** A session that touches related concept pages can
  introduce new broken links (e.g. adding `[[attention]]` somewhere
  when `attention` still has no page). The per-session check is
  delta-from-baseline, not absolute zero.

## Recipe: cross-vault enrichment pass (any wiki)

Same playbook, adapted for other research wikis:

1. Read the destination's `SCHEMA.md` — tag taxonomy, frontmatter
   fields, page thresholds. Different wikis tag differently.
2. Read the vault structure — find `Research/Papers/`, `Github/Repos/`,
   `Concepts/`, and the MOC note that governs each.
3. Pick a 4-6 paper batch from the vault's most-cited entries.
4. Run steps 1-5 as documented above, validating after each.
5. The skill improvement at the end is the wiki, the vault, the log,
   and the index — plus the report-back naming every file.

## Cross-references

- `references/project-notes-enrichment.md` — the SAFE enrichment
  pattern (temp file → verify → apply) is similar in shape to this
  five-step batch; the same temp-then-verify discipline applies to
  large concept-creation batches.
- `references/graph-growth-levers.md` → **Atoms vs hubs for
  paper-side concepts** — same ≥2-paper threshold is right for
  concepts in this workflow, with the caveat that one-paper
  concepts are sometimes wanted when the paper is canon
  (not in this wiki's threshold model).
- `references/graph-growth-levers.md` → **See also round-trips
  across a batch** — exact pattern for ordering concept-creation
  to avoid cascading dangling wikilinks.
- `references/graph-growth-levers.md` → **Inline-code false-positive
  trap** — the wikilink verifier must strip backtick-wrapped prose
  before counting; backticks in this file's own examples would
  otherwise show up as broken.
