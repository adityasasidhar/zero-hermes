# Image-to-Vault Transcription

When the user sends a photo (handwritten notebook, whiteboard, printed schedule,
screenshot of a list) and asks to add it to the knowledge graph, this is a
**single-source single-note** task — not a bulk enrichment. Different from the
GitHub-repo enrichment pattern, different from `ocr-and-documents` (which is
for technical PDFs).

Came out of Arctic's session 2026-07-26: handwritten 24-hour daily schedule
photo → new `Personal Growth/Daily Schedule.md` with cross-links to existing
stubs.

## When to use

- User attaches an image and asks to "add it to the knowledge graph" / "put
  this in the vault" / "save this to my notes".
- The image contains a **list, schedule, protocol, plan, or reference card**
  the user wants preserved as a single concept note.
- Not for: bulk paper OCR (use `ocr-and-documents` + `obsidian-paper-library`),
  not for mass enrichment (use the bulk-recipes in this skill).

## Before writing — the diagnostic dance

In this order, in parallel where possible:

1. **Read `CLAUDE.md` / `AGENTS.md` at the vault root.** Vaults often declare
   sacred stubs (*"empty files are deliberate placeholders, don't fill them"*).
   The image may point at content you've been told not to invent.

2. **Survey target location.** Search for likely filenames that already exist
   — `Daily Schedule`, `Routine`, `Time Allocation`, etc. Two parallel instances
   writing the same note is the most common duplicate. Pick the canonical name
   *first* (use the user's wording if possible).

3. **Find the right MOC parent.** List the top-level MOCs and their
   sub-headings. The image's content type maps to a parent:
   - Schedule / routine / time-budget → `Personal Growth` (umbrella) →
     `Physical Fitness` (subsystem) → new note
   - Reading list / paper notes → `Research` → `Papers`
   - Project plan / roadmap → `Projects`
   - Tech stack / architecture → `Github` or `Projects`
   - Reading tracker / book log → `Books`

   Resist the temptation to add the new note to the top-level MOC directly
   unless no subsystem MOC exists. MOC depth is real graph value.

4. **Check for in-progress writes from another instance.** If the user mentions
   "another instance is working on this too", run:
   `ls /tmp/ | grep -iE 'lock|coord|enrich|hermes'`
   and check modification times on the candidate parent MOC. If something looks
   like it just got written, surface the overlap risk before proceeding.

## The transcription

1. **Read the image with `vision_analyze`.** Don't guess from the filename.
   What you perceive in the image is the source of truth — note uncertain
   words/phrases explicitly so the user can confirm.

2. **Write a single concept note.** Not a MOC, not a project note. The schema:
   ```
   ---
   type: Concept       # or Plan, Protocol, Reference — pick the closest fit
   tags:
     - <parent-domain>
     - <subsystem>
   created: YYYY-MM-DD
   updated: YYYY-MM-DD
   source: image       # distinguishes from text-sourced notes
   ---

   # <Title from the image, verbatim if possible>

   <body — a faithful transcription of the image, restructured into prose
   + tables so the reader can scan it. Don't pad.>

   ## See also
   - <parent MOC> — the umbrella this belongs under
   - <existing stub 1> — what this block owns
   - <existing stub 2> — what this block owns
   ...

   ## Source
   - YYYY-MM-DD — handwritten notebook entry, photographed. <one-line provenance>.
   ```

   A few things to keep in mind:
   - The image is the source. Don't invent numbers that aren't in the image.
   - If the image totals don't add up (e.g. 24h schedule that tallies to 23h
     or 25h), note that in the body. User may have made an arithmetic error.
   - Document the **shape** of the image (table? list? mind-map?) — it tells
     the reader how to think about the note.

3. **Wire into the parent MOC.** Add a one-line entry under a new section
   (`## Time allocation`, `## Plans`, etc.) — don't drop it into a generic
   "subsystems" list. The new section header signals "this is a new axis".

4. **Don't touch the existing stubs the new note links to.** CLAUDE.md says
   they're sacred. The new note links *to* them; the user fills them in
   later.

## What's different from the existing image-to-text skills

- `ocr-and-documents` is for **technical PDFs / scannable documents** — it
  expects text images, not handwritten scheduler pages. Wrong tool.
- `obsidian-paper-library` is for **research papers** — expects a PDF and a
  slug-matched folder. Wrong shape.
- The bulk-enrichment recipe in this skill expects **GitHub repo data + a
  cloned source tree**. The image-to-vault class has neither.

The closest in this skill is **Lever 5 (Add a new domain — scaffold-then-fill)**
but the difference is material: Lever 5 is for *subsystems with multiple
files* (umbrella MOC + reference folder + tracker + concept files). The
image-to-vault class is *one image, one note, one parent MOC link*. Reserve
Lever 5 for "set up a whole new domain" requests; use this recipe for
"transcribe this one thing".

## Pitfalls (learned 2026-07-26)

- **Don't auto-rotate blind.** Image may be rotated 90° CW or CCW; check
  the snapshot before guessing. If words are still unclear, say so rather
  than fabricating. The user can reattach a clearer version.
- **The image is one source. Don't drag in "other context" the user didn't
  ask for.** A handwritten schedule stays a transcription. If you notice
  it conflicts with an existing tracker log, *surface that as a question*,
  don't silently overwrite.
- **The "ignore the previous image" mid-task cue.** If the user sends two
  images and then says "ignore the one I sent now", they're retracting the
  second. Don't back-integrate retracted content into earlier work. Drop it
  cleanly.
- **The third image that's "for another instance".** If the user says
  something like *"another Hermes is working on this too"*, treat both
  instances as collaborators, not competitors. Surface the overlap risk
  explicitly so the user can reconcile output downstream.
- **Don't parallelise a single-note write.** This is one write. The
  verify-before-apply + subagent dance from the bulk recipe is overhead —
  write directly with `write_file`, then verify with `read_file`.
- **The "Where does this go?" question is the actual work.** The transcription
  is mechanical. Choosing the right MOC parent is the judgment call. Spend
  the time on the diagnostic, not the prose.
- **Multiple images across multiple turns = multiple notes, but they're
  related.** When the user sends image 1 in turn N, transcribes it, then
  sends image 2 in turn N+1 (same notebook page or related protocol), DO
  NOT assume the second image is a continuation of the first. Treat each
  as its own diagnostic. The notes will often cross-link (e.g. a daily
  schedule links to a gym routine block; a gym routine links to a sauna
  block). Pass each image through the diagnostic dance; let the cross-
  links emerge from the writes, not from a prior assumption. Detected
  2026-07-26: two images of the same notebook page arrived across two
  turns (24h schedule + gym/sauna rotation). Each got its own note in
  its own MOC slot, with cross-links via the `See also` sections.
- **Existing note has an umbrella role — don't overwrite it.** The
  inverse of the stub rule. If a file isn't empty, it has a *parent
  concept* or *MOC* role. Writing a narrower concept into its place
  destroys that role. Workflow: read the existing note first; if your
  new content is narrower than the existing scope, write a NEW file
  with the narrower scope and keep the existing note as the umbrella /
  parent. Caught 2026-07-26 transcribing the gym/sauna page: nearly
  overwrote `Personal Growth/Recovery.md` (5-line umbrella for
  {sleep, sauna, stretching, mobility}); recovered by writing the
  narrower content to a new `Sauna cold wash.md` and restoring
  `Recovery.md`'s umbrella role. See the SKILL.md "Existing non-empty
  notes have an umbrella role" pitfall for the general rule.
- **After vault, the wiki-handoff is the second step.** When the user
  says "add it to my knowledge graph / my own routine" after a vault
  transcription, they usually want the **wiki journal stub** updated,
  not a new wiki page. Most wiki journals in `wiki/personal/` (e.g.
  `personal-growth-journal.md`) follow the rule *"vault is the source
  of truth; wiki keeps only a state summary + cross-links"* — so:
  (a) **don't** copy the routine content into the wiki; (b) **do**
  update the journal stub's `updated` date + tags + `sources` +
  add a `## Current state` section listing the new filled vault notes
  with full-path wiki links (`[[Personal Growth/Daily Schedule|Daily
  Schedule]]`); (c) **do** append an entry to `wiki/log.md` per the
  wiki's SCHEMA.md. Verify with `python3 wiki/build_index.py --check`
  that zero new broken links were introduced. The diagnostic: read
  `wiki/personal/index.md` and the affected journal's frontmatter — if
  the wiki already has a 'journal' page covering the domain, update,
  don't create. Caught 2026-07-26: user said "add it to my knowledge
  graph to my own routine" — the right answer was to update the
  existing `wiki/personal/personal-growth-journal.md` state, not
  create a new `daily-routine` page.

## Verification

After the write:

1. `read_file` the new note — confirm it looks right.
2. `read_file` the parent MOC — confirm the new link is there.
3. `cat` the file count of the new note vs. estimated size from the image.
   Order-of-magnitude check; a 1-page image typically yields a 1-3 KB note.
4. If the user's CLAUDE.md mandates a `wiki/build_index.py --check` pass,
   run it and confirm zero new broken links introduced.
