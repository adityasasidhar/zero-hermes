# Person super-cluster pattern (multi-facet hub)

When the user says *"super-cluster my information"*, *"add everything
you know about me to the graph"*, *"make it a real cluster, not just
a star"*, or *"I want my profile to feel like a graph"* — extend a
basic person profile (rule 5a) into an N-facet super-cluster.

The base pair from rule 5a (`People/Me.md` +
`wiki/<area>/entities/<slug>.md`) is **necessary but not sufficient**
for a real cluster. With only those two, the cluster is a star: both
have `incoming=0` (nothing in the vault points at the person), and the
person's content is a *target* the rest of the vault orbits around but
the orbits go to nowhere.

The super-cluster pattern is a *mesh*: a central hub surrounded by
~6–8 facets, each cross-linking to the hub AND to every other facet,
and reciprocating from the rest of the vault. Result: `incoming`
jumps from 0 to ≥3 per note, and the cluster becomes a navigation
surface the user can actually use.

## When to use

After rule 5a (the basic person profile) is in place, the user asks
for any of:
- "Super-cluster my information"
- "Make the profile denser / richer"
- "I want a real cluster, not just a star"
- "Add everything you know about me" (after the base pair exists)
- "There's a star shape — fix it"

Don't speculatively build this before the user asks. The base pair
(rule 5a) is fine for many sessions. The cluster adds N more files
and N more cross-cuts to maintain — only worth the cost when the user
sees it as the goal.

## The 8 facets (Aditya worked example)

Picked on **orthogonal axes** (not category overlap), so each facet
has lateral links to others:

| # | Facet | Purpose | Anchored MOC |
|---|---|---|---|
| 1 | Identity & bio | Birth date, GitHub, school, timezone, pronouns | `[[Me]]`, `People/Me.md` |
| 2 | Projects & work | Active threads, OSS contributions, side projects | `Projects/` |
| 3 | People & relationships | Familia, Friends, communication filter | `People/` |
| 4 | Knowledge graph & infrastructure | Vault structure, 4 wikis, Astro blog, scripts | `wiki/index` |
| 5 | Preferences & working style | Comms, 8 operational principles, burn-mode style | (no anchor — discovers via cluster) |
| 6 | Hobbies & creative | Cooking, artistic, instruments | `Hobbies/` |
| 7 | Personal growth | Daily schedule, gym, sauna, recovery | `Personal Growth/` |
| 8 | Open questions | Pronouns, household, long-term goals — canonical unknowns | (no anchor — always recommended) |

The "8" is a feel-good number, not a magic count. Use 5–8 facets;
fewer than 5 isn't worth the multi-file cost, more than 8 starts
overlapping on axes. The 8 axes chosen here are *dimensionally
distinct* (axis = a kind of fact about the person), not *categorically
distinct* (don't subdivide one axis into two facets — e.g. don't
have both "Bio" and "Contact info" facets; merge into "Identity &
bio").

Open questions as the 8th facet is **always recommended** — it's
the canonical home for the things you don't know (pronouns from
inferred cues, birthday not yet captured, long-term goals not yet
written). Without it, unknowns dangle in the central entity forever;
with it, when something is resolved (e.g. birthday captured 2026-07-29),
you promote it *out of* open-questions and *into* the relevant facet
("Identity & bio"). Cluster ↔ factual capture work as one
continuous workflow.

## File shape (per facet = 2 notes)

### Hand-vault facet: `People/Facets/<Name>.md` (200–1,500 bytes)

Short, snapshot-only, MOC-shaped. Pattern:

```markdown
---
type: MOC
tags:
  - moc
  - self
  - facet
  - <axis>
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# <Facet name>

> Facet of [[Me]]. Long-form at
> [[wiki/<area>/facets/<name>|wiki: <name> facet]].

## <Section matching the facet's purpose>

<one-paragraph snapshot, ~50–100 words>

## Linked

- [[Me]] — central hub
- [[Facets/<adjacent-facet>|<Adjacent facet name>]] — sibling facet
- [[<root MOC for the axis>|<anchor>]] — hand-vault anchor

## See also

<optional: one or two outbound links to existing root-MOCs that
this facet cross-cuts>
```

The hand-vault facet is a *navigation node*, not a destination.
Don't duplicate content from the wiki long-form here — the central
hub page points at the wiki form, and Obsidian's Quick Switcher
finds the wiki form on click-through.

### Wiki long-form: `wiki/<area>/facets/<name>.md` (1–4 KB)

Structured body with `## Cluster position` and `## Related`
sections. Pattern:

```markdown
---
title: <Facet name> (facet)
created: YYYY-MM-DD
updated: YYYY-MM-DD
type: facet
tags:
  - facet
  - <axis>
sources:
  - "<hand-vault facet path>"
confidence: high
private: true
counterpart: People/Facets/<Name>.md
---

# <Facet name> — <Subject>

> **Facet of the <Subject> cluster.** Hand-vault hub:
> [[<basename-resolves-to-hand-vault-facet>]] (basename resolves).

## Cluster position

This facet is the **<axis> anchor** of the cluster — every
other facet cross-references it for stable <axis> facts. Keep it
small and high-signal.

## <Body section matching the facet's purpose>

<2–5 paragraphs with real content, citations where applicable,
wikilinks to the underlying vault notes.>

## Related

- [[Me]] — central hub (hand-vault)
- [[wiki/<area>/entities/<slug>]] — long-form entity
- [[Facets/<adjacent-facet-1>|<Adjacent facet 1>]] — sibling facet
- [[Facets/<adjacent-facet-2>|<Adjacent facet 2>]] — sibling facet
```

The wiki facet is the *destination*. Wikilinks here resolve to
other wiki notes (or hand-vault notes via the bridge pattern), so
resolving within the wiki is the goal. Keep it focused: one
facet = one axis, not a kitchen-sink.

## Cross-linking pattern (the mesh)

Each facet links to:
- The **central hub** (one of `People/Me.md` /
  `wiki/<area>/entities/<slug>.md`)
- **Every other adjacent facet** in the cluster

The central hub and entity each list all 8 facets.

```
                  ┌─ Identity & bio ────┐
                  ├─ Projects & work ───┤
                  ├─ People & rels ──────┤
  People/Me.md  ←→ ├─ Infra & KG ────────┤ ←──→  <slug>.md
  (hand hub)     ├─ Preferences ────────┤      (wiki long-form)
                  ├─ Hobbies ───────────┤
                  ├─ Personal growth ──┤
                  └─ Open questions ───┘

          ↕ reciprocated from Hobbies/, Personal Growth/, Goals/
```

The result: 18 cluster notes, 192 outbound edges, 105 incoming
edges. Before the cluster: 2 notes, 92 out / 0 in. The metric
that matters is `incoming` per facet, not total edge count —
incoming=0 is the star-shape signal that triggered the rebuild.

## Reciprocation from existing root MOCs

The single biggest `incoming` win comes from reciprocating
**from existing root MOCs**. For the Aditya cluster, three
existing notes that *should* link to the cluster but didn't:

- `Hobbies/Hobbies.md` → `[[People/Facets/Hobbies and creative|Hobbies & creative (facet)]]` (under `## See also`)
- `Personal Growth/Recovery.md` → `[[People/Me]]` + `[[People/Facets/Personal growth|Personal growth facet]]`
- `Goals/goals.md` → `[[People/Me]]` + `[[People/Facets/Open questions|Open questions facet]]`

Root MOCs have lots of traffic elsewhere in the vault (they're
linked from `Me.md`, referenced from subtopics, etc.), so
*each reciprocation pulls multiple following-citations along
with it* via back-link discovery.

Pick the reciprocation targets deliberately:

| Reciprocate if | Don't reciprocate if |
|---|---|
| The MOC is visited often (root-level, transitively linked) | The MOC is a dead-end (no other notes link to it) |
| The MOC's topic maps cleanly to a facet | The MOC's topic is borderline (could go in 2+ facets; unclear) |
| A natural "See also" sentence exists (no awkward wording) | You'd have to write forced prose to make the link fit |

## Pre-write validation (the broken-link gate)

Cluster builds emit three predictable broken-link patterns. **Check
for them after each wave**, not at the end:

1. **Wrong namespace prefix** —
   `[[aiml-entities/babylm-recursive-hybrid]]` looks right
   semantically (it *is* in `wiki/aiml/entities/`) but the resolver
   doesn't walk the path. The form that resolves is
   `[[babylm-recursive-hybrid]]` (bare basename). Subagents
   faithfully copy whatever the brief says; if the brief says
   `<namespace>/<basename>`, you'll ship broken links.
2. **Ambiguous bare basename** — `[[aiml]]` matches `wiki/aiml/index.md`,
   `wiki/aiml/SCHEMA.md`, `wiki/aiml/log.md`, `wiki/aiml/concepts/aiml.md`...
   Resolver picks one (usually by length, picking the shortest path),
   which may not be the one you want. **Fix:** use
   `[[wiki/aiml/index|aiml]]` (path-with-filename + pipe-alias).
3. **Folder-only paths** — `[[Hobbies/Artistic/]]` looks plausible
   because the folder exists. Resolver doesn't walk folders; it
   does `c.lower() == want` + `endswith("/" + want)`, neither matches
   the folder. **Fix:** either drop the link, use a folder-with-
   filename path (`Hobbies/Artistic/Artistic.md` if it existed), or
   reference the file directly in prose.

The two-second verification that catches all three:

```python
import re, os
new_note = "/home/arctic/Documents/fun/<path>/<note>.md"
text = open(new_note).read()
targets = set(re.findall(r'\[\[([^]|]+)(\|[^]]*)?\]\]', text))
for t in targets:
    basename = t.split('|')[0].strip().split('/')[-1].lower().rstrip('.md')
    matches = []
    for r, _, files in os.walk('/home/arctic/Documents/fun'):
        for f in files:
            if f.lower() == basename + '.md':
                matches.append(os.path.join(r, f))
    if not matches:
        print(f'BROKEN: [[{t}]] — no note named "{basename}.md" found')
    elif len(matches) > 1 and not '/' in t:
        print(f'AMBIGUOUS: [[{t}]] — {len(matches)} matches: {[m for m in matches]}')
```

Run that BEFORE `write_file` on every new note, or at minimum after
every wave. The first cluster build in this session shipped **13
broken links** that took 5 patch iterations to clean — running the
gate beforehand would have caught all of them at write time.

## Closing checklist

After the cluster is built and validated:

1. `python3 wiki/build_index.py --check` shows `broken=0` and
   `ambiguous` rose by the expected amount (Aditya cluster: +5
   intentional bridges).
2. Each of the 18 cluster notes has `incoming ≥ 3` (the
   mesh-shape signal). Run the diagnostic at the start of
   `references/broken-link-repair.md` Step 1 if any node reports
   `incoming=0` after the cluster build — it missed a
   reciprocation.
3. Append a single dated log entry to
   `wiki/<area>/log.md` capturing: files created (count),
   files updated (count), incoming/outbound deltas before→after,
   and a one-paragraph recipe for adding the *next* facet (so
   future agents don't have to rediscover).
4. Bump `updated:` on the central entity long-form
   (`wiki/<area>/entities/<slug>.md`) and on the hand-vault hub
   (`People/Me.md`).

## Extending the cluster later

Adding a 9th facet 6 months from now:

1. Write the 2 new facet notes (hand-vault + wiki).
2. Add the central-hub link in **every existing facet** (touch
   9+1 files). One `[[Facets/New Name|New Name]]` line per
   existing facet's `## Related` section.
3. Add the new entry to `wiki/<area>/index.md` under `## Facets`.
4. Bump `updated:` on the central hub + entity.
5. Run `python3 wiki/build_index.py --check` — `broken=0`.
6. Log entry.

The cluster grows outward cleanly without rebuilding. The
"8 → 9 → 10" growth is local + idempotent; the only risk is the
"touch every facet" step being skipped, leaving the new facet
disconnected. Make that step a checklist item.

## What this is NOT

- **Not a knowledge-base build.** This is a navigation graph. The
  knowledge base is the wiki `aiml` / `learning` / etc. The cluster
  is the *index* to that knowledge base for facts about the
  person.
- **Not a primary source.** Don't write facts here that aren't
  backed by a primary source (vault note, GitHub repo, etc.). The
  cluster links to sources; the sources carry the facts.
- **Not a privacy tool.** Everything here is `private: true`
  in frontmatter but Obsidian doesn't enforce that — vault
  permissions are operational, not artifact-enforced.
