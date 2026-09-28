# Graph growth levers (Obsidian vault)

Method + verification used to take adityasasidhar's vault from a 122-note star
(avg 1.5 out-links/note, one `Public Repos` hub) to a 145-note web (avg 5.0
out-links/note, 5 theme hubs + 18 concept hubs), with **zero new broken links**.

## The diagnosis that matters
A knowledge graph's payoff is **lateral linking + topical hubs**, not note
count. A star topology (every leaf links only *up* to its index) is the most
common failure. Detect it with `scripts/graph_stats.py <vault>`:
- `avg outbound links/note` ≈ 1.5 → star.
- in-degree leaderboard: one note (e.g. `Public Repos`) dominates; leaves are 0.

## Lever 1 — cross-link leaves + build topical hubs
1. Group repos into 4–6 themes by reading `Public Repos.md` / `Private Repos.md`
   descriptions. Hubs: `LLMs and SLMs`, `Agents and Orchestration`,
   `Applied ML and CV`, `Web Tools and Learning`.
2. For each repo, add `- Themes: [[Hub1]], [[Hub2]]` (after the `Part of:` line).
   Idempotent: skip if `- Themes:` exists.
3. Create each hub note (`Themes/<Name>.md`): `type: Theme Hub` frontmatter,
   `> Cross-cutting hub · [[Themes]]`, a blurb, `## Repos` list, `## See also`
   (the other hubs + `[[Research]]` / `[[learnings]]`).
4. Create `Themes/Themes.md` MOC and link `[[Themes]]` into `Me.md` (root MOC).

## Lever 2 — atomic concept notes (the dense web)
1. **Discover from content, not filenames.** Regex-scan every repo note body
   (lowercased) for technique keywords. Example patterns:
   `rope|rotary`, `swiglu`, `rmsnorm|rms norm`, `babylm`, `mcp|model context protocol`,
   `dspy`, `ollama`, `recursi|weight-?tied`, `reinforcement|\brl\b`.
2. Keep only concepts appearing in **≥2 repos** (prefer ≥4 — those form real
   hubs). `Transformers` is a catch-all; keep it but it's weak signal.
3. Skip concepts that duplicate a repo note name (e.g. `Small Language Models`
   concept is distinct from the `Small-Language-Model` repo note).
4. For each concept, create `Concepts/<Name>.md`: `type: Concept` frontmatter,
   blurb, `## Repos using this` (every member), `## See also` (related concepts
   + theme hubs). Add `- Concepts: [[C1]], [[C2]]` to each member repo note
   (idempotent; append to existing `- Concepts:` line if present).
5. Create `Concepts/Concepts.md` MOC, link `[[Concepts]]` into `Me.md`.

## Verification (run after every lever)
```
python scripts/graph_stats.py /path/to/vault
```
Assert: `NEW broken (excluding pre-existing): []`, and `avg out-links/note`
rose. Pre-existing dangling links you deliberately left alone (e.g. doc-example
`[[wikilinks]]` in CLAUDE.md) must be excluded from the broken count.

## Pitfalls encountered (and fixed)
- **False-positive broken-link detector**: building the name-set by walking the
  tree and keying on `splitext(f)[0]` is fine, but a verifier with a buggy dedup
  flagged `adityasasidhar.github.io` as broken even though the note existed.
  Fix: exact `os.path.splitext(basename)[0]` match; the link resolved. Use
  `graph_stats.py` which does this correctly.
- **Wikilinks inside backticks get flagged as broken links.** A line like
  `...export to PDF removes the `[[...]]` brackets automatically...` is prose
  describing wikilink syntax, not an actual link. `graph_stats.py` now
  strips inline-code regions before counting
  (`re.sub(r"`+[^`\n]*`+", "", txt)` before the wikilink regex). If you write
  your own verifier, replicate that strip — otherwise every footer with a
  syntax example surfaces as `NEW broken: 1`.
- **Created a dangling link by referencing an uncreated concept** (`Neural
  Networks`, `Transformers`, `From-Scratch ML` in `See also`). Fix: either
  create the concept note or don't link it. Creating is better for graph density.
- **`if False` dead branches** in injection glue — keep the live path simple and
  test the tail of one note after the run (e.g. `read_file` last 260 chars).
- **Script's "Orphans" count conflates "real orphans" with empty/stub files.**
  `graph_stats.py` defines an orphan as "no real outbound AND no inbound" —
  true for any 0-byte file or sub-50-byte stub. In one session the script
  reported `Orphans: 10` but only 5 had non-trivial content; the other 5 were
  empty files / 8-byte paste-buffer fragments. Mitigation: compute and report
  two numbers — `Orphans (script's definition)` and `Real orphans
  (content ≥200 bytes, no inbound, no outbound)`. Empty/stub files usually
  want deletion, not orphan-link wiring. The SKILL.md step 0 now spells this
  out so the diagnostic step doesn't load this pitfall only to rediscover it.
- **Root-folder filename collisions with the vault dirname are operationally
  orphaned.** A note named `<vault-name>.md` sitting at the root (e.g.
  `fun.md` inside vault `/Documents/fun`) is invisible to Obsidian's index,
  receives no navigational links, and won't appear in any MOC — even when
  other notes can technically wikilink to it. Mitigation: during diagnosis
  check `basename == vault_dirname` for every root-level `.md`. If it
  matches, the note is **misplaced, not orphaned** — recommend moving it to
  a topical folder and adding it to the relevant MOC, not creating
  inbound wikilinks from random places.
- **Non-`.md` assets (PDFs, images, zips) are invisible to the diagnostic and
  to wikilink detection.** `graph_stats.py` walks `.md` only — so unindexed
  PDFs won't surface in `Orphans` or `NEW broken`. In one session 43 PDFs
  lived in the vault with zero `![[...pdf]]` embeds; only 12 of them sat next
  to a per-folder `.md` and were effectively indexed. The other 23 were
  paper drops in `Work/Papers/papers/<subdir>/` with no inbound. Treatment:
  this is a separate skill-class (`obsidian-paper-library`) — always run
  that org/foldering pass for PDFs, and for any non-PDF non-`.md` assets just
  propose a folder index if it's >2 files in the same dir.
- **Curation policies live inside the MOC, not just in `CLAUDE.md`.** When
  a folder like `Research/Papers/` has a MOC file with an explicit *is/isn't*
  block (e.g. *"is: Moonshot AI papers only — not Anthropic / OpenAI / VAE"*)
  that's the active curation policy. Read the MOC body before asking the user
  *what does this folder contain*; the answer is in the document. Before any
  bulk move into a curated folder, surface the existing policy and offer
  Path A (re-purpose as general folder), Path B (sibling folder for
  non-conforming items), Path C (new top-level sibling) — never silently
  violate the policy.
- **Filenames with curly Unicode quotes (U+2019 `’` vs ASCII `'`) silently
  break `shutil.move()` paths.** When moving ≥5 files from a folder you don't
  fully control, don't hand-type the source paths from an earlier `ls` output —
  relist the directory at move time and build paths from the actual bytes.
  In one session 3 of 23 PDF moves failed with "missing source" because the
  source paths were constructed from strings with ASCII apostrophes while
  the on-disk filenames had curly ones. Fix: build the source path from
  `os.path.join(vault, f)` where `f` comes from a fresh `os.listdir(...)`.
- **Curly-quote redirects on renames.** When a wikilink target uses ASCII
  apostrophes but the file is named with curly ones (e.g. `Don’t Always…
  .md`), Obsidian's wikilink resolution handles both — but the diagnostic
  script's `os.path.splitext(basename)[0]` lookup uses the literal bytes on
  disk. Don't rely on ASCII/Curly normalization; verify the exact byte
  sequence at write time.
- **Augmenting an existing concept note with a parallel section.** When
  adding `## Papers using this` to an existing concept note that already has
  `## Repos using this`, keep the two sections structurally parallel
  (same heading style, same bullet style) so the note reads as one concept
  with two facets rather than two halves glued together. Update the MOC's
  counts separately for *repos* and *papers* so a future session can
  quickly tell which axis the concept is dominant on.
- **Wikilink basename resolution with spaces.** When a paper note links to a
  concept whose filename is *`<Concept>.md` with a space* (e.g. `Mixture of
  Experts.md`), Obsidian wikilink resolution uses the exact basename — so
  `[[Mixture of Experts]]` resolves and `[[mixture-of-experts]]` does not.
  Don't bash-slugify concept names in `See also` sections without first
  checking the actual `Concepts/<X>.md` filenames; mismatched casing or
  space-vs-dash produces false-dangling-link errors.
- **Atoms vs hubs for paper-side concepts.** Repo-side concept notes
  (`RoPE`, `RMSNorm`, `SwiGLU`, `Ollama`) recur across many codebases.
  Paper-side concept notes recur across fewer notes but each one is
  *high-signal* — e.g. `Long Context` is in 12 papers, `Chain-of-Thought`
  in 8, `Reasoning Models` in 8. Use the same ≥2-paper threshold as the
  repo-side ≥2-repo threshold but tolerate lower counts because each paper
  mention is denser than each repo mention. Prefer ≥4 papers for a new
  concept note, but don't reject a clean 2-paper concept if both papers
  draw on each other.
- **`[[See also]]` round-trips across a batch.** When writing a batch of new
  concept pages that mutually reference each other in their `## See also`
  sections, batch the references and resolve all sibling references BEFORE
  writing any of them. Otherwise you get cascading dangling wikilinks that
  only show up in `graph_stats.py` *after* the batch is done. Pattern:
  maintain `pending_concepts = set()` of planned names and
  `pending_targets_for[name] = set()` of referenced names; only call
  `write_file(path, body_with_see_also)` after every name in
  `pending_targets_for[name]` is known to either (a) already exist on disk,
  (b) be in `pending_concepts` and scheduled for this same batch, or
  (c) be deliberately dropped (no wikilink syntax). Expect `NEW broken: 0`
  on the first `graph_stats.py` run.
- **`- Concepts:` line injection placement.** When adding a
  `- Concepts: [[X]], [[Y]]` line via an in-script patch, the insertion
  point is *immediately before* the existing `Part of [[Group]]` footer
  line, NOT at the end of the file. Algorithm: `idx = body.rfind("Part of
  [[Group]]")`, `start_of_line = body.rfind("\n", 0, idx) + 1`,
  `body = body[:start_of_line] + concepts_line + "\n" + body[start_of_line:]`.
  Spot-read the last 15 lines of one modified note before declaring the
  batch done — the `Part of` footer must remain the literal last content
  line. Appending at EOF silently breaks Dataview queries keyed on
  `endswith("Part of")`.
- **Transposed-syllable typos in wikilink target names.** Writing wikilink
  targets by intuition in a long batch produces consistent typos: swapping
  `attention` ↔ `alignment` (alignment faking / induction heads are the
  common offenders), `induction` ↔ `instruction`, `research` ↔
  `reasearch`. Pre-verify every name by `os.listdir`'ing the destination
  folder and building the exact basename from the file list, then writing
  the wikilink. The visible failure pattern is: write a long batch, see
  `graph_stats.py` report N broken, fix them, see N+1 broken because the
  fixes also had typos. Pre-verification eliminates the loop.
