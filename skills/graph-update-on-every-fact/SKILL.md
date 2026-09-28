---
name: graph-update-on-every-fact
description: "Persist user facts to the KG before replying. See body."
version: 1.0.0
author: Hermes Agent
license: MIT
---

# Graph-update on every fact

## The rule

Whenever the agent learns a new fact about the user, the fact must be
**written to the knowledge graph before the agent replies**. No
"I'll add it later." No "let me batch this at end of session." The
graph is the source of truth — keeping it current is non-negotiable.

This is operational principle #9 in the user's `Preferences & working
style` facet. Captured 2026-08-03 after the Daily Memory subsystem
build, when the user said: *"whenever you learn something about me,
you need to add it to the knowledge graph as a note done forget that"*.

## When does this fire?

| Trigger | Example |
|---|---|
| User states a preference | "I prefer X over Y" |
| User states a project / repo | "I'm working on a new project called Z" |
| User states a habit / routine | "I usually do X at HH:MM" |
| User states a relationship | "X is my friend / colleague / family" |
| User states an environment fact | "I'm on a new device", "I switched from X to Y" |
| User states a recurring schedule | "Every Mon–Sat I do X" |
| User states a constraint | "I never use X", "I always do Y" |
| Agent notices a pattern | "I notice you keep X-ing" (only if the user confirms) |

## What NOT to capture (over-capture anti-patterns)

- Don't capture ephemeral task state (what they're working on *right now*)
- Don't capture every sentence (the graph is a journal, not a transcript)
- Don't capture things already in the cluster (use the `vault-knowledge-graph`
  skill's KG-search-before-asking rule to find existing notes)
- Don't capture private data the user didn't volunteer (medical, financial
  beyond what they shared, family member names without context)

## The workflow (every fact → before reply)

1. **Categorize the fact** to a facet:
   - Identity, projects, people, infra, preferences, hobbies,
     personal-growth, open-questions. See `vault-knowledge-graph`
     skill's "person-cluster-pattern" reference.
2. **Search first** — does this fact belong in an existing note? Run
   the KG-search-before-asking check from `vault-knowledge-graph`
   skill golden rule 1a before writing.
3. **Pick the target file:**
   - Hand-vault facet: `People/Facets/<Name>.md` (200-1500 bytes,
     snapshot-only)
   - Wiki long-form: `wiki/<area>/facets/<name>.md` (1-4 KB, structured)
   - Or a project / concept / daily note if the fact is about something
     other than the user themselves
4. **Write the fact** with a date stamp (`Captured YYYY-MM-DD`) so
   future agents can see when the fact was captured.
5. **Bump `updated:`** on the frontmatter.
6. **Cross-link** if it's a new facet / project / person — wire it
   into the relevant MOCs.
7. **Verify** with `python3 wiki/build_index.py --check` — must
   show `broken=0` (no new broken links from your edit).
8. **THEN reply to the user.** The graph update is part of the
   response, not a follow-up.

## Wikilink conventions (the one rule that breaks the most)

**Bare basename only.** `[[Daily Memory]]`, `[[Me]]`, `[[Tracker]]`.
**No** `[[../Daily Memory]]`, **no** `[[Daily Memory/]]`, **no**
`[[People/Me]]` (use `[[Me]]` — the basename resolver picks the
shortest path). The vault's `wiki/build_index.py` resolver does not
normalize `..` segments, so any `..`-style link is silently broken.

This rule has been baked into the `daily-memory-yesterday` cron
prompt (job `de9d4ff14fa7`) after the burn hit 49 broken links from
this exact mistake. Don't reproduce it.

## Idempotency rule (parallel-write safety)

When a cron or subagent is writing to a file the user might also
edit, the **idempotency check** must come first: "Does the file
exist + is it > 1000 bytes? Skip." No overwrites of valid content.
This is also operational principle #9 in the Preferences facet.

## Verification checklist (per fact)

- [ ] Fact categorized to a facet
- [ ] Searched for existing note that already covers it
- [ ] Picked the right target file (hand-vault vs wiki long-form)
- [ ] Wrote the fact with a date stamp
- [ ] Bumped `updated:` on the frontmatter
- [ ] Cross-linked if a new entity
- [ ] `python3 wiki/build_index.py --check` shows broken=0
- [ ] Index rebuilt (write, not just --check) for any agent that
  will search the graph later
