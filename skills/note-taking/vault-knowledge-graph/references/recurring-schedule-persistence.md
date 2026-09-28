# Recurring-Schedule Persistence (vault + wiki)

When the user states a **recurring weekly schedule block** ("DO Code
internship is Mon–Sat 5–7 PM", "morning workout is now 6:30–7 AM",
"Wednesdays = paper-reading evening"), persist it to BOTH the relevant
**vault routine note** and the relevant **wiki entity note** so the
fact is queryable from either side of the parallel-systems split. This
pattern came out of a 2026-07-29 session where Aditya stated the DO Code
internship window and I edited two notes to mirror it.

## When to use

Use this pattern when **all** of these are true:

- The user states a *recurring* block, not a one-off meeting or task.
- The block has *days* (e.g. Mon–Sat, Tue + Thu) AND a *time window*
  (e.g. 17:00–19:00, 06:30–07:00).
- The block maps to either an existing vault routine note OR an existing
  wiki entity note (or both). If neither exists yet, this isn't the
  pattern — first create the note, then come back.

Skip if the user only states a one-off ("let's meet Thursday 3 PM") — that
goes into a calendar / meeting note, not a routine block.

## The two-side write

A single recurring-schedule fact almost always has **two homes**:

| Side | Type | Example |
|---|---|---|
| Vault routine note | High-level "shape of my day/week" note in `Personal Growth/` or similar | `Personal Growth/Daily Schedule.md`, `Personal Growth/Gym routine.md`, `Personal Growth/Sauna cold wash.md` |
| Wiki entity note | Domain-specific entity that owns the schedule block | `wiki/work/entities/dataobserve-internship.md`, `wiki/work/projects/<x>-project.md` |

Edit both — the vault routine captures the *shape* ("this 2h block of my
week belongs to X"), the wiki entity captures the *ownership* ("this
entity's working hours are Y"). They cross-link via the vault routine's
MOC and the wiki entity's `Related` section.

## Edit checklist (per side)

For each note that gets the new schedule fact:

1. **Bump frontmatter `updated:`** to today's date. This is the
   cheapest signal that the note changed.
2. **Update any time-block table** that captures the schedule. If the
   note has a markdown table (e.g. Daily Schedule's 24-hour block, Gym
   routine's weekly split), find the matching row/cell and replace the
   generic description with the concrete window.
3. **Add a `## Fixed <X> schedule` section** with three bullets:
   - The block ("Mon–Sat 17:00–19:00 IST")
   - The off-days ("Sunday: no block")
   - The interpretation note ("This is a fixed two-hour work window; daily
     tasks should be selected to fit inside it")
4. **Cross-link if needed.** If the entity note links to the vault
   routine (or vice versa), add the back-link in `Related` /
   `See also` so the two notes resolve to each other.

## Verification (mandatory)

After editing both sides:

```bash
cd ~/Documents/fun && python3 wiki/build_index.py --check
```

Expect `broken=0`. A rising broken count means a wikilink rewrite went
wrong — run `patch` to fix before declaring done. (The skill's Golden
Rule 6 covers the verify-before-apply gate for any wikilink change.)

## Pitfalls

### Don't invent a third home

If the user states "internship is Mon–Sat 5–7 PM", don't create a new
"Internship schedule" note. The fact belongs in the existing
`Daily Schedule.md` (vault side) and the existing
`dataobserve-internship.md` (wiki side). Creating a third home
fragments the schedule across three notes and invites drift.

### Mirror, don't paraphrase

The two notes should say the same thing in their own voice. The vault
routine is high-level ("2h internship, 17:00–19:00 Mon–Sat"); the wiki
entity is project-specific ("DO Code internship runs Mon–Sat 17:00–19:00
IST, 2h block per working day"). Don't make them duplicate prose
verbatim — each side has a different audience.

### Schedule table row alignment

When updating a markdown table that has columns like
`| Block | Hours | Activities |`, the new "Activities" cell needs to fit
the existing column width visually but contain the concrete window.
Don't widen the table — match the existing visual rhythm.

### Cross-link safely

`build_index.py --check` is strict about wikilinks. If you cross-link
from the vault routine to a wiki entity, use the qualified form
`[[wiki/work/entities/dataobserve-internship|DataObserve Internship]]`
so it resolves cleanly from inside the vault tree. Bare `[[dataobserve
-internship]]` will silently fail.

### Update both sides in one pass

If you only update the vault side and forget the wiki entity, the wiki
will read as out-of-date when someone navigates from `Related` →
`[[dataobserve-internship]]` and sees no schedule there. Single-pass
edits prevent the "wiki is stale" failure mode that the parallel-systems
split is designed to avoid in the first place.

### Run the time arithmetic in a tool, not in your head

When a user states a window ("sleep 22:30 → 06:00", "exercise 7:00 →
8:30"), don't narrate the duration from memory. Mental math on 24h
blocks is the failure mode — verified in the 2026-07-29 session: I
claimed "11 clear daytime hours" and the user replied *"yo nigga 10 30
pm to 6 am is 8.5 hours"* and *"lets buddy calculate it again think
harder"*. Always:

```python
from datetime import datetime
delta = datetime(2026,7,31,6,0) - datetime(2026,7,30,22,30)
print(delta, "=", delta.total_seconds()/3600, "hours")
```

Then put the verified number in the table. The user will trust the
result; they won't trust the agent's off-the-cuff summary.

### Re-read the file before each subsequent patch in the same session

Successive `patch` calls on the same file can land the same content
twice. Verified 2026-07-29: I wrote a `## Fixed sleep window` section
twice in `Daily Schedule.md` because the second `patch` anchored on
trailing context the first `patch` had already moved. The fix: after
every `patch` that touches a heading or block, re-read the file (no
`offset` / `limit` pagination — full read) before the next `patch`. If
a section appears twice, `patch` away the duplicate immediately.

### Splitting a row is one patch, not two

When the user wants one block split into two ("17:00–19:00 Internship"
→ "17:00–18:00 Internship + 18:00–19:00 Meeting"), do it as a single
`patch` against the table row tuple (the line plus surrounding context).
Two separate `patch` calls risk the second landing after the first
shifted lines, producing a duplicate or a misplaced row. After the
patch, re-read the file to confirm only one row was added — not two.

### Bump frontmatter `updated:` on every schedule edit

The note is the user's canonical time-allocation. Staleness = silent
drift. Easy to forget when the user drips the schedule across turns
("5–7pm Mon–Sat", then "wake 6am", then "exercise 7–8:30", then
"1h work + 1h meeting"). Bump the frontmatter `updated:` field after
**every** schedule edit — not just the first one. Verified 2026-07-29:
I bumped once after the internship edit, then missed three subsequent
schedule edits in the same session.

### Block-name monotonicity in the 24h table

When the user drips the schedule across turns, accumulate into one
canonical 24h-block table by the end of the session. Update the
block-table row, the "What each block owns" list, and any "Fixed
<X> schedule" subsections in **lockstep** — never one without the
others, or the note desyncs internally. Verified 2026-07-29: after
splitting the Internship row, I updated the table but the
"What each block owns" list still described it as a single 2h block;
the next edit had to reconcile both.

## Worked example (2026-07-29)

User stated: *"for DO code internship, im gonna start working at 5 pm
to 7pm, monday to saturday"*.

Edits made:

| File | Change |
|---|---|
| `Personal Growth/Daily Schedule.md` | Frontmatter `updated: 2026-07-29`. Replaced the "Internship" row's generic "1.5h coding · 0.5h meeting" with "**17:00–19:00, Monday–Saturday** — DO Code internship work". Added `## Fixed internship schedule` section with three bullets. Updated the "Internship (2h)" bullet in `## What each block owns` to reference the DO Code window. |
| `wiki/work/entities/dataobserve-internship.md` | Frontmatter `updated: 2026-07-29`. Added `## Working schedule` section mirroring the three bullets. |

Verification:

```
$ cd ~/Documents/fun && python3 wiki/build_index.py --check
notes=370 stubs=21 assets=44 broken=0 ambiguous=154 dup_stems=22 orphans=1
                                                                       ^^
                                                                       ✓ unchanged
```

The same pattern will fire when the user next states "morning workout
is now 6:30–7 AM" → update `Personal Growth/Gym routine.md` (and any
work-block carve-out in `Daily Schedule.md`). Or "Tuesday evening is
paper-reading" → update `Daily Schedule.md` and the matching
research-side wiki entry.

## Worked example #2 — multi-turn schedule drip (2026-07-29, extended)

The user dripped the schedule across five turns in one session:

1. *"for DO code internship, im gonna start working at 5 pm to 7pm,
   monday to saturday"* → added `## Fixed internship schedule` to
   `Daily Schedule.md` and `## Working schedule` to
   `dataobserve-internship.md`.
2. *"lets setup up a wake up time and sleep time"* → added
   `## Fixed sleep window` to `Daily Schedule.md` (22:30 → 06:00, 7.5h).
   Then the agent offered a multi-choice; the user picked; the agent
   ran `execute_code` to verify 22:30 → 06:00 = 7.5h.
3. *"yo nigga 10 30 pm to 6 am is 8.5 hours"* + *"lets buddy calculate
   it again think harder"* → agent re-ran `datetime` arithmetic; the
   math was confirmed right (7.5h, not 8.5h). User accepted. **Lesson:**
   the user sanity-checks the agent's arithmetic; run the math in a
   tool before announcing it.
4. *"also you know other things i want to do throughout the day right?"*
   + out-of-band: *"excercise from 7 to 8 30 in the morning, right
   after i wake up i study and do some work or yoga for a bit"* →
   agent rewrote the 24h-block table with `06:00–07:00 Morning`,
   `07:00–08:30 Exercise`, `08:30–17:00 Work (8.5h)`, all in lockstep
   with the "What each block owns" list and the new "Fixed sleep
   window" section.
5. *"i have a company meeting from 6 to 7 pm so im gonna do an hour of
   work before hand and then attend the meeting for an hour boom, my
   work is done"* → split the 17:00–19:00 Internship row into
   17:00–18:00 Internship + 18:00–19:00 Meeting. Updated
   `## What each block owns` to add a separate "Meeting" bullet.
   Mirrored the split in `dataobserve-internship.md`.

What went wrong and the lesson:

- **Agent narrated "11 clear daytime hours" without verifying**
  (06:00 → 17:00 is 11 hours; the agent said "11.5" once and "11"
  once, neither run through a tool). User caught it. Lesson: every
  narrated number must come from `execute_code`, not from mental
  arithmetic.
- **Agent duplicated the "Fixed sleep window" section** in step #2
  because two `patch` calls in a row both anchored on the same
  trailing context after the first patch had moved the relevant text.
  Lesson: re-read the file before each subsequent patch; if a section
  is duplicated, fix it in the same pass.
- **Agent missed bumping the `updated:` frontmatter** on three of
  the five edits. Lesson: bump the frontmatter on every schedule
  edit, not just the first.

Final shape after all 5 turns:

```
Daily Schedule.md (Personal Growth/, 2026-07-29):
  24h block table:
    06:00–07:00  Morning     1.0   study · yoga · light work
    07:00–08:30  Exercise    1.5   0.5h cardio · 1.0h lifting
    08:30–17:00  Work        8.5   research · coding · learning · open sourcing
    17:00–18:00  Internship  1.0   Monday–Saturday — DO Code coding work
    18:00–19:00  Meeting     1.0   Monday–Saturday — DO Code company meeting
    19:00–21:00  Food/fam    2.0   meals + family
    21:00–22:30  Fun         1.5   Brawl Stars · music · YouTube · chill
    22:30–06:00  Sleep       7.5   22:30 → 06:00 fixed window

  Sections:
    ## Fixed internship schedule (3 bullets)
    ## Fixed sleep window (6 bullets)
    ## What each block owns (8 bullets, in lockstep with the table)
    ## How to use this (5 bullets, lever rule updated for 8.5h work block)

dataobserve-internship.md (wiki/work/entities/):
  ## Working schedule (5 bullets: block, split, Sunday, interpretation)
```

`build_index.py --check` ran after every patch; `broken=0` throughout.

## Worked example #3 — distributed-fun + 4-chunk Work block (2026-07-29, final)

The user pushed back on a single end-of-day Fun block ("*fun should also
be there during the day, i want you to plan a perfect day for me
distribute everything*"). The result is the **distributed-fun day
plan** — a reusable design pattern, not a fact.

### The design

| Time | Block | What |
|---|---|---|
| 06:00–07:00 | Morning | wake · sunlight · water · study / yoga / light work |
| 07:00–08:30 | Exercise | 30m cardio + 60m lifting |
| 08:30–09:00 | Reset | bath · breakfast · bathroom · cleanup |
| 09:00–11:00 | **Work A** | hardest cognitive work — freshest brain (2.0h) |
| 11:00–11:15 | **Fun micro #1** | coffee · music · walk (15m) |
| 11:15–13:00 | **Work B** | second-hardest work (1.75h) |
| 13:00–14:00 | **Lunch + Fun** | real lunch + 30 min Brawl Stars / YouTube / music (1h) |
| 14:00–15:30 | **Work C** | admin / lighter tasks — **post-lunch dip is real** (1.5h) |
| 15:30–15:45 | **Fun micro #2** | coffee · walk · music (15m) |
| 15:45–17:00 | **Work D** | wind down, write next-day plan (1.25h) |
| 17:00–18:00 | Internship | DO Code coding |
| 18:00–19:00 | Meeting | DO Code meeting |
| 19:00–21:00 | Food + fam | meals + family (dinner ends ~21:00) |
| 21:00–22:30 | **Evening fun** | Brawl Stars · music · YouTube · chill (1.5h) |
| 22:30 | Sleep | 7.5h fixed window |

= **3.0h explicit fun** distributed across 5 beats + 1h morning non-work
non-screen reset ≈ 4h of non-work recovery woven through the day. Total
tally 24h.

### Three design rules that made it work

1. **Post-lunch dip is real → Work C is admin, not peak cognitive work.**
   Chunk A is hardest (fresh brain), chunk C is lightest (dip), chunk D
   is wind-down. Don't reshuffle A and C — that puts your hardest work
   in the dip.
2. **Micro-breaks exist to keep 8h Work sustainable**, not to be
   collapsed into the evening block. The 15m coffee at 11:00 and the
   1h Lunch+Fun at 13:00 are load-bearing for Work quality.
3. **The evening fun block is the largest single fun beat on purpose.**
   It's where the day's tension gets metabolised before bed. Skipping
   it doesn't create more work time, it creates worse sleep.

### How to present it in the table

Two tables, not one mega-table:

- **Top-level 24h block table** keeps the same column shape as the
  existing schedule, but rows subdivide the Work block into A/B/C/D
  with explicit times. Don't try to keep Work as one row — the
  internal structure is the value.
- **Fun-distribution sub-table** lists the 5 beats with one-line
  rationale for each ("breaks up the morning peak before lunch").
- **Inside-the-Work-block sub-table** spells out A/B/C/D's purpose.
  This is the part the user pushes back on if you leave implicit.

### Pitfall: don't over-subdivide

The 15-minute micro-breaks only earn a row if they're *load-bearing*
for Work quality. Two micro-breaks is the right number — three becomes
accounting noise. Resist the urge to also add a 10:30 stretch break or
a 16:00 eye-rest; the 11:00 + 15:30 pair is the sweet spot.

## Worked example #4 — concept mirror for a routine note (2026-07-29)

After the schedule was locked, the user said "*obviously add it to
knowledge graph*". The right answer is **bridge-wiring**, not creating
a duplicate — the hand-vault `Personal Growth/Daily Schedule.md` is the
primary; the wiki gets a concept mirror.

### When a routine note earns a concept mirror

- The note is **load-bearing for the user's day** (24h allocation,
  weekly split, fixed commitments).
- The user just **spent multiple turns iterating on it** in this
  session. The mirror locks in the shape so future agents / queries
  find the canonical structure.
- The wiki side is currently **empty for that topic** (no
  `wiki/personal/concepts/daily-schedule.md` exists yet). Mirror is
  the right move; bridge-to-existing is not.

Skip if the routine note already has a wiki mirror (use the existing
counterpart link), or if the note is too thin to mirror (e.g. a
single-bullet stub — fill the hand-vault first, mirror later).

### Frontmatter pattern (per `wiki/personal/SCHEMA.md`)

```yaml
---
title: Daily Schedule (concept)
created: YYYY-MM-DD
updated: YYYY-MM-DD
type: concept
tags: [health, fitness]                # match the wiki's tag taxonomy
sources:
  - /home/arctic/Documents/fun/Personal Growth/Daily Schedule.md
confidence: high
private: true                          # default true — everything here is personal
counterpart: Personal Growth/Daily Schedule.md
---
```

The `counterpart:` field is the discovery hook — future agents
reverse-engineer the bridge from either side without a content scan.

### Body sections (canonical schema for routine concept mirrors)

1. Top quote block pointing at the hand-vault source.
2. Current state / schedule (the table).
3. Fun distribution (or equivalent: how the non-work time is split).
4. Internal structure of the dominant block (Work chunks, weekly
   rotation, etc.).
5. Pinned commitments (fixed wake/sleep windows, off-days).
6. "What this note is NOT" — daily log vs target, target-as-contract,
   not a substitute for individual concept pages.
7. Cross-cutting rules (the "How to use this" rules).
8. `## See also` — links to related vault + wiki notes.
9. `## Counterpart` — back-link to the hand-vault source.
10. `## Source` — provenance.

### Side-effects (mandatory, not optional)

When you create a concept mirror, also:

1. **`wiki/personal/index.md`** — add a new section or row pointing at
   the new concept. Bump the page count in the header line.
2. **The closest journal / state-summary note** — e.g.
   `wiki/personal/personal-growth-journal.md` for any personal-area
   concept. Update its description of the routine if the mirror
   captures new structure.
3. **The closest facet note** — e.g.
   `wiki/personal/facets/personal-growth.md`. Add a one-line pointer
   under "Where things live".
4. **`wiki/personal/log.md`** — append a new action entry listing
   files created + updated.
5. **The hand-vault source** — add `[[wiki/personal/concepts/daily-schedule|wiki: daily-schedule]]`
   to its `## See also` so the bridge is bidirectional.
6. **`build_index.py --check`** — verify `broken=0`. The mirror is
   one of the most likely places to introduce a broken link, since
   the qualified-path convention (`wiki/personal/concepts/...`) is
   easy to typo.

### Don't subagent for a single concept mirror

One concept mirror is **one** link pair (hand-vault → wiki), so the
"Don't subagent when scope ≤ 1 link" rule from
`references/bridging-parallel-systems.md` applies — patch directly.
The bridge mechanics are exact enough that a subagent adds latency
without adding accuracy.