# Personal-Tracker + Reference-Hub Pattern

When the user asks to add a personal-life subsystem that combines **two
artifacts** — a *reference knowledge base* (curated from a public source) and
a *daily tracker* (your own data) — wire them together via an Adoption log.
This pattern came out of setting up the Bryan Johnson protocol reference +
fitness tracker in `Personal Growth/` (Arctic, 2026-07-26).

## When to use

Use this pattern when **all** of these are true:

- The user names a public figure / company / system whose protocol they want
  to follow (Bryan Johnson, Andrew Huberman, Tim Ferriss, etc.).
- The user wants to log their own daily habits alongside it (sleep,
  supplements, mood, energy, vitals).
- The user implicitly or explicitly says *"reference, not prescription"* —
  i.e. they understand they shouldn't 1:1 copy a protocol optimized for a
  different body/context.

Skip if the user only wants one of the two (pure reference → just create a
reference folder; pure tracker → just create a tracker folder).

## The four-file shape

```
Personal Growth/                       (or whatever parent MOC exists)
├── Personal Growth.md                  ← updated MOC: list the new hub as one
│                                          of 2-4 top-level sections
├── Physical Fitness.md                 ← NEW umbrella MOC (the new hub)
├── Recovery.md / Supplements.md / ...  ← NEW empty concept stubs the user
│                                          will populate from their own data
├── Tracker/                            ← NEW: the user's data
│   ├── Tracker.md                      ← NEW: MOC (so bare [[Tracker]] resolves)
│   ├── _template.md                    ← NEW: copy-paste starter
│   ├── dashboard.md                    ← NEW: Dataview aggregate
│   └── YYYY-MM-DD.md                   ← NEW: today's empty log
└── bryan_routine/                      ← NEW: the reference knowledge base
    ├── README.md                       ← NEW: index + "reference, not Rx" warning
    ├── Protocol.md                     ← NEW: high-level, dated
    ├── Source log.md                   ← NEW: provenance ledger (every claim
    │                                          traces to a row here)
    ├── Adoption log.md                 ← NEW: what you tried, effect, status
    └── Concepts/
        ├── Sleep.md / Nutrition.md /   ← NEW: one per subsystem
        ├── Supplements.md / ...
        └── (each marked `status: scaffolding`)
```

## Why each file exists

| File | Job |
|---|---|
| **Umbrella MOC** | Single navigation point. Lists subsystems, points at reference folder, points at tracker folder. Without this, the new domain is invisible from the top-level MOC. |
| **Reference README** | Index for the reference. Carries the "reference, not prescription" warning at the top so the user sees it every time they open the folder. Includes a link to the Adoption log. |
| **Protocol** | High-level summary, dated `last_updated`, with a sources list. Empty content fine on first pass — gets populated by a subagent pulling from `blueprint.bryanjohnson.com` or equivalent. |
| **Source log** | Provenance. Markdown table with columns: Date / Source URL / What we took / Trust level / Updated which notes. The skill's #1 rule ("never fabricate") makes this mandatory. |
| **Concept files** | One per subsystem (Sleep, Nutrition, etc.). Mark `status: scaffolding` and `counterpart:` in frontmatter. Body has section headers (What he does / What we know / What we don't know / What we'd adopt / See also) so future subagents have a fillable structure. |
| **Adoption log** | One row per thing the user actually tried. This is what makes the reference actionable instead of aspirational. Without it, the user just collects protocol content and never runs an experiment. |
| **Tracker MOC** | Exists so `[[Tracker]]` resolves from anywhere. Without it, every Tracker reference becomes a broken folder-only wikilink. |
| **_template** | Single-page daily template. Goal: fillable in 30 seconds. Sections: Sleep / Nutrition / Training / Supplements / Vitals / One thing I noticed / BJ protocol adherence today. If you can't get it under 30s to fill, you're over-scoping. |
| **dashboard.md** | Dataview table over the last 7 days + 30-day averages. Tables only populate as data accumulates, so this file is mostly empty for the first 2 weeks. |
| **YYYY-MM-DD.md** | Today's empty log. User copies `_template` into this daily. The dated filename is the join key for the dashboard queries. |

## Pitfalls (learned 2026-07-26)

### Wikilink traps in the new files

After building this whole tree in one go, `build_index.py --check` reported
37 broken links. All from the same class of mistakes:

1. **`[[bryan_routine]]`** (folder-only) → doesn't resolve. Fix:
   `[[bryan_routine/README|bryan_routine]]`.
2. **`[[Tracker]]`** (folder-only) → doesn't resolve. Fix:
   `[[Tracker/Tracker|Tracker]]` (with the MOC file).
3. **`[[../Physical Fitness]]`** (up-path with spaces in name) → doesn't
   resolve because the resolver does `c.lower() == want` + `endswith` and
   "physical fitness.md" is never a basename match for the path-form. Fix:
   `[[Physical Fitness|Physical Fitness]]` (basename).
4. **`[[Concepts/Sleep|Sleep]]`** when no `Concepts/Sleep.md` exists → 0
   matches. Fix: either create the stub (best), or convert the link to
   italic prose (`_Sleep concept hub (not yet written)_`) which doesn't
   render as a broken wikilink.

**Lesson:** when scaffolding a whole subtree in one batch, plan the link
forms *first*. Default safe patterns:
- Cross-folder: `[[path/to/file|display]]` with the actual file basename
- Up-folder to existing note: `[[Basename|Display]]` (rely on global stem lookup)
- Forward-reference to not-yet-written note: italic prose, not wikilink

### Verification steps after the scaffold

After creating the new tree:

1. `cd wiki && python3 build_index.py --check` — expect `broken=0`
2. Spot-check 3-5 of the new files and verify the `See also` links
   actually point at intended targets (not just *any* target with the same
   stem). `build_index.py --check` reports `ambiguous > 0` is fine, but
   for any ambiguous target the agent must decide which candidate wins.
3. Confirm the new umbrella MOC appears in the top-level MOC (so the user
   sees it on `Me.md`).

### Filling in the scaffold (next passes)

The scaffold deliberately has placeholders. Populate in this order to avoid
the "skeleton without meat" failure mode:

1. **Protocol.md high-level shape** — 1-2 paragraphs of "what the protocol
   is at the highest level". User can do this in 5 minutes from memory;
   no subagent needed.
2. **One concept at a time** — usually Sleep first because it's the most
   universal. Dispatch one subagent per concept, with the rule "every claim
   cites a row in Source log.md; if no row exists, don't write the claim".
3. **Source log entries** — populated *as* concepts are populated, not
   after. Otherwise you lose the chain of custody.
4. **Adoption log** — only the user fills this, based on their own
   experiments. Don't seed it with fabricated rows.

### The "don't die from Bryan Johnson" warning

Bryan Johnson specifically has a $2M/year clinical team monitoring him.
**Copying his protocol 1:1 is medically risky for a 20-something in Delhi
with different genetics, sunlight exposure, gut biome, and stress load.**

This warning belongs at the top of:
- The reference folder's README
- Each concept file
- Optionally the umbrella MOC

Don't bury it. Every time the user opens the folder, they should see it.

## Worked example

`Personal Growth/` (Arctic, 2026-07-26). 22 new files created in one pass,
all linked (`broken=0`), content left as `status: scaffolding` to be
populated by future subagents with sourced data. Tracker is empty waiting
for the user's first daily log. Adoption log has zero entries waiting for
the first experiment.