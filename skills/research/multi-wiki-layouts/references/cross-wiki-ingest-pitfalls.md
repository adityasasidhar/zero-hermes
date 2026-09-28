---
name: multi-wiki-layouts
reference: cross-wiki-ingest-pitfalls
---

# Cross-Wiki Ingestion — Pitfalls & Patterns

Concrete lessons from the `learning` wiki enrichment session (2026-07-19), where
two TILs were ingested from a source whose natural home was another vault
(`/home/arctic/Documents/fun/Learning/Rust/Rust.md` lives in the personal Obsidian
vault, not in any wiki folder).

These pitfalls compound: hitting more than one in a single enrich pass is common
because cross-wiki work pulls at every schema assumption at once.

## 1. The "move files into canonical subfolders?" Decision Rule

When a wiki has existing files at the root and the SCHEMA says subfolders are
the canonical home, the move is **not free**. The cost is every existing
`[[wikilink]]` to those files.

**Decision rule (apply in this exact order):**

1. `ls` the wiki root + each existing file. If files live at root but the
   schema names a subfolder (`til/`, `concepts/`), the move looks obvious.
2. Stop. Run `search_files` for `\[\[[^\]]+\]\]` across all `.md` in the wiki.
   Tabulate how many references would need rewriting.
3. **If any cross-references use bare name** (`[[til-foo]]`, not
   `[[til/til-foo]]`), the move is a no-op reorganization that breaks links
   for no schema benefit.
4. Only move if (a) the schema forbids root-level placement AND (b) every
   existing link already uses qualified paths OR you're willing to bulk-rewrite
   every link. Confirm the scope with the user before doing the latter.

**Session evidence (2026-07-19):** learning wiki had 6 TILs at root despite
SCHEMA naming a `concepts/` subfolder. SCHEMA didn't forbid root-level TILs,
every existing wikilink used bare names (13+ across the wiki, plus
cross-wiki `[aiml/...]` references), and the seed batch had chosen root.
Decision: leave at root, document in log.md. Result: zero broken links, one
log entry explaining the reasoning.

## 2. Cross-Wiki Source Citation — Two Distinct Patterns

When you ingest from a source that lives OUTSIDE the target wiki (different
folder, different vault, or different machine), there are two citation
patterns. Pick based on whether the source is also wiki-indexed.

| Pattern | When | Form |
|---|---|---|
| **Cross-wiki wikilink** | Source is itself a wiki page in another subwiki | `[[other-wiki/page-name]]` |
| **Absolute path** | Source is a raw file (e.g., user's `Documents/fun/` vault) | `sources:[/absolute/path/to/file.md]` in frontmatter, and inline citation pointing at the section |

**Cross-wiki convention this codebase uses:** `[[aiml/concept-name]]` (qualified
prefix). Existing TILs in `learning/` follow this for cross-references
into the `aiml/` wiki. New pages must do the same to be picked up by
Obsidian graph view and backlink panes.

**Raw-file convention:** `sources:` frontmatter takes an absolute path so the
provenance can be re-verified even if the obsidian vault reorganizes. Add the
section reference inline as well (e.g., "Rust tutorial §2, §8") — section
numbers are stable across file moves, line numbers are not.

## 3. Source-Backed vs Fabricated TILs — The Hard Rule

When a source has 4+ clearly articulable insights but you can only fully
back 2 of them from line-by-line reading, **don't fabricate the other 2 as
TILs.** Two ways out:

1. **Skip the unbacked TILs entirely.** Cite only the backed ones from the
   new pages.
2. **Flag them as candidates in `raw/articles/<source>-tracker.md`** without
   writing TIL files for them. New pages don't link to the candidates, only
   the tracker does. When a future session backs them, the candidate
   graduates to a real TIL.

The hard rule "NEVER invent facts" means: don't write a TIL whose body
claims "the source says X" when the source says something different or
nothing at all. If you can't quote or paraphrase a concrete claim with a
section anchor, you don't have a TIL — you have a topic idea, which goes
in `raw/articles/<source>-tracker.md`, not in `til-<topic>.md`.

**Tracker shape (matches SCHEMA.md `raw/` frontmatter):**

```yaml
---
source_url: file:///absolute/path/to/source.md
ingested: YYYY-MM-DD
sha256: pending
---

# <Source name> — raw ingestion tracker

> Condensed raw-ingestion note, not a finished synthesis.
> Full per-section insights live in the extracted TILs (see Related).

## Insights worth extracting
(Only the ones you've actually extracted. Mark each with the TIL slug + section.)

## Other concrete insights present but not yet extracted
(List candidates explicitly here, with "DO NOT cite these until extracted" caveats.
This is the staging area.)

## Related
(Only link to TILs that actually exist. No aspirational links.)
```

## 4. The `patch` Tool Swallows Section Boundary Lines

`patch` (mode='replace') does a literal find-and-replace. If your `old_string`
ends at a section heading boundary (`## [date] section-name`) and your
`new_string` doesn't include a corresponding new heading, the heading is lost
in the diff. The body line is reassigned to the PRECEDING section silently.

Hit this live on `log.md` in 2026-07-19:

```diff
-## [2026-07-18] ingest | Learning seed batch (TILs + concepts)
+## [2026-07-19] enrich | Learning cross-pollination
+Files created (3):
 ...files created block...
 Files created (8):
```

The `## [2026-07-18]` heading was lost; the "Files created (8)" block was
appended to the new section as if it belonged there. Had to rewrite the file
from scratch.

**Mitigation:** when adding a new section via `patch`, include the old
section heading in BOTH old_string and new_string (it disappears from the
former and reappears at the correct position in the latter). Or for
append-heavy log files, prefer `write_file` (full rewrite) over `patch`.

## 5. Total Pages Header Must Match Real Filesystem

`index.md` carries a `Total pages: N` counter. When enriching:

- Count actual `.md` files in `entities/`, `concepts/`, `comparisons/`,
  `queries/`, plus root-level TILs/concept pages — exclude `SCHEMA.md`,
  `index.md`, `log.md`, and anything in `raw/`.
- Subfolders (`comparisons/`, `concepts/`, `entities/`, `queries/`) may be
  empty by design. Don't count their absence toward the total.
- If the user supplies an initial count from the prior session, verify
  against the filesystem, don't trust it.

**Session evidence (2026-07-19):** index.md said `Total pages: 8`. After
creating 2 new TILs + 1 raw ingestion note, `Total pages: 10` was correct
(raw/ doesn't count). Did not trust the input number — verified with `ls`.

## 6. Concept Cross-Linking Is Bidirectional, Not Free

When two concept pages synthesize the same set of TILs, they both
**should** link to the TILs, AND they should link to each other. The
bidirectional link is what makes them feel like a coherent group instead
of three independent pages that happen to share a topic.

Check both directions explicitly:

- For each concept page, list the TILs it synthesizes.
- For each pair of concept pages, verify each has at least one
  `[[wikilink]]` to the other. (The reverse direction is what gets
  forgotten.)

The `learning` wiki had this gap: `concept-rmsnorm.md` did NOT link to
`concept-modern-llm-trifecta.md` even though the latter linked to it. Caught
during enrichment by reading both files; patched to add the missing
outbound link.

## 7. Two `[date]` Sections With the Same Date? Keep Both Headers

A wiki log is append-only and chronological. If you enrich on the same day
as a prior action, you add a NEW `## [YYYY-MM-DD]` section, you don't merge
into the existing one. Even though two entries on the same day reads
slightly redundant, it preserves the action grouping (each `## [...]` heading
documents one logical action with its own file list). Merging them loses
the action-provenance boundary.

**When in doubt:** add a new section. Worst case, the log has two headings
for the same day. Best case, the log cleanly separates the two distinct
actions that happened to land on the same date.
