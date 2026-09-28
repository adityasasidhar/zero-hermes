---
title: Competition-notes pattern — MOC + per-comp folders + status field
source: session 2026-08-10 (Aditya registered for 4 competitions)
applies_to: vault-knowledge-graph skill, rule 5 ("Add a new domain — scaffold-then-fill")
---

# Competition-notes pattern

When the user has a list of competitions they want to be in (hackathons,
contests, challenges, prize-benchmarks, datathons), and asks for the
agent to scaffold the workspace so they can start their notes, this is
a discrete class of work. Session-evidenced 2026-08-10: user said *"I
have a list of competitions I wanna be in, so check it out once"* and
shortly after *"make a dir in the root for each of these so that I can
start my notes directly"* — the deliverable was an umbrella MOC + one
folder per competition, NOT a deep per-comp note.

## Minimum-viable shape

```
Competitions/
├── Competitions.md           # MOC, lists each comp with status + deadline + link
├── _template.md              # optional, copy-paste per-comp scaffold
└── <Competition Name>/       # one folder per competition
    ├── <Competition Name>.md # the per-comp note (live notes go here)
    ├── prep.md               # optional, prep checklist / links
    └── submissions/          # optional, for code/demos/written submissions
```

**Vault-root placement.** When the user says "in the root" / "make a
dir in the root", interpret as **vault root** (`/home/arctic/Documents/fun/`),
NOT filesystem root. Vault structure is the only sensible home for
personal knowledge; placing competition notes under `/` or
`/home/arctic/Competitions/` orphans them from the rest of the graph
and breaks MOC navigation.

**Folder name = competition name, verbatim.** The user wrote the names
themselves ("e-Yantra", "arc agi 2", "IBM August challenge", "Into the
Scrape-Verse"); preserve the casing and spacing — these become
candidate wikilink targets and on-disk folders. Don't normalize them
to "eyantra" or "arc-agi-2" without explicit user confirmation. Spaces
in folder names are fine (Obsidian handles them; so does Unix
filesystems — quote the paths in shell calls).

## Order of operations (don't over-engineer the first pass)

1. **Confirm the list is final** before mkdir'ing. If the user
   says *"I have a list of X"* and immediately follows with *"make a
   dir for each"*, the list is implicitly final — don't ask "should I
   also create a dir for X+1?". A clarifying question is correct only
   if the user's intent is ambiguous (e.g. *"what about conferences?"*).
2. **Create one folder per item, in one shell call.** `mkdir -p` with
   all paths, one terminal invocation. Don't loop with separate
   `mkdir` calls — slower and easier to fail halfway.
3. **Stop there for the first pass.** The user said *"so that I can
   start my notes directly"* — they want empty workspaces, not a
   4-note deep-enrichment burn. Don't proactively fill the folders
   with research, prep plans, or web-search results; that's the
   user's job. The first pass's deliverable is *navigability*, not
   *content*.
4. **Optionally scaffold the MOC + a `_template.md`.** If the user
   later asks to "structure this properly" / "add a MOC" / "set up a
   dashboard", THEN build the umbrella note and the per-comp schema.
   The order is: folders → user fills notes → MOC + template. NOT:
   folders + MOC + template + auto-research in one pass.

## Per-comp note schema (for the second pass)

When the user later wants structure inside each comp folder, the
schema is:

```markdown
---
competition: <name>
status: planning | registered | building | submitted | result | dropped
deadline: YYYY-MM-DD | rolling
prize: <amount + currency>
url: <official URL>
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# <Competition Name>

## At a glance
- **Format**: <hackathon / static benchmark / datathon / prize-bench>
- **Team**: solo / team (members: ...)
- **Prize**: <amount>
- **Deadline**: <date>
- **Fit**: <1-3 bullets on why/why-not>

## Plan
- [ ] Step 1
- [ ] Step 2
- [ ] Step 3

## Notes
(live notes — research, decisions, links, blockers)

## Submission
- [ ] Code repo: <url>
- [ ] Demo video: <url>
- [ ] Writeup: <url>
```

`status:` field is the **single most important** schema field — it
makes the MOC queryable via Dataview (`TABLE status, deadline FROM
"Competitions" SORT deadline ASC`). Without it, the user has to open
each note to see what's active.

## MOC schema (Competitions.md)

```markdown
---
type: MOC
tags: [moc, competitions]
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# Competitions

## Active

- [[Into the Scrape-Verse]] — Aug 17-23 2026, $15K + DGX Spark
- [[ARC-AGI-2 on Kaggle]] — Nov 2 2026, $700K
- [[e-Yantra Robotics Competition]] — Aug 25 2026, ₹10L

## Submitted / awaiting results

(none yet)

## Dropped

(none yet)
```

The `## Active` / `## Submitted` / `## Dropped` buckets track the
**lifecycle**, not just the comp list. Comp notes themselves carry
the status; the MOC reflects it.

## Common pitfalls

- **Don't mkdir under `/` or `/home/arctic/`.** Always under the vault
  root. The user's personal knowledge lives in the vault; competitions
  are personal knowledge.
- **Don't quote-comp-normalize the names.** User wrote "IBM August
  Challenge" — folder is `IBM August Challenge/`, not
  `ibm-august-challenge/` or `IBM-August-Challenge/`. Same for
  "Into the Scrape-Verse" (the dashes + capital S are intentional).
- **Don't auto-research before the user asks.** The user wanted
  *directories*, not a research dump. Research happens when the user
  opens one of the folders to start notes.
- **Don't wire the MOC into Me.md on the first pass.** The Competitions
  MOC is new; until the user signals it's part of their graph (asks to
  "add to my MOC" or "link from Me"), leave Me.md alone. Adding
  `[[Competitions]]` to Me.md speculatively clutters the user's home
  note.
- **Don't use case-sensitive file names for wikilinks unless you've
  verified the case.** `[[competitions to be in]]` (lowercase, spaces)
  resolves because the file IS lowercase + spaces. `[[Competitions]]`
  does NOT resolve to `competitions to be in.md` — capitalization
  matters. Match the on-disk case exactly. (See skill rule 5 +
  `references/broken-link-repair.md` for the case-sensitivity rule.)

## Worked example (2026-08-10)

Aditya said *"I have a list of competitions I wanna be in, so check
it out once"*. Searched the vault (case-insensitive broad pass after
a narrow miss), found `competitions to be in.md` with 4 lines. Asked
which direction to take; user picked "research each one" — agent ran
4 parallel web searches and produced a deadline-ordered comparison
table. User then said *"i registered for all of these"* — agent
updated the vault note with status, committed to memory, and asked
*"want me to build a Competitions/ MOC with one note per competition
(deadlines, prep checklist, links, status)?"*. User said *"make a dir
in the root for each of these so that I can start my notes directly"*
— agent ran `mkdir -p` for all four folders, stopped there. **No
notes written inside the folders; no MOC created.** That's the
correct first-pass scope.