# Worked example — rocky_code gap analysis (2026-08-06)

Source deliverable: `/home/arctic/projects/rocky_code/GAP_ANALYSIS.md` (full version, ~280 lines).

This reference condenses the worked example to show what the technique produces when
applied to a real codebase. Read it next to the SKILL.md, not instead of it.

## Project shape

- Bun + TypeScript, ~3,000 LOC.
- Terminal coding agent CLI (Claude-Code-style), three providers, two TUIs, sub-agents,
  plan mode, post-edit checks, compaction, bench harness.
- Vendors a full `opencode/` monorepo (~2.5 GB) as reference material.

## What the architect's own writing told us

`DESIGN.md` milestone table: hooks / MCP / `/undo` all "cut by user decision";
background bash and `--resume` "deferred by user decision." Resumable streaming
"future work." This was the single most important read — it eliminated ~6
proposals in the "things people will suggest" bucket before they were written.

`DESIGN.md` "What I'd do differently" surfaced three first-class items:
mutable Session, compaction-fires-twice-per-turn, sub-agent black-box. All three
became the §3.A lead items because the architect had already done the analysis
and accepted the trade-off of leaving them. Highest leverage.

`AGENTS.md` "Gotchas" + `DESIGN.md` "Tradeoffs taken" gave the §5 anti-list
(no path jail, no sandbox, no long-lived shell, no bundled grep fallback, no
mid-stream resume).

## What the vendored opencode/ told us

`find opencode/packages/opencode/src -maxdepth 2 -type d` mapped the peer's
surface. The directories that exist in opencode but not in rocky: `mcp/`,
`lsp/`, `worktree/`, `question/`, `skill/`, `plugin/`, `account/`, `ide/`,
`effect-drizzle-sqlite/`, etc. Each is a concrete parity candidate.

After C/D/M categorization:
- `mcp/`, `plugin/`, `ide/` → **(C)ut** — DESIGN.md milestone 5 says no.
- `lsp/`, `worktree/`, `skill/` → **(M)issing** — not cut, not deferred, real gap.
- `effect-drizzle-sqlite/` → speculative; the user runs single-process, no need.

This gave §3.B its concrete anchors (file paths in opencode as references to
model on).

## What the bench told us

`bench/run.ts` ran 4/9 → 7/9 on a coding-perf pass. Remaining 2 failures are a
*model* failure (the do-nothing mode), not a loop failure. §6 named which of
the proposed gaps actually moves that metric — only skills (B3) had a clear
bench impact, because a `bun-bench` skill would directly attack the do-nothing
mode. The other gaps (A1, B1, B2, B4) move reliability or DX, not bench.

## Anti-list (the "looks like a gap but isn't" table)

Captured 5 items the user would expect a casual reviewer to flag:
- "No images in the prompt" — actually fine for a CLI coding agent; add later.
- "No `--print` JSON output" — useful but skip unless scripting use case.
- "No test coverage report" — CI concern, not a code one.
- "No `webfetch` tool" — the tool exists; read before flagging.
- "No model X" — list of unsupported models; each is a provider capability, not
  a code gap.

This section earned its keep: a reader who doesn't see it will ask "but why
no images?" in the reply.

## What I did NOT do, and the right call

- **Did not delegate to OpenCode/Claude Code.** The bottleneck was synthesis
  from DESIGN.md intent, not multi-file code surgery. Delegating would have
  lost the categorisation discipline.
- **Did not propose re-adding MCP / hooks / /undo** — DESIGN.md milestone 5 is
  explicit. Proposing them would signal I hadn't read the project.
- **Did not produce a feature wishlist.** Each of the ~13 items in §3 is
  anchored to a file path with an effort estimate.

## Effort totals

Total real gaps identified: ~13 (3 architect-admission + 4 SOTA peer + 3 ops +
3 small/misc). Of those, ~5 are 1-hour-to-half-day wins; ~4 are 1–3 day
projects; ~3 are 1–2 week projects. The priority ordering puts the 1-hour item
second, because it's the cheapest ship-it.

## Lesson for next gap analysis

The "anchor every recommendation to a file path" rule is what makes the
deliverable *useful*. Without anchors, the user gets a feature wishlist they
have to do the work of locating in their own codebase. With anchors, they
have a roadmap they can hand to a subagent and say "go."