---
name: codebase-gap-analysis
description: Audit missing features; anchor to files; tag C/D/M.
---

# Codebase Gap Analysis

> When the user asks "what should we add to project X?" or "is X state-of-the-art?",
> the bottleneck is *synthesis*, not *file reading*. Most of the work is reconciling
> the architect's own admission (DESIGN.md / README) with peer practice and the
> actual code. A bot that just enumerates "competitor features" misses the user's
> actual trade-offs and re-proposes things they deliberately cut.

## Workflow

1. **Read the architect's own writing first.**
   - `DESIGN.md`, `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `CONTEXT.md`.
   - Hunt for the milestone / status table — it tells you what's done, what's
     deferred, and what's cut, often in three columns.
   - Read "Tradeoffs taken" / "What I'd do differently" / "Known limitations"
     / "Future work" sections *first* — those are pre-existing gap analyses
     and are higher-signal than any external review could be.
   - `git log --oneline -20` and grep commits for `defer|TODO|wishlist|next`
     — commit messages are a second channel of the architect's intent.
   - If there's a `bench/` / `eval/` / metrics, read it last. It tells you
     what the user has decided *matters*, so recommendations can be ranked.

2. **Categorize every absence before recommending anything.** Two-column rule:
   - **(P) = present / wired**
   - **(A) = absent**, split into three sub-buckets:
     - **(C)ut by user decision** — present in DESIGN.md / AGENTS.md as a deliberate de-scope. Do NOT re-propose; it's busywork and erodes trust.
     - **(D)eferred, not abandoned** — user said "later." Re-proposing with a fresh pitch is OK if the deferral reason no longer applies.
     - **(M)issing** — actually a gap, peer parity, or SOTA candidate.

   A reviewer who lists MCP / hooks / sandbox without checking the cut-list
   signals they didn't read the project. Categorization is the discipline.

3. **Anchor every recommendation to a specific file or section.** Vague
   recommendations ("improve the loop") are vapor. Concrete ones ("make
   `src/core/loop.ts` event-sourced — move `Session.messages` into a reducer
   passed through the loop, yields `Intention<T>` that an outer interpreter
   applies") are actionable. Each gap should name the file it would touch
   and, if possible, a sibling file or vendored reference to model on.

4. **Mine peer codebases that are already in the tree.** If the audited
   project vendors a reference (e.g. `opencode/`, `claude-code/`, `codex-cli/`),
   the directories it has but the audited project doesn't are *concrete
   parity gaps with reference implementations one directory tree away*.
   Use `find <vendored>/packages -maxdepth 4 -type d` to map the peer's
   surface and diff against the audited project's `src/`.

5. **Reality-check SOTA claims per recommendation.** A feature in peer A
   doesn't make it SOTA — peer B might not have it. Add a short
   "SOTA reality check" line per gap: split / consensus / one-sided. This
   stops cargo-culting.

6. **Produce the deliverable in priority order with honest effort estimates.**
   - Lead with the architect's own admission (highest leverage — they've
     already done the analysis and accepted the trade-off of leaving it).
   - Then SOTA peer parity (biggest user-visible wins).
   - Then operational / observability gaps (move the metric, not the feature).
   - End with: "what looks like a gap but isn't" — actively defuse bad
     proposals before they reach the user.
   - Effort estimates: "1 hour" means one hour, "1–2 weeks" means 1–2 weeks.
     Don't pad or shrink to make the list look better.

## Pitfalls

- **Don't delegate when reasoning is the bottleneck.** OpenCode / Claude Code
  / Codex CLI are great at multi-file code surgery. They're not great at
  synthesizing design intent from a `DESIGN.md`. If the user's request is
  *review / audit / gap-analyze*, read the source yourself and produce the
  artifact. Save delegation for "implement this list of changes."

- **When the user says "maybe use X," it's optional.** Pick the tool that
  fits the bottleneck. If they meant X as a reference (vendored codebase,
  doc set), mine it directly. If they meant X as a worker, delegate only if
  the work is mechanical and reasoning-light.

- **Don't re-propose deliberately-cut features.** Search `DESIGN.md` for
  `cut|removed|killed|rejected|defer|not implement` before writing the gap
  list. The cut-list belongs in a table in the deliverable so the user sees
  you've read it.

- **Don't anchor on a single peer.** LSP exists in OpenCode / Cursor but
  not in Claude Code / Codex CLI. Skills exist in Claude Code / OpenCode
  but not in Cursor's CLI. Reality-check each SOTA claim with a "what do
  peers say?" note rather than a single reference.

- **Watch the metric.** If the audited project has a benchmark or eval,
  separate "moves the metric" from "doesn't move the metric but is
  otherwise good." The user often has the metric in mind already and
  appreciates proposals that name the connection.

- **Length: one screen of markdown for the prose, tables for everything
  else.** A 30-page review is a feature wishlist in disguise.

## Anti-patterns

- **Feature wishlist.** "Add images, voice, MCP, hooks, plugins, themes,
  animations, …" is a search-results page, not a review.

- **Cargo-culting.** If the user's project is explicitly minimal ("small
  sharp kernel," "deliberately from scratch"), most peer features are
  *intentionally* absent. Read the project's own framing first; respect
  it.

- **Conflating "SOTA" with "complex."** Some SOTA moves are simplifications
  (event sourcing, content addressing, single-reducer loops). Some are
  additions (skills, worktrees, LSP). Be explicit which type you're
  proposing.

- **Proposing things the architect just deferred yesterday.** Check the
  recent commits and the milestone table's "deferred" column. Fresh
  deferrals are still deferrals.

- **No anchor = no recommendation.** If you can't point at the file it
  would touch, you haven't thought hard enough about the gap. Drop it
  or qualify it as "speculative."

## Deliverable shape

```
# <Project> — Gap Analysis

## One-line summary
## Inventory of what's wired (table: capability | status | notes)
## Deliberately cut or deferred (table — DON'T re-propose)
## Real gaps
  ### A. Architect's own admission (from DESIGN.md "What I'd do differently")
  ### B. SOTA peer parity (compare against vendored or known peers)
  ### C. Operational / observability
## Priority ordering (with effort estimates)
## What looks like a gap but isn't
## Bench impact (if there's a benchmark)
```

Length: ~250–400 lines for a substantial project. Smaller for a small one.
Use `templates/gap-analysis.md` as the starting skeleton.

## Verification

Before declaring done, check:

- [ ] Every recommendation has a file path or section anchor in the codebase.
- [ ] Every absent item is categorized C / D / M, not just "missing."
- [ ] The cut / deferred table is non-empty (if it's empty, you didn't read DESIGN.md).
- [ ] The "looks like a gap but isn't" section exists and is non-empty (it defuses bad proposals).
- [ ] Effort estimates match the architect's own if any exist.
- [ ] If there's a benchmark, the bench-impact section names which proposals move the metric.
- [ ] The deliverable fits in one screen of prose; tables for everything else.

## References

- `templates/gap-analysis.md` — copy-able markdown skeleton for the deliverable.
- `references/rocky-code-worked-example.md` — condensed walkthrough of the
  2026-08-06 rocky_code gap analysis: what DESIGN.md contributed, what the
  vendored opencode/ contributed, what the bench told us, and the anti-list
  that defused bad proposals. Read before your first run of this skill.