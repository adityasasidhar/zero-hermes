# <Project Name> — Gap Analysis

> Generated YYYY-MM-DD from a static read of <list of dirs>. Nothing was run.

> Two-column rule: **(P) = present / wired**, **(A) = absent or deliberately cut/deferred**.
> Where the cut is already noted in <DESIGN.md / AGENTS.md>, that's called out — no point
> re-proposing what was decided.

---

## 0. One-line summary

One or two sentences naming the project's bet, where the loop is tight, and where the
real openings are. Don't repeat the README.

---

## 1. Inventory of what's already wired

| Capability | Status | Notes |
|---|---|---|
| ... | **(P)** | file path + 1-line description |
| ... | **(P)** | ... |
| ... | **(P)** | ... |

Cover the major subsystems: loop, providers, tools, permissions, TUI, observability, tests,
bench. ~15–25 rows. Tables only — no prose here.

---

## 2. What's deliberately cut or deferred (do NOT re-propose)

State these in a separate table. Each row: feature, where the decision lives (file +
section), and the stated reason.

| Feature | Where | Why |
|---|---|---|
| ... | `DESIGN.md` milestone N | "Cut by user decision" / "Deferred — milestone N+1" / "Tradeoff: ..." |
| ... | `AGENTS.md` gotchas | ... |
| ... | `DESIGN.md` "Tradeoffs taken" | ... |

Re-surfacing these in §3 is busywork.

---

## 3. The real gaps

### A. Architect's own admission (read `DESIGN.md` "What I'd do differently" first)

For each:

- **What**: one-sentence summary
- **Why it matters**: impact on users / correctness / leverage
- **How**: concrete plan with file paths
- **Effort**: 1 hour / half-day / 1 day / 1 week / 1–2 weeks

### B. SOTA features absent that peers ship

For each:

- **What**: the feature
- **Why it matters**: real user-visible win
- **Cost**: effort + dependencies
- **SOTA reality check**: split / consensus / one-sided across Claude Code / OpenCode /
  Cursor / Codex CLI

### C. Operational / observability gaps

Often small but high leverage: session telemetry, cost/latency budgets, structured logs,
etc.

---

## 4. Priority ordering

If you only have one burn-window, here's what to do in order:

1. **A1** — ... (effort, why)
2. **A2** — ...
3. ...

---

## 5. Things that look like gaps but aren't

The "defuse bad proposals" section. Each item: the surface-level feature someone would
flag, why it's actually deliberate. 5–10 items.

---

## 6. Bench impact (if applicable)

If the audited project has a benchmark / eval / metric, separate "moves the metric"
from "doesn't move the metric but is otherwise good." This is where the user often has
strongest priors.