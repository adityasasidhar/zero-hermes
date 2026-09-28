# Bulk vault reorganization (PDFs, folders, MOCs)

This reference is the playbook for moves that aren't "enrich one note" but "restructure
N notes / folders / PDFs in one session". Use it when the user says things like
*"all my research papers should be in one folder"*, *"stray PDFs"*, *"this folder is
a junk drawer"*, or *"consolidate"*.

## Pre-flight: read CLAUDE.md / AGENTS.md FIRST

Open the vault root and read `CLAUDE.md` (or `AGENTS.md`) before touching anything.
Many personal vaults declare out-of-band policies there that override what the
diagnostic scripts suggest. The two we hit in real sessions:

- **Empty stub files are deliberate placeholders.** A 0-byte `Books/books.md` may
  not be "a stub note to fill in" — it may be a *reservation* for a topic the
  user knows they want and will write later. The diagnostic script counts these
  as orphans/stubs; you need human-overridable policy to know whether to act.
- **Folder conventions are not always uniform.** A vault may have one folder that
  follows `<Topic>/<Topic>.md` (Kimi-canons use this) and one that doesn't
  (`Books/books.md` is the only MD in `Books/`). Don't extrapolate from one folder
  to all the others.

If `CLAUDE.md` exists, **quote it back to yourself** before proposing fixes for
whatever category it talks about (empty files, in particular). Re-reading it
mid-session is cheap; regretting an execution is expensive.

## The two-blind-spots problem in `graph_stats.py`

`scripts/graph_stats.py` is `.md`-only and *keys on basename only*. So:

- It can't tell you about PDFs/images/zips. Run a parallel non-`.md` walk
  separately (see `obsidian-paper-library` for the canonical version).
- It can't tell you about folder-level conventions. A folder with one
  well-indexed `paper.pdf` looks the same as a folder where the PDF is just
  sitting there with no adjacent note; the "Is this covered?" question is
  contextual, not graph-shaped.

Before any move: also read the *folder's MOC* (if any) by hand. The user's
stated curation rules often live there, not in CLAUDE.md.

## Curation-policy conflicts (the "consolidate vs. is/isn't gate" problem)

When the user requests "consolidate everything into one folder", the existing
MOC may carry an explicit is/isn't policy. Example from a real session: a
`Papers.md` that opened with *"Is: Moonshot papers. Isn't: papers that name Kimi
but aren't from Moonshot. External follow-up analyses."*

Surface this conflict to the user **before** moving 23 files. Don't silently
override the curation rule. The three paths to offer:

- **A.** Re-purpose the target folder as the general "all my research papers"
  folder. Move everything in. Rewrite the MOC intro to remove the gate.
  Honors the user's literal request. Violates the existing convention.
- **B.** Keep the curated folder, create a sibling for the rest. Honors the
  convention. Two "papers" locations.
- **C.** Keep curated, create a top-level `Papers/` (or `<Domain>/Papers/`)
  outside the curated folder for the rest. Honors the convention. Two folders
  with "Papers" in the name.

Default is to ask, not guess. The cost of being wrong is 23 PDFs in the wrong
home and an MOC rewrite that needs another rewrite.

## File moves: the right order of operations

For an N-asset move where each destination gets a new `.md` note:

1. **Pre-flight.** Read 2–3 existing notes at the destination to match
   template/format. Use `od -c` or `ls | cat -A` if filenames look weird
   (apostrophes especially).
2. **Move every asset first.** Pure file moves, no `.md` writes. If a move
   fails, halt and report — don't write notes that reference files that
   didn't arrive.
3. **Verify disk state.** For each destination, confirm `paper.pdf` exists
   with nonzero size. Sanity-check that the source tree is empty (or only
   has the trash you explicitly deleted).
4. **Write notes.** Now write the matching `.md` notes — every destination
   has its asset in place.
5. **Update MOC.** Rewrite the index MOC last, after notes exist.
6. **Delete source tree** (if applicable), only after step 3 confirmed empty.
7. **Re-run the diagnostic.** Confirm `NEW broken == []`.

Reverse order (write notes first, move later) risks orphaned prose referencing
files that never arrived.

## Filename character pitfalls

Hardcoded Unicode filenames in Python `shutil.move(src, dst)` calls fail
silently when the literal in your code uses a different glyph from what's on
disk. The failure is `FileNotFoundError` — quiet enough to mistake for "the
file was renamed".

Affected characters in real sessions:

- **Curly apostrophe / right single quote (U+2019, `'`)** — most common.
  Visually identical to ASCII `'` in editors but bytewise different. The
  browser and the clipboard both love to upgrade ASCII `'` to `'` silently.
- **Em dash (U+2014, `—`)** — less common but same problem.
- **Smart double quotes (U+201C / U+201D)** — same.

Two options:

- **A. Read directory listing to build paths.** `os.listdir(dir)` returns
  whatever glyphs the filesystem actually has, bytewise correct by
  construction. Use `endswith()` instead of equality when matching.
- **B. Quote-and-check.** Hardcode the literal, wrap the move in a
  `try/except FileNotFoundError`, and on failure re-list the source
  directory and grep for the unprintable characters.

Option A is more robust. Use it whenever you're moving ≥5 files from a
folder whose contents you don't fully control.

Wikilinks: `graph_stats.py` does **case-sensitive basename matching**
(`os.path.splitext(basename)[0]`). This means `[[Alignment-Faking-in-LLMs]]`
looks fine in the rendered Obsidian graph but **breaks** the diagnostic's
broken-link check if the on-disk folder is `Alignment-Faking-in-LLMs/` but
the `.md` is `alignment-faking-in-llms.md`. When writing link text in a
new note that references an existing `Foo-Bar/` folder, link it as
`[[foo-bar]]` (matching the `.md` basename), not `[[Foo-Bar]]` (matching
the folder name). The note body can use the capitalized phrase in prose
freely — only the `[[...]]` is the binding contract.

**Inline-code false-positive trap (the `Rust.md`-class edge case).**
`graph_stats.py`'s wikilink regex doesn't know about markdown code spans.
A line like `*Wikilinks use Obsidian syntax; export to PDF removes the
\`[[...]]\` brackets.*` trips the broken-link check because the literal
`[[...]]` matches the regex. Real recurrence: any study note's footer
line that explains wikilink syntax to the reader hits this. When
debugging a `[[...]]`-or-similar short-token broken link, **check the
source line for backticks first** — if it's inside a code span, it's a
false positive, not a typo. Diagnostic fix (one-liner, apply inside the
walk loop in `collect()`):

```python
txt_no_code = re.sub(r"```[\s\S]*?```", "", txt)
txt_no_code = re.sub(r"`+[^`\n]+?`+", "", txt_no_code)
# apply wikilink regex to txt_no_code instead of txt
```

The `DOC_EXAMPLE_LINKS` exclusion handles `CLAUDE.md` / `AGENTS.md`
samples (`[[wikilinks]]`, `[[Note Name]]`) but does *not* handle
backtick-wrapped prose anywhere in the vault. Patch the script or treat
backtick-broken as known cosmetic noise (filter with `--known`).

**"Likely typo; check" — the unnoted real project.** When a broken link
turns out to point at a real-but-unnoted project (e.g. `[[Paper Clip]]`
resolving to a GitHub repo that just doesn't have a vault note), action
depends on what's available:

- *You have content for it* (`repos.md` mentions the repo, README pulls
  cleanly, etc.) → write a proper project note via the existing per-repo
  template (YAML frontmatter + overview + inside the codebase + stats +
  links). The standard repos.MOC pattern picks it up automatically.
- *You have no content* (zero references anywhere except the broken
  wikilink itself) → **remove the dangling link from the source list**
  rather than create a fabricated stub. Leave the numbered slot empty
  (e.g. `2.` with no body) so the user can re-add a real entry later.
  Creating a note with no source material violates golden rule #1.

Default to removing the link. Ask only when the user has expressed a
strong "must keep this list" preference (rare). Cost of an empty
numbered slot: zero. Cost of a fabricated stub: harder to undo.

## What to verify before saying "done"

Run `python3 scripts/graph_stats.py <vault>` after the move + notes + MOC
rewrite. The contract:

- `NEW broken == []` (or only pre-existing ones you deliberately skipped).
- `avg out-links/note` should *not* drop. If it does, you broke links you
  shouldn't have.
- The MOC's in-degree should rise proportionally to N new notes that link to it.
- All new notes resolve when you click the wikilink in Obsidian.

If a newly introduced broken link appears, fix it immediately and re-run.
Do not report "done" with `NEW broken > 0`.

## Example session arithmetic

Moving 23 PDFs from `Work/Papers/papers/<org>/*.pdf` → `Research/Papers/<Slug>/paper.pdf`
plus 23 new notes + 1 MOC rewrite:

- Expected diagnostic delta: `avg out-links/note` rises by ~0.02 (5.14 from 4.91
  in the actual session). MOC in-degree roughly triples.
- Time cost: ~10–15 minutes for the whole pipeline (including the 2-minute
  Unicode apostrophe debugging step, which is unfortunately a known cost).
- Risk surface: 6 distinct hazards (CLAUDE.md override, curation
  conflict, Unicode filenames, case-sensitive wikilinks, inline-code
  false positives in the diagnostic, missing-note-vs-fabricated-stub
  decision for unnoted real projects). Each one bites on the first run.
  None are environment-dependent; all recur.
