---
name: vault-knowledge-graph
description: "Build and bulk-enrich an Obsidian-style personal knowledge vault (project notes, repo notes) from real sources — GitHub metadata, READMEs, and cloned code — using safe delegated subagents. Covers note schema, gh data pulls, and the verify-before-apply delegation pattern."
version: 1.1.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
---

# Vault Knowledge Graph

When the user asks to populate, structure, or bulk-enrich a personal knowledge
vault (Obsidian-style: `[[wikilinks]]`, MOC index notes) — especially project
notes sourced from real data rather than invented text — follow this playbook.

This came out of enriching 50+ GitHub-repo notes + 3 local projects for a
user's vault at `/home/arctic/Documents/fun`, pulling real data from `gh` and
cloned source.

## Golden rules
1. **Never fabricate vault content.** Pull real metadata (GitHub API), real
   READMEs, and read the cloned source. Fall back to existing index descriptions
   only when a repo has no README — and label it as such.
1a. **Search the vault BEFORE asking the user "who/what is X".** When the user
   mentions a name (person, project, repo, paper, place, tool) or a
   referenceable identifier, grep the vault + the 4 llm-wikis
   (`/home/arctic/Documents/fun/wiki/{aiml,learning,personal,work}/`)
   *before* asking a clarifying question like "who is X" or "where should
   X go." User has corrected this explicitly: *"Why are you not
   automatically checking my knowledge graph?"* The graph is the source
   of truth — asking the user to repeat what the graph already says is
   a failure mode, not a clarification step. **Concrete recipe** before
   any "who/what" question:
    - `search_files(pattern=<name>, target='content', path='/home/arctic/Documents/fun')`
      (catches the hand-vault `People/`, `Projects/`, etc.)
    - `search_files(pattern=<name>, target='content', path='/home/arctic/Documents/fun/wiki')`
      (catches the 4 llm-wikis)
    - `search_files(pattern='<name>*', target='files', path='/home/arctic/Documents/fun/People')`
      (catches the `contacts.csv` / `special_people.csv` phonebook)
    - If the user gave a *handle / alias* (e.g. "nancy", "the dataobs
      project"), also search `/home/arctic/projects` for matching
      directories: `search_files(pattern='*<handle>*', target='files',
      path='/home/arctic/projects')`.

   **Case-insensitive default when the user references a known-named
   note.** When the prompt is *"I have a list/note of X, check it
   out"* / *"my notes on Y"* / *"find my file about Z"* (signals the
   user expects a specific existing file with a known name), default
   to a case-insensitive broad regex on the first search — for example
   `re.compile(r"competition|competitions|contest|hackathon|challenge", re.I)`
   over the vault via `execute_code` with `os.walk`, NOT a narrow
   literal-substring `search_files("competition")`. `search_files` with
   `target='files'` is exact-match on basename; with `target='content'`
   it requires the regex to match against the file body — either can
   miss a file whose name is lowercase + spaces (`competitions to be
   in.md`) or whose body has zero matches of a narrow term. Session-
   evidenced 2026-08-10: a narrow literal `search_files("competition",
   target='content')` missed `/home/arctic/Documents/fun/competitions to
   be in.md` (the body has just 4 lines, no occurrence of the literal
   word "competition" — the file lists "e-yantra", "arc agi", etc.).
   The user had to nudge *"try to find it"* before the broad-regex
   pass surfaced the file via a content scan with synonyms. Rule of
   thumb: **start broad when the user is pointing at a named file**
   ("my list", "my notes", "the file about X"), narrow only after
   the broad pass returns candidates. The user's mental filename and
   the on-disk filename are usually NOT identical — capitalization,
   spacing, plurals, synonyms, and abbreviations all drift.

   Only after the searches come back empty (or surface conflicting
   candidates) is it OK to ask. **Hard cap on clarifying questions:**
   one round max for this class of lookup; if you still don't know,
   make a reasonable default and announce it ("I'll assume X unless you
   tell me otherwise") rather than asking again.
2. **Subagents must NOT edit vault notes directly.** They write an enriched
   FULL copy to a temp file; you verify, then apply. A flaky agent must never
   blank/corrupt a note.
3. **Wire new notes into a MOC** with `[[wikilinks]]` and verify every link
   resolves, so nothing is orphaned.
4. **Read `CLAUDE.md` / `AGENTS.md` at the vault root before any bulk work.**
   Vaults frequently declare out-of-band policies there (e.g. *"empty stub
   files are deliberate placeholders, don't fill them in"*) that override what
   the diagnostic scripts suggest. Without this read, you'll propose fixes
   for things the user explicitly told themselves not to fix.
5. **Wikilinks resolve against `splitext(basename)[0]`, case-sensitively.**
   When writing a new note that links to an existing folder `Foo-Bar/` whose
   `.md` is `foo-bar.md`, write `[[foo-bar]]`, not `[[Foo-Bar]]` — even if
   `Foo-Bar` reads better in prose. See
   `references/bulk-reorganization.md` §"Filename character pitfalls".
   **Three-style resolution rule for writing new wikilinks** (verified
   2026-07-29 across 60+ wikilinks added during the Aditya profile +
   8-facet cluster build): pick the resolution style by target
   ambiguity, NOT by which form "looks right" in prose:

   - **Bare basename** — when the target resolves to exactly one
     note across the entire vault. Preferred, default form.
     `[[small-language-model]]`, `[[rmsnorm]]`,
     `[[babylm-recursive-hybrid]]`. Uses the resolver's `by_stem`
     lookup; works regardless of where the target note lives in the
     vault tree.
   - **Path-with-filename + pipe-alias** — when the bare basename
     resolves to many notes (vault has multiple `aiml.md`,
     `learning.md`, `index.md` matches etc.). The disambiguator.
     `[[wiki/aiml/index|aiml]]`, `[[wiki/learning/index|learning]]`,
     `[[Projects/Projects|the Projects hub]]`. Pipe-alias is
     OPTIONAL but reads better in prose.
   - **Prose with absolute filesystem path** — when the target is
     a file OUTSIDE the vault (e.g. `/home/<user>/projects/...`).
     Wikilinks can't resolve across the vault boundary, so use
     `` `/home/<user>/projects/night-oss-list.md` ``. Don't try to
     wikilink these.

   **Critical corollary**: folder-only wikilinks like `[[Foo-Bar]]`
   (where `Foo-Bar` is a folder, not a note) are silently broken even
   when `Foo-Bar/foo-bar.md` exists — the resolver does not walk
   folders. Always use the bare basename (`[[foo-bar]]`) or the full
   path including filename (`[[Foo-Bar/foo-bar]]`). See
   `references/broken-link-repair.md` Pitfalls → "Folder-path link
   targets" for the resolver mechanics + diagnostic snippet.

   **Sub-corollary: `..`-prefixed wikilinks are silently broken too.**
   `build_index.py`'s resolver (`wiki/build_index.py:223-226`) treats
   the target literally — for `[[../Daily Memory|Daily Memory]]` it
   sets `want = "../Daily Memory.md"` and looks for files matching
   `c.lower() == want` or `endswith("/" + want)`. Neither matches,
   because the resolver does NOT normalize `..` segments. Session-
   evidenced 2026-08-03 (Daily Memory backfill, 18 subagents): all
   18 daily notes initially shipped with `[[../Daily Memory|Daily
   Memory]]` and `[[../../Me|Me]]` and produced ~50 broken links in
   a single shot. The fix is to use **bare basenames** for vault-
   internal links from any depth — `[[Daily Memory]]`, `[[Me|Me]]`,
   `[[People/Nancy Sharma]]` (forward path-with-filename is fine; the
   resolver handles it). **Rule of thumb for parents writing briefs**:
   never tell subagents to use `..` in wikilinks. After any bulk
   write from a deep folder, the parent must run `build_index.py
   --check` and patch any `[[../...]]` / `[[../../...]]` patterns
   before declaring the batch done. The patcher is also finite — if
   a subagent re-dispatch re-introduces the bad form (rare, observed
   on the 07-29 retry), re-run the patch + `build_index.py --check`
   *immediately after* the subagent lands, not at end-of-batch.

   **Pre-write validation gate** (verified 2026-07-29): before
   `write_file` on a new note with wikilinks, run a 2-second\n   verification — `grep -lE '\\[\\[[^]]+\\]\\]' <new_note_path> | head`\n   then for each unique target basename, `find <vault> -name \"<basename>.md\"`\n   and confirm exactly one match (or use the path-with-filename\n   form if multiple). This catches ALL three broken-link\n   patterns (wrong namespace prefix like `[[aiml-entities/babylm-recursive-hybrid]]`,\n   ambiguous bare basename like `[[aiml]]`, folder-only\n   `[[Hobbies/Artistic/]]`) before they ship. The first\n   cluster build in this session shipped 13 broken\n   wikilinks and required 5 patch iterations to reach\n   `broken=0` — the pre-write gate would have caught all\n   13 at write time.
6. **Verify resolved targets match intent, not just broken counts.** After any
   wikilink rewrite pass, "broken went from N to 0" is necessary but NOT
   sufficient. Check each *resolved* target — a wikilink that resolves to the
   *wrong* note is worse than a broken one, because it will silently send
   the user to unrelated content. Fast check: grep the resolver's output for any
   link where the target basename has ≥1 sibling with the same stem (the
   `ambiguous` count in `build_index.py --check`) and verify each resolves to
   the intended candidate. Caught 2026-07-26: a five-link fix almost shipped
   `[[Work/Internship/Dataobserve|DataObserve]] → [[internship|Dataobserve]]`
   resolving to `Work/Internship/internship.md` (the parent MOC, ~100 bytes)
   instead of the intended `Work/Internship/Dataobserve/Data Observe.md`
   (the subfolder MOC). Both have stems that match; the resolver's
   tie-breaker picked the shorter path. Always confirm intent before
   declaring a rewrite pass done.

## Note schema
Each project note = YAML frontmatter (machine-readable, good for Dataview) +
readable body. Full schema + `gh api` pull commands (incl. the private-repo
and jq-object-constructor gotchas) are in `references/project-notes-enrichment.md`.

**Two-pass enrichment.** The first-pass pattern (in `project-notes-enrichment.md`)
covers adding a ~250-word `## Inside the Codebase` section to stub notes.
For the **deep-enrichment** variant — taking notes from 1.5k → 10k+ chars with
a full 11-section schema, real tech stack, project Status, Use cases, and
Concepts/Themes cross-links — see `references/deep-enrichment.md`. The
deep-enrichment brief supports a "DEEPEN don't rewrite" mode for notes that
already have good detail. Use the 11-section schema target for any notes
the user wants comprehensively documented ("enrich the content even more",
"add the tech stack I used", "go in great detail about every aspect").

For the parallel **aiml-wiki research-note deep-enrichment** pattern (single
note in `wiki/aiml/concepts/`, `wiki/aiml/entities/`, OR `wiki/aiml/raw/articles/`,
BRIEF-driven, 11-section research schema — Overview / History / Motivation /
Mechanism / Math / Implementation / Variants / Use cases / Trade-offs /
Connections / Open questions / References — `^[raw/articles/<source>.md]`
citation markers, 6–10 KB byte target, durable-disk output at
`/home/arctic/projects/aiml-burn-tmp/waveN/`, no-fabrication hard rule), see
`references/aiml-wiki-deep-enrichment.md`. Different from the GitHub-repo
deep-enrichment above: different schema, different source convention, different
output path, different cross-link rules (must resolve in aiml wiki). Do NOT
apply the project-notes schema or the `/tmp/enrich-waveN/` output convention
to aiml-wiki work. The reference includes a per-section byte-budget table
(observed distribution from a 9,813-byte leaf) and a pre-emit stem-based
resolution probe for outbound wikilinks — both useful pre-write aids.

For the **aiml-wiki comparison-note** variant (notes in `wiki/aiml/comparisons/`, BRIEF-driven, 9-section Setup / Design / Hypothesis / Methodology / Results framing / Trade-off table / Open questions / Related work / References layout), see `references/aiml-comparison-notes-deep-enrichment.md`. Comparison notes are a distinct schema — the 11-section research-note pattern does NOT apply. Section-byte-budget risks are different (Design with math and Trade-off table dominate) and the byte-ceiling switch-to-FAIL threshold is broadened to fire on 3+ consecutive <100-byte trim iterations regardless of absolute distance from the ceiling. The reference includes an observed per-section budget table from a 12,404-byte wave14 attempt and the full trajectory of that file's convergence-pattern failure.

`raw/articles/` notes use a different frontmatter schema** than
concept/entity notes: `source_url`, `ingested`, `sha256`, `arxiv_id` instead
of `title`/`created`/`type`/`tags`/`sources`/`confidence`. The body still
uses the 11-section research schema and the same `^[raw/articles/...]`
citation markers, but the frontmatter preservation discipline is different
(see the "raw/articles sub-pattern" subsection in
`references/aiml-wiki-deep-enrichment.md`). When the brief calls out
*"This is a raw/articles note (different frontmatter: ...)"*, do NOT
retrofit `title:` / `created:` / `type:` / `tags:` — the parent agent
chose the raw/articles form deliberately, and `build_index.py` indexes
these notes by sha256.

## Local-only project notes (no GitHub remote)

When the user adds new local projects to `/home/arctic/projects/` (no GitHub
remote — `uv init` scaffolds, hand-written C/Rust/TS projects, from-scratch
Python exercises, etc.), generate notes using the **same schema** as
`references/project-notes-enrichment.md`, with three field swaps:

```yaml
---
repo: <name>
description: <one-line>
local_path: /home/arctic/projects/<name>      # ← required for local notes
github: none                                  # ← always literal "none"
visibility: local                             # ← always "local" (not public/private)
language: <primary>
topics: [a, b]
git_initialized: <true|false>                 # ← check `git rev-parse` exits 0
status: active | scaffold/in-progress | experimental/inactive | archived
created: YYYY-MM-DD                           # ← from filesystem mtime if no git
last_updated: YYYY-MM-DD                      # ← from filesystem mtime if no git
type: <auto-category>
vault_group: Projects                         # ← always "Projects" (not Public/Private Repos)
---
```

**Body sections are identical** to the GitHub-side schema (Overview, Status,
Inside the Codebase with 6 H3s, What this project is about, Use cases, Key
Features, Getting Started, Stats, Links). The `## Links` section ends with
`Part of: [[Projects]]` instead of `Part of: [[Public Repos]]` /
`Part of: [[Private Repos]]`.

**Re-state the local-only fields even if they seem redundant.** `local_path:`
and `git_initialized:` are required for future readers to find the source tree
and to know whether the source is under version control. The `projects-notes-enrichment.md`
reference already documents this in one sentence ("For local-only projects... swap
`github:` → `none`, `visibility: local`, and add `local_path:` + `git_initialized: false`");
this section makes it a full working contract.

## When the user says "make sure we add all"

When the prompt is some flavor of *"I have added new repos / projects / folders;
make sure we add all"* (verified 2026-07-31: *"i have added github repo and more
local projects make sure we ad all"*), the right operational read is:

1. **Inventory exhaustively.** Walk every candidate root, not just the obvious
   ones. For the GitHub side: `gh api "users/<owner>/repos?per_page=100"`. For
   the vault side: `find /home/arctic/Documents/fun -maxdepth 4 -name '*.md' -not
   -path '*/wiki/*'`. For the local side: `find /home/arctic/projects -maxdepth 4
   -name '.git' -type d` plus walk non-git roots separately.
2. **Classify each candidate** into one of: (a) `github-already-noted` (remote
   matches a vault note), (b) `external-upstream-clone` (external fork like
   `docling/`, `opencode/`, `gemini-cli/`, `claude-code/`, `doclang/` — usually
   upstream only, no user-authored work), (c) `vault-already-has-note` (existing
   `Projects/<X>.md` or `Research/<X>.md`), (d) `container-or-scratch` (`coding/`,
   `website/`, `aiml-burn-tmp/`, `kg-sync-tmp/`, empty
   `paper-reproduction/.trackio/`), (e) `UNCLASSIFIED-CHECK` (everything else —
   needs a real note).
3. **Surface alternate working trees of the user's own repos.** A directory
   whose `git remote get-url origin` matches another local working tree *and*
   whose HEAD is byte-identical to that tree is an alternate clone, not a new
   project. Verified 2026-07-31: `~/projects/marie/` (same remote + same HEAD
   `d178a0d` as `~/projects/Gappy/`) and `~/projects/babylm/` (same remote +
   same HEAD `f2ff97c` as `~/projects/recursive-babylm/`). **Don't write a new
   repo note for these.** Instead, add a `Local working trees:` line to the
   canonical note's `## Links` section, e.g.
   ```markdown
   - Local working trees: `~/projects/Gappy/` (canonical) and `~/projects/marie/` (alternate; same HEAD `d178a0d`, untracked `panchai_flowchart.html`)
   ```
   See `obsidian-github-sync` "Alternate working trees" pitfall for the full rule.
4. **Re-audit at the end.** Before declaring done, run a final pass with
   `find /home/arctic/projects -maxdepth 2 -type d | sort` and confirm every
   candidate is accounted for.
5. **Re-run `python3 wiki/build_index.py --check` before declaring done.** Confirm
   `broken` did not increase from before the sync, and that `notes` increased by
   the expected number (one per new note). When the sync creates new
   cross-links to concepts that don't yet exist, create 0-byte-or-short stub
   placeholders under `Concepts/<X>.md` (consistent with the vault's
   `CLAUDE.md` "empty stub files are deliberate placeholders" rule) and verify
   `build_index.py --check` no longer reports them as broken.

## Safe enrichment pattern
See `references/project-notes-enrichment.md` for the exact steps:
- Unique temp dir (e.g. `/tmp/enriched/`); one leaf subagent per repo.
- Each subagent: read note + cloned source → write FULL enriched note to temp
  (copy original verbatim, insert `## Inside the Codebase` after `## Overview`,
  preserve `Part of: [[Group]]`), never touch the real note.
- Verify each temp: `## Inside the Codebase` present, frontmatter intact,
  `Part of: [[..]]` preserved, file size grew. Then copy temp → real note.
- Dispatch in waves ≤ `delegation.max_concurrent_children` (default 3; raise
  with `hermes config set delegation.max_concurrent_children 10` — the agent
  cannot edit `~/.hermes/config.yaml` directly).
- **Fetching READMEs safely:** use `scripts/fetch_readmes.py` (guards `gh`'s
  exit code + decodes safely). The naive `gh api .../readme | base64 -d` pattern
  SILENTLY corrupts repos with no README (404 JSON decodes to ~38 bytes of
  identical garbage). See the **Pitfalls** section in
  `references/project-notes-enrichment.md` for the bug + the verify-before-apply
  gate that catches it.
- **Broken-link policy kernel:** `scripts/strip_wikilinks.py` is the
  reusable implementation of STRIP / CREATE_STUB / FUZZY_AUTO / AUTO_FIX
  policy application across a slice. Each bridge-slice subagent can run it
  as a CLI (`python3 strip_wikilinks.py <slice.json> <out-dir>`) or import
  `apply_policies` / `SPECIAL_CASES` as a library. The script handles
  four things at once: the regex strip patterns, the orphan-bullet drop,
  the special-case overrides (folder redirects, sibling-note fuzzy-matches),
  and the display-text convention. See `references/broken-link-repair.md`
  Pitfalls for the full algorithm.
- **aiml-wiki output verifier:** `scripts/verify_aiml_wiki_output.py` runs
  all post-write BRIEF-TEMPLATE checks in one call (byte range, frontmatter,
  11-section presence, ≥5 outbound wikilinks, PASS-line format and
  accuracy, `build_index.py --check` broken-link baseline). Use after every
  trim round in an aiml-wiki leaf run; the trim loop otherwise costs 4–5
  verification tool calls per round. Pass `--baseline-broken N` to enforce
  the no-regression rule (current broken must not exceed pre-write broken).
- **Multi-wave parent orchestration.** When the parent agent dispatches
  10+ leaf subagents across waves (e.g. *"deepen all 50 notes in the
  aiml wiki to 6–10k over a 4-hour wall-clock budget"*), the parent's
  playbook is different from the leaf's. Use `references/multi-wave-enrichment-burn.md`
  for the 5-component minimum-viable flow: durable scratch dir (NOT
  `/tmp/`), single-task dispatch (the parent-side batch wrapper
  crashes on multi-task dispatch), poll-and-apply between iterations,
  HTTP 429 re-dispatch (don't abort), closeout log entry. Session-
  evidenced 2026-07-28/29: shipping 41 subagents / 50 enriched notes
  in ~4.5 hours required every one of these steps.

## Growing the graph (the levers) — what to do AFTER notes exist
Once a vault has notes but they form a star (one hub, leaves only link *up*),
the value is in **density + lateral structure**, not more notes. Work the
levers in this order (each verified with the diagnostic script below):

0. **Diagnose first.** Run `wiki/build_index.py --check` (the script the
   vault itself ships with) — NOT `scripts/graph_stats.py` — for the
   *authoritative* broken/ambiguous/orphan counts. The two scripts disagree
   on path-style wikilinks: `graph_stats.py` resolves relative to each
   note's folder (incorrect for this vault's layout) and reported 406
   "broken" links where the actual count was 5. `build_index.py --check`
   reported the correct 5. Trust the latter; if you quote `graph_stats.py`
   numbers, lead with the caveat. Run `scripts/graph_stats.py <vault>` for
   the *in-degree leaderboard* and avg-out-links-per-note (it's better at
   those) — use it for "what's worth deepening", not for "what's broken".
   For broken-link repair, the recipe is in `references/broken-link-repair.md`:
   classify targets into AUTO_FIX / FUZZY_AUTO / CREATE_STUB / STRIP, slice
   the work, dispatch 3 subagents to write to `/tmp/bridge-slice-N/`, parent
   verifies and applies. Reserve that fan-out for ≥4 link pairs; below that,
   do it with `patch` directly.
   **Important interpretation note**: `graph_stats.py`'s `Orphans` count
   includes empty (0-byte) files and sub-50-byte stubs by design — any file
   with zero real outbound links and zero inbound *is* an orphan by the script
   even if it's just garbage. Always separately compute the "real orphans"
   subset (notes with substantive content, conventionally >200 bytes, that have
   no inbound and no outbound links) before planning fixes. Report both
   numbers; don't conflate "vault tidiness" with "real content missing from
   the graph". See `references/graph-growth-levers.md` → Pitfalls for the
   full reasoning.
0a. **Diagnose non-`.md` assets too.** `graph_stats.py` walks `.md` only — so
   PDFs, images, zips, and other binary assets are invisible to it. When
   the user mentions "stray PDFs" or asks about unindexed files, **complement the
   diagnostic** with a non-`.md` pass: walk every non-`.md` file, classify
   each as (a) `![[file.pdf]]` or `[[file]]` referenced from some `.md`, (b)
   enumerated in a folder index, or (c) truly unindexed. The PDF version of
   this is end-to-end covered by the `obsidian-paper-library` sibling skill —
   use its organizer pattern, but for images/zips just listing orphans by
   folder and proposing a single index note per folder is usually enough.
1. **Cross-link leaves + topical hubs.** Add 4–6 thematic hub notes (e.g.
   `[[LLMs and SLMs]]`, `[[Agents and Orchestration]]`, `[[Applied ML and CV]]`,
   `[[Web Tools and Learning]]`) each collecting related repos, plus a `Themes.md`
   MOC. Injected via a `- Themes:` line per repo note.
2. **Atomic concept notes.** Discover concepts by *scanning repo contents*
   (regex over note bodies), keep only cross-cutting ones (≥2 repos, prefer
   ≥4). Inject a `- Concepts:` line per repo. Concepts sit orthogonally to
   themes (a concept like `[[RoPE]]` spans 3 theme buckets) → real graph density.
   See `scripts/discover_concepts.py` (scaffold) + `references/graph-growth-levers.md`.
3. **Real `gh` enrichment (#4).** Backfill metadata — this is the existing
   "Safe enrichment pattern" section above.
4. **Bridge parallel systems (hand-written vault ↔ agent-maintained wiki).**
   When the vault has intentional duplicates — same concept living in two
   places (e.g. a hand-written `Concepts/RMSNorm.md` *and* an agent-maintained
   `wiki/aiml/concepts/rmsnorm.md`) — the user often wants navigation between
   the two systems, not merge. Some pairs are already partially bridged,
   some are not, and some are not bridgeable (one side is a 0-byte stub — see
   `CLAUDE.md`). Full playbook with the canonical 18-stem list, exact
   wikilink conventions, and the worked example is in
   `references/bridging-parallel-systems.md`. The trap to avoid: assuming
   the manifest's "ambiguous_links" count is a "broken" problem — those are
   *intentional* duplicates, not bugs. The 113 ambiguous links in Arctic's
   vault are all by design (per `wiki/AMBIGUOUS-LINKS.md`).
5. **Add a new domain — scaffold-then-fill.** When the user asks to add a
   whole new subsystem (e.g. *"set up a Physical Fitness hub with a BJ
   protocol reference and a daily tracker"*), do it as a single batch with
   every file scaffolded and `broken=0` verified, then populate content in
   later passes. The shape is:
   (a) **Umbrella MOC** at the parent (e.g. `Personal Growth/Physical Fitness.md`)
       that lists the subsystems and points at the reference folder and the
       tracker folder.
   (b) **Reference folder** (`bryan_routine/`) with a `README.md` index, a
       dated `Protocol.md` (high-level), a `Source log.md` for provenance,
       and `Concepts/<X>.md` files marked `status: scaffolding` and
       `counterpart:` frontmatter pointing at the user's parallel concept
       note if one exists.
   (c) **Tracker folder** (`Tracker/`) with a MOC file (so bare `[[Tracker]]`
       resolves), a `_template.md` copy-paste starter, a `dashboard.md` with
       Dataview queries, and one dated file per day.
   (d) **Adoption log** inside the reference folder — one row per thing the
       user actually tried, so the reference becomes a *menu* not a
       prescription.
   Then `python3 wiki/build_index.py --check` and confirm `broken=0`. Content
   gets filled in subsequent passes (often via subagents that *cite sources*
   — never fabricate). The first pass's job is navigability, not depth. Full
   recipe + worked example in `references/personal-tracker-pattern.md`.

5b. **Add a session-history-backed daily journal.** When the user asks
   for a day-by-day record of working with Hermes (*"do you store
   memory of each day?"*, *"update my knowledge graph with a daily
   memory section"*, *"add a journal section"*), the source of truth
   is the local SQLite session store at `~/.hermes/state.db` — already
   digital, not handwritten. Distinct from rule 5 (`personal-tracker`)
   which is for handwritten tracker subsystems (sleep, fitness). The
   canonical layout, scaffold order, subagent dispatch plan, and
   wikilink-fix discipline are in
   `references/daily-memory-subsystem.md`. **Minimum-viable shape:**
   ```
   Daily Memory/
   ├── Daily Memory.md           # MOC, index of all dated notes
   ├── _template.md              # schema (At a glance / What I worked on /
   │                             #   Decisions / Sessions table / People /
   │                             #   Projects touched / Concepts / See also)
   ├── scripts/
   │   └── daily_memory_helper.py  # argparse CLI over state.db
   └── YYYY/YYYY-MM-DD.md        # one per day the user used Hermes
   ```
   Dispatch one leaf subagent per day in waves of ≤
   `delegation.max_concurrent_children` (Arctic: 10, but UX-tested
   at 3 with no race issues). Brief the subagent to (a) run the helper
   for its date, (b) fill the schema, (c) use **bare basenames** in
   every wikilink (never `..`), (d) overwrite the stub, (e) verify
   `head -12` shows the frontmatter intact. Parent runs
   `build_index.py --check` after each wave, fixes any `..`-prefixed
   links the helper script can't catch, and only then declares the
   wave done. Session-evidenced 2026-08-03: 18 days × 6 waves of 3
   leaf subagents × ~60s each → all 18 dated files + MOC + _template
   + helper in ~12 minutes wall-clock, `broken=0`, helper-output
   session counts match file sessions-table row counts exactly. See
`references/daily-memory-subsystem.md` for the full worked example
(including the `..`-wikilink fix loop and the 5-check verifier).

5a. **Create a person entity for the vault owner (or a frequent
collaborator).** Triggered by *"add everything you know about me to
   the graph"* / *"set up a profile for <person>"* / *"create a hub for X"*
   / *"super-cluster my information"* (the last one signals
   extension from 2-note bridge to multi-facet cluster — see rule 5a-2
   below). Distinct from rule 4 (bridging existing duplicates) — here
   you're *creating* the pair from scratch. Session-evidenced
   2026-07-29 with the Aditya profile + the 8-facet super-cluster.
   The base two-note pair shape (`People/Me.md` +
   `wiki/personal/entities/aditya-sasidhar.md`) per the worked
   Aditya example:
   (a) **Survey first.** Check if `People/` already has a stub for the
       person (often present, often empty — see CLAUDE.md stub rule).
       If the user's parallel hand-vault hub already exists, treat it
       as the authoritative contact-info source; the agent-maintained
       view becomes the long-form structured profile.
   (b) **Hand-vault hub.** Mirror the existing `People/<Person>.md`
       convention (phone, birthday, group link, `starred: true` for
       self/close-family). Keep it short — quick reference, not
       biography. Point at the long-form agent view.
   (c) **Agent-maintained long-form.** Place at
       `wiki/<area>/entities/<slug>.md` matching whatever wiki domain
       fits — `wiki/personal/entities/` for self/family/friends,
       `wiki/work/entities/` for colleagues. Use the entity-schema
       frontmatter + `counterpart: People/<Person>.md` field. Body
       should carry the structured profile the agent reasons over:
       Identity, Work, Operational principles, People in their life,
       Vault structure, Personal stuff, Side projects, Current focus,
       Communication preferences, Observed quirks, Open questions
       (pronouns, birthday, etc., inferred cautiously from chat cues).
   (d) **Wire the MOCs.** Add the new node to `People/People.md`
       (the hub-and-spoke MOC) and to the relevant wiki's `index.md`.
       Both must link back to the bridge.
   (e) **Wikilink conventions apply** — bare basename for unambiguous
       targets (`[[small-language-model]]`), path-with-pipe-alias for
       ambiguous basenames (`[[wiki/aiml/index|aiml]]`), prose-with-
       absolute-path for files outside the vault (e.g. project root
       files like `/home/arctic/projects/night-oss-list.md`). The
       three-rule rule is in Golden Rule 5 sub-bullet "Critical
       corollary" and tested end-to-end here.
   (f) **Final gate.** `python3 wiki/build_index.py --check` shows
      `broken=0`; `ambiguous` may rise (intentional dual-system
      bridges). Log entry in the relevant `wiki/<area>/log.md`.

5a-2. **Extend a person entity to a multi-facet cluster.** Triggered by
   *"super-cluster my information"* / *"make it denser"* / *"I want a
   real graph, not just a hub"* / *"add everything you know about me"*
   after the base entity is already created. This is rule 5a's
   continuation — the two-note pair is necessary but not sufficient
   for a "real cluster"; the cluster is a *mesh*, not a star. Session-
   evidenced 2026-07-29 with the Aditya 8-facet cluster: started with
   `People/Me.md` + `wiki/personal/entities/aditya-sasidhar.md` (both
   with `incoming=0` — i.e. nothing in the vault pointed at the person),
   ended with 18 cluster notes, 192 outbound edges, 105 incoming edges,
   `broken=0`, and reciprocity from `Hobbies/Hobbies.md`,
   `Personal Growth/Recovery.md`, and `Goals/goals.md`.

   The rule:

   (g) **Diagnose the star-shape.** Before adding facets,
      `python3 -c "import json; ..."` to count incoming to the
      central hub. If `incoming=0`, the entity is stranded — facets
      alone won't fix it. Subsequent steps patch that.

      ```python
      import json
      with open('wiki/INDEX.json') as f:
          idx = json.load(f)
      entity = idx['notes'].get('wiki/personal/entities/<slug>.md', {})
      incoming = sum(
          1 for np, nd in idx['notes'].items()
          for ol in nd.get('links_out', [])
          if ol.get('resolved') == 'wiki/personal/entities/<slug>.md'
      )
      print(f'incoming={incoming}')  # if 0, this rule applies
      ```

   (h) **Pick 6–8 facets** by **dimension**, not by category
      overlap. The Aditya 8 were chosen on orthogonal axes:
      identity (biographical facts), projects (work focus),
      people (relationships), infrastructure (knowledge graph +
      tools), preferences (communication + working style), hobbies
      (creative pursuits), personal-growth (routines), open-questions
      (canonical unknowns). Aim for facets that have **lateral
      coupling**, not "everything-fan-into-one-big-hub"
      subtypes.

   (i) **Each facet = 2 notes** (hand-vault short + wiki long).
      Hand-vault: `People/Facets/<Name>.md`, 200-1500 bytes,
      snapshot-only, MOC-shaped (a `## Linked` section with
      `[[central hub]]` + adjacent facets). Wiki long-form:
      `wiki/<area>/facets/<name>.md`, 1-4 KB, structured body
      with `## Cluster position` and `## Related` sections,
      `counterpart: People/Facets/<Name>.md` frontmatter.

   (j) **Cross-link the cluster with both directions.** Each
      facet links to the central hub AND every adjacent facet.
      Central hub + entity each list all 8 facets. Result: the
      cluster becomes a mesh (not a star) — `incoming` goes from
      0 to ≥3 per note.

   (k) **Add a canonical Open questions facet (always include it).**
      Single source of truth for unknowns (pronouns, birthday,
      long-term goals). When something is resolved, *promote it
      out of this facet* (delete from open-list, capture in
      Identity & bio). The facet is the new home for items that
      would otherwise dangle in the central entity forever.

   (l) **Reciprocate from related root-MOCs.** Find the 3-5
      existing notes that *should* link to the cluster but
      don't: `Hobbies/Hobbies.md`, `Personal Growth/Recovery.md`,
      `Goals/goals.md` for the Aditya case. Add a `## See also`
      section pointing at the relevant facet. This is the
      single biggest `incoming` win — root MOCs have lots of
      traffic elsewhere in the vault, so a single reciprocation
      pull-multiple-following-citations along with it.

   (m) **Run `build_index.py --check` after EACH round**, not at
      the end. Cluster builds emit predictable broken-link
      patterns (wrong namespace prefix, ambiguous bare basename,
      folder-only paths — see rule 5 sub-bullet "Pre-write
      validation gate"). A short-cycle check after the first
      wave catches them in 5 seconds; an end-of-burn check
      means rebuilding the whole cluster if you missed any.

   (n) **Log entry.** Append one entry to the wiki `<area>/log.md`
      capturing: files created, files updated, incoming/outbound
      deltas before→after, and the recipe for the next facet
      addition (so future agents don't have to rediscover it).

   The full worked example for the Aditya cluster:
   `references/person-cluster-pattern.md`.

   **Important distinction from rule 5a**: rule 5a is the
   *minimum-viable* person profile (2 notes, bidirectional bridge).
   Rule 5a-2 is the *super-cluster* extension (16 notes for 8 facets).
   Apply rule 5a first; rule 5a-2 only when the user asks for
   density, a cluster, or "add everything." Don't speculatively
   build a cluster the user didn't ask for.

   **Idempotency / re-clustering**: if a facet is added later
   (e.g. user adds "Health" facet 6 months in), the recipe is:
   write the 2 new facet notes, add the central-hub link in
   every existing facet (touch N+1 files), add the new entry
   to `wiki/<area>/index.md`, log. The cluster grows outward
   cleanly without rebuilding.

**Idempotency + safety rules (learned the hard way):**
- All injections are idempotent: skip a note if its marker (`- Themes:` /
  `- Concepts:`) is already present. Re-running must not duplicate links.
- Link only to **resolvable** targets. After any bulk link pass, re-run
  `graph_stats.py` and confirm **NEW broken == []**. The only acceptable
  "broken" links are pre-existing ones you deliberately left alone.
- **`[[See also]]` round-trips across batches.** When writing several new
  concept pages that mutually reference each other in their `## See also`
  sections, batch the references and resolve all sibling cross-references
  BEFORE writing any of them — otherwise you get cascading dangling
  wikilinks that only show up in `graph_stats.py` *after* the batch is
  done. See `references/graph-growth-levers.md` pitfall "`[[See also]]`
  round-trips across a batch" for the working pattern.
- **`- Concepts:` injection must byte-preserve `Part of [[Group]]`.** The
  insertion point is immediately *before* the existing footer line, NOT at
  EOF. Appending at EOF silently breaks Dataview queries keyed on
  `endswith("Part of")`. See `references/graph-growth-levers.md` for the
  exact `rfind` algorithm.
- When a "See also" link points at a concept you skipped, **create the note**
  rather than dumbing down the link — a skipped catch-all (e.g. `Transformers`)
  is usually worth keeping.
- Don't collide concept names with existing repo notes: `Small Language Models`
  (concept) vs `Small-Language-Model` (repo) are deliberately separate.
- `CLAUDE.md` / `AGENTS.md` doc-example wikilinks (`[[wikilinks]]`,
  `[[Note Name]]`) are NOT real links — exclude them from broken-link counts.
- Wikilinks inside inline backtick code (`` `[[...]]` ``) are also
  syntax examples, not links. `graph_stats.py` strips backticks before
  counting; if you write your own verifier, replicate
  `re.sub(r"`+[^`\n]*`+", "", txt)` before the wikilink regex.
- **`graph_stats.py` and `wiki/INDEX.json` can disagree on what's broken.**
  See Golden Rule 0 (Diagnose first) for the resolution order — `build_index.py
  --check` is authoritative. `graph_stats.py`'s 699 "broken" in Arctic's vault
  was a false positive caused by folder-relative resolution. The two scripts
  *can* be complementary (use `graph_stats.py` for in-degree / out-degree
  histograms; use `build_index.py` for broken/ambiguous/orphan counts).
  **Bonus trap**: `build_index.py --check` reports broken-link line numbers that
  are often wrong (off by one or more) — the count is real, the line numbers
  are not. Re-derive the line by
  `open(p).read().count('\n', 0, m.start()) + 1` after locating the literal
  `[[target]]` in the source.
- **Stub files are sacred (CLAUDE.md).** 0-byte and sub-50-byte files are
  *deliberate placeholders* — see `CLAUDE.md` bullet 4. Don't try to
  "fix" them by filling them in unannounced, and don't bridge to them
  (an empty target produces a broken link). When the user asks to bridge
  a pair where one side is a stub, surface the rule and propose: fill
  the stub first, link to the non-stub side only, or skip entirely.
  Arctic's `Research/LEAP/leap.md` is the canonical example — bridging
  `Github/Repos/LEAP.md` ↔ `Research/LEAP/leap.md` is blocked until the
  research note is written.
- **Existing non-empty notes have an umbrella role — don't overwrite them
  with narrower content.** The inverse of the stub rule. If a file isn't
  empty, it has a *parent concept* or *MOC* role. Writing a narrower
  concept into its place destroys that role. Workflow: read the existing
  note first; if your new content is narrower than the existing scope,
  write a NEW file with the narrower scope and keep the existing note as
  the umbrella/parent. Caught 2026-07-26 transcribing a notebook page
  about sauna/cold/hair-wash: nearly overwrote `Personal Growth/Recovery.md`
  (5-line umbrella for {sleep, sauna, stretching, mobility}); recovered by
  writing the narrower content to a new `Sauna cold wash.md` and restoring
  `Recovery.md`'s umbrella role. The rule generalises to image-to-vault
  transcription, weekly-note enrichment, and bulk rewrites — anywhere
  you're replacing content, scope-check first. When the new content is
  *the same scope* as the existing note, replace is fine; when it's
  narrower, prefer a sibling note + parent kept.
- **Folder-only wikilinks like `[[Hobbies/Cooking/recipies]]` are silently
  broken even when the file exists.** `build_index.py`'s path-resolver
  (`wiki/build_index.py:223-226`) does `c.lower() == want` + `endswith("/" + want)`
  where `want = target + ".md"` — neither matches a file at a deeper
  segment count than the link's path segments. Symptom:
  `build_index.py --check` reports the link broken, but `find` shows
  both folder and `.md` file at exactly that path. Fix: rewrite to
  bare basename (`[[recipies]]`) or full path with filename
  (`[[Hobbies/Cooking/recipies/recipies]]`). Full mechanism + diagnostic
  in `references/broken-link-repair.md` Pitfalls → "Folder-path link
  targets". Caught 2026-07-26 fixing 5 dangling links in Arctic's vault
  (down from 5 broken → 0 broken, zero new broken introduced).
- **Don't subagent when scope ≤ 1 link.** A "bridge hand-written ↔ wiki"
  audit can collapse to 1 link pair after the duplicate-stem check. Don't
  dispatch 3 subagents for 1 patch — the per-subagent overhead (cold-start,
  brief context, transcript streaming) exceeds the edit cost. Reserve the
  fan-out pattern for ≥4 link pairs; below that, do it with `patch`
  directly. The user explicitly redirected away from a 3-subagent plan
  when this happened; surface the scope ("only 1 link pair to do")
  *before* announcing the subagent plan.
- **Subagent wikilinks that produce `Concepts/...` paths are bridge
  links that fail to resolve in the aiml wiki.** Session-evidenced
  2026-07-29 (small-language-model entity, wave 13): a subagent wrote
  `[[Concepts/RoPE (Rotary Position Embeddings)|RoPE]]` as the
  cross-system bridge to the hand-written vault concept hub. The
  resolver matched the bare basename `RoPE` against `wiki/aiml/...`
  only — and there is no `roPE` concept note at the top level. Result:
  `broken=1` mid-burn, parent had to `patch` the link back to the
  resolvable in-wiki note (`[[rope-positional-encoding|RoPE]]`). The
  defence: short-cycle a `build_index.py --check` after the FIRST
  wave's applies (not at end-of-burn), so cross-vault bridge drifts
  land in the parent's lap within minutes, not after 41 subagents have
  finished. Better: in the brief, list the wikilink targets as bare
  basenames that resolve in the aiml wiki, not `Concepts/RoPE` style
  paths — subagents faithfully copy whatever the brief says.
- **Verify negative claims about source state before accepting them.**
  Session-evidenced 2026-07-31 (knowledge-graph sync): a subagent
  reported "Cargo cannot load the rqlite workspace because ten declared
  core modules are missing" and "the C API's `engine.rs` is independent
  of `rqlite-core`". Both claims were true and caught a fabrication
  risk in the parent's own draft — the parent had shipped a note
  claiming all 21 submodules existed on disk and that the C API was
  a thin shim over `rqlite-core`. The defence is *not* "trust subagent
  self-reports more" (the parallel-orchestration rule already says
  verify subagent outputs) — it's the narrower rule: when a subagent
  makes a **negative claim about source state** (X module is missing,
  code doesn't compile, declared surface > actual surface), `grep` /
  `ls` / count the on-disk evidence yourself before propagating the
  claim. Positive claims ("the README says X") can wait for the
  end-of-wave check; negative claims about structural state are cheap
  to verify *now* and expensive to live with if wrong.
- **One writer per staging path; partition by target, not by subagent.**
  Session-evidenced 2026-07-31: dispatching multiple subagents to the
  same `/tmp/kg-sync-tmp/local-notes/<slug>.md` path produced
  `_warning: modified by sibling subagent` messages and a race whose
  winner was determined by mtime, not by correctness. The parent-
  applied versions won by accident (later writes), but the pattern is
  fragile. Either: (a) pre-allocate unique output paths per subagent
  (e.g. `/tmp/kg-sync-tmp/local-notes/v1-<subagent>-<slug>.md`) and
  merge at apply time, (b) run sequentially not concurrently for
  file-writing fan-outs, or (c) gate each `write_file` on a
  `read_file` immediately before to invalidate the write-through-cache
  warning. Option (a) is the cleanest.
- **Use `wiki/build_index.py --check` for vault audit, not a
  hand-rolled script.** Session-evidenced 2026-07-31: a hand-rolled
  audit script flagged 8 issues, of which 3 were false positives
  (`[[X]]` resolved via case-insensitive basename, `[[path/note|alias]]`
  resolved via the pipe alias, and backticked `[[wikilink]]` example
  syntax in prose were counted as real links). `build_index.py --check`
  already handles all three cases correctly. Read its output; do not
  re-implement it. When the hand-rolled script disagrees with
  `build_index.py`, `build_index.py` is correct — the audit script
  is wrong.
- **Don't silently overwrite a user-curated note's body description
  with a GitHub API one-liner.** Session-evidenced 2026-07-31: six
  repo notes (yoga-mail, candy-mail, projectscope, Well-Weather,
  Student-Performance-Ananlysis, Info-finder) had frontmatter
  descriptions that drifted from the GitHub API because the user (or a
  previous deep-enrichment pass) had written longer body descriptions.
  The vault body's description is the more valuable artefact; the
  GitHub one-liner is the inferior copy. Fix frontmatter fields that
  are objectively verifiable from the API (stars, forks, last_pushed,
  language); leave `description` alone unless the user asks
  explicitly. If the GitHub description was updated and the body
  hasn't been refreshed, surface the drift as a list and let the user
  decide.
- **Parent-orchestration traps specific to multi-wave enrichment
  burns.** Not in the leaf-subagent contract — these belong to the
  parent agent that dispatches 10+ leaf subagents in sequence:
  1. **Durable scratch dir, not `/tmp/`.** `/tmp` is a tmpfs on
     Arctic's setup; snap recovery wiped it mid-burn (2026-07-28
     23:26 IST), losing 9 finished wave files. Use
     `/home/arctic/projects/<burn-name>-tmp/waveN/` — durable disk.
  2. **Single-task dispatch, not batched.** Current Hermes's
     parent-side batch wrapper crashes with `"Delegation owner
     exited before recording a terminal result; outcome unknown"`
     on every multi-task dispatch — even when subagents completed
     cleanly. Workaround: dispatch 1 task per `delegate_task(goal=...)`
     call. Slower, but reliable (the wrapper handles single-task
     completion surfacing correctly).
  3. **Poll-and-apply, not end-of-wave apply.** Parent should `cp`
     each subagent's output file to the real target the moment it
     appears in the scratch dir. End-of-wave batch apply lost 9 files
     to the snap recovery before they could be moved.
  4. **Sync-pool fallback ≠ parallelism.** A `max_concurrent_children:
     10` config does not give 10-way parallelism when the pool is
     full — subagents fall into a synchronous queue. The aiml burn
     sustained ~6 min/subagent effective throughput (41 subagents in
     ~4.5 hours) regardless of advertised concurrency.
  5. **HTTP 429 → re-dispatch, don't abort.** When a subagent hits
     the rate limit mid-trim, its transcript ends with `exit_reason:
     max_iterations` and a missing/stale PASS line. Check whether
     the file on disk has usable content; if so apply it. If the
     rate-limit window hasn't cleared yet, hold further dispatches
     until it has, then re-dispatch the failed ones with no brief
     changes (they usually succeed in one pass).

## Recurring schedule facts (cross-vault + cross-wiki)

When the user states a **recurring weekly schedule block** ("internship is Mon–Sat 5–7 PM", "Wednesday = paper-reading evening"), persist it to BOTH the relevant **vault routine note** (e.g. `Personal Growth/Daily Schedule.md`, `Gym routine.md`, `Sauna cold wash.md`) AND the relevant **wiki entity note** (e.g. `wiki/work/entities/<x>.md`). Bump frontmatter `updated:`, update any time-block table, add a `## Fixed <X> schedule` section. Then `python3 wiki/build_index.py --check` and confirm `broken=0`. Full playbook + worked example: `references/recurring-schedule-persistence.md`.

When the user asks for **fun distributed throughout the day** (not bunched at the end), the reference has a reusable design pattern (Work split into A/B/C/D internal chunks + 2 micro-breaks + 1h Lunch+Fun; post-lunch dip → chunk C is admin; do not reshuffle A and C). When the user then asks to "add it to the knowledge graph" after a multi-turn schedule build, the reference also covers the **routine concept mirror** pattern: when to create one, the frontmatter (`type: concept`, `counterpart:`), the canonical 10-section body schema, the mandatory side-effects (update `index.md`, journal, facet, `log.md`, hand-vault back-link; verify `broken=0`), and the "don't subagent for one mirror" rule.

## Bulk reorganization (moving PDFs, folders, or merging MOCs)

When the user asks to consolidate folders, fix stray PDFs, or restructure a
research-papers canon, this is a different class of task than "enrich one note".
Follow the playbook in `references/bulk-reorganization.md`. Key points it covers
that are NOT obvious from the diagnostic alone:

- **Curly-quote Unicode in filenames** (`'` vs `'`) breaks hardcoded
  `shutil.move()` paths silently — read directory listings to build paths when
  moving ≥5 files from a folder you don't fully control.
- **Move files first, then write notes, then rewrite the MOC** — reverse order
  risks orphaned prose referencing files that never arrived.
- **Wiki/vault path relocation** (e.g. merging a separate wiki tree into the
  vault root) is a content-side migration, not just an `mv`: every hardcoded
  path in active scripts/refs/memories must be updated, but dated `log.md`
  entries should be left as historical record. Backup tar before moving, then
  classify-and-update each reference. Full playbook in
  `references/vault-path-relocation.md` (covers the three-class rule for
  in-file path references: active config / live docs / dated history).
- **Surface curation-policy conflicts before executing.** A `Papers.md` that
  declares *"is/isn't: this canon is Moonshot papers only"* may conflict with
  the user's "consolidate everything" request. Offer Path A / B / C and let
  the user pick.

Files moved to a new home: must verify disk state (asset exists at destination,
source tree is empty) before deleting source. Bulk notes written: every
destination needs its asset in place. MOC rewrite: last.

## Image-to-vault transcription (single handwritten / photographed note)

When the user attaches an image (handwritten notebook page, whiteboard, printed
schedule, screenshot of a list) and asks to add it to the knowledge graph, this
is a **single-source single-note** task — not a bulk enrichment. Different from
the GitHub-repo enrichment pattern, different from `ocr-and-documents` (which
is for technical PDFs). Recipe + the diagnostic dance + pitfalls:
`references/image-to-vault-transcription.md`.

This is a *single* note (the image IS the source — no GitHub API, no cloned
source tree). Don't drag in the bulk-enrichment machinery (subagents, temp
dirs, verify-before-apply fan-out) for one write. Do run the diagnostic
dance (read CLAUDE.md, survey target location, find the right MOC parent,
check for in-progress writes from another instance) before writing.

## Cross-vault wiki enrichment (wiki ← vault)

When the user has an llm-wiki destination and a personal-vault origin and asks
the agent to enrich the wiki from the vault ("mirror the research-side
material", "enrich the aiml wiki from `~/Documents/fun`"), this is its own
class of batch — not the per-repo enrichment loop above. The playbook is in
`references/cross-vault-wiki-enrichment.md`. Distinguishing points that trip
agents used to free-form ingest:

- The vault note is the **primary source**, not pre-edited wiki material.
  Treat it as a citation, not a draft.
- Hard rule: never invent facts. Every claim in a new wiki page must
  trace to a specific `raw/articles/*.md`, a vault paper note path, or a
  vault repo note path. If you can't cite a source, do not write the claim.
- Five steps run after orient, **validated after each step, not at the end**:
  pick 4-6 papers → write raw articles → write 3-5 concepts → cross-link
  entities → conditionally add comparisons.
- Wikilink-resolution budget is *zero new broken links*, not zero total.
  Track pre-existing broken-link baselines explicitly so a per-session check
  can confirm only zero *new* broken links were introduced. Always run
  `python3 wiki/build_index.py --check` before editing to capture the
  baseline counts (`broken`, `ambiguous`, `dup_stems`, `orphans`,
  `unknown_tags`, `deprecated_tags`) into a sidecar file
  (`/tmp/bridge-baseline.json` or equivalent) and again after the edit to
  diff against it. A rising broken count = introduced a dangling link =
  run `patch` to fix before declaring done.
- **Skip the comparison step if no justified concept pair exists.** A forced
  comparison without sources launders implied authority and is worse than
  no comparison.
- **Bridge-work is a separate task class.** When the user asks "connect
  things" / "wire the wiki into the vault" / "bridge the two systems" and
  NOT "enrich the wiki from the vault", the answer is *paragraph links*
  (one `[[wiki/...]]` and one `## Counterpart` back-link per duplicate-stem
  pair), not new wiki material. See `references/bridging-parallel-systems.md`
  for the playbook + the canonical 18-stem list.

## Lateral density beyond repos — papers and concepts

Lever 2 was originally written for `Github/Repos/` notes. The same playbook
applies (with adjustments) to *any* homogeneous set of related notes in the
vault — most commonly `Research/Papers/<X>/<X>.md`. When treating papers as
the unit:

- The ≥2-paper threshold is right; paper mentions are denser than repo
  mentions, so a clean 2-paper concept is worth keeping (don't gate at ≥4).
- Concept names may collide with paper names — e.g. `Small Language Models`
  (concept) vs `Small-Language-Model` (a repo AND a paper). Keep them
  separate explicitly; don't dedupe by name.
- Each paper note gets a `- Concepts: [[C1]], [[C2]]` line; each concept
  note gets a `## Papers using this` section paralleling
  `## Repos using this`.
- The `Concepts.md` MOC should report per-concept *repos* and *papers*
  inbound counts separately (`RoPE — 9 (7 repos, 2 papers)`) so the
  reader can tell the dominant axis at a glance.
- See `references/graph-growth-levers.md` → **Pitfalls** → the
  "Wikilink basename resolution with spaces" entry for the case where
  concept filenames contain spaces (e.g. `Mixture of Experts.md`) and
  wikilinks must use the exact capitalisation + spacing.

## Relevant skills
- `obsidian` (bundled) — base vault read/write/list conventions.
- `github-auth` — authenticating `gh` for repo access.
- `autonomous-ai-agents` — delegation mechanics.

## Reference index (load order)

When this skill fires, the references directory is the deep support.
Load in this order based on the user's request:

1. **`references/person-cluster-pattern.md`** — if the user asks for
   a *person cluster / hub / super-cluster* / "add everything you know
   about me" / "make it denser, not a star." Session-evidenced 2026-07-29
   (Aditya cluster went from 2 notes / 0 incoming to 18 notes / 105
   incoming in one session). Don't speculate — only fire when the user
   asks.
2. **`references/broken-link-repair.md`** — if `wiki/build_index.py --check`
   reports `broken > 0` and you want them gone. The classify-and-fix
   rubric + slice-and-dispatch subagent pattern lives here.
3. **`references/graph-sync-burn-playbook.md`** — the end-to-end recipe
   for "sync `/home/arctic/projects/` and the user's GitHub repos into
   the vault." Session-evidenced 2026-07-31 (52 GitHub repos, 10
   local projects, 30 concept stubs). Load when the user says "make
   sure we add all", "sync my projects", or any "bulk-add new repos"
   prompt. Includes the 5-bucket classification (github-already-noted,
   external-upstream-clone, vault-already-has-note, container-or-scratch,
   LOCAL-PROJECT), the alternate-working-tree rule, the
   `git_status --short` post-burn gate, and the anti-patterns
   (negative-claim verification, single-writer staging, audit-script-
   vs-build_index, don't-silently-overwrite-user-curated-bodies).
4. **`references/deep-enrichment.md`** — if the user wants the 11-section
   *deep* schema (1.5k → 10k+ chars) on GitHub-repo notes.
5. **`references/aiml-wiki-deep-enrichment.md`** — if the work is on
   `wiki/aiml/{concepts,entities,raw/articles,comparisons}/`. Distinct
   schema, distinct source convention, distinct output path. Do NOT
   apply the project-notes pattern or `/tmp/enrich-waveN/` here.
6. **`references/aiml-comparison-notes-deep-enrichment.md`** — narrow
   sub-pattern of #5: comparison-note schema + byte-budget traps.
7. **`references/multi-wave-enrichment-burn.md`** — parent-orchestration
   playbook (durable scratch, single-task dispatch, poll-and-apply,
   429 re-dispatch). Load BEFORE running a 10+-subagent batch.
8. **`references/graph-growth-levers.md`** — what to do AFTER notes
   exist (cross-link leaves, atomic concepts, hub notes).
9. **`references/bridging-parallel-systems.md`** — if the user asks to
   "connect things" / "wire wiki into vault" / "bridge the two
   systems" and NOT "enrich the wiki from the vault." Paragraph links,
   not new wiki material.
10. **`references/cross-vault-wiki-enrichment.md`** — if the user asks
    to enrich the wiki FROM the vault. Five-step validated-after-each
    pattern; zero-new-broken budget.
11. **`references/personal-tracker-pattern.md`** — if the user asks to
    set up a new tracker-heavy subsystem (e.g. Physical Fitness with a
    daily log + reference folder + Dataview dashboard).
12. **`references/recurring-schedule-persistence.md`** — if the user
    states a recurring weekly time block. Persist to both the relevant
    vault routine note AND the relevant wiki entity.
13. **`references/bulk-reorganization.md`** — file moves, folder
    consolidation, MOC merges.
14. **`references/vault-path-relocation.md`** — merging a separate wiki
    tree into the vault root (content-side migration, not just `mv`).
15. **`references/image-to-vault-transcription.md`** — transcribing a
    single handwritten / photographed note (NOT a bulk enrichment).
    Single note, no subagents, no temp dirs. Just run the diagnostic
    dance.
15a. **`references/competition-notes-pattern.md`** — when the user asks
    to scaffold a workspace for a list of competitions / hackathons /
    contests. Distinct from generic `rule 5` because the first pass is
    *only* `mkdir -p` per item (no auto-research, no MOC, no per-comp
    notes yet). Schema + lifecycle buckets + Dataview-friendly
    `status:` field are documented in the reference.

The references themselves are loaded on demand via
`skill_view(name=..., file_path=references/<x>.md)`. Don't auto-load
all 15 — load only the one(s) that match the request.