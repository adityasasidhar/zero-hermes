# THE HERMES TIMES — v3 architecture (Pattern B working example)

> **⚠️ Historical reference — superseded by v4.** v3 hung on its first
> force-run (8+ min, no output) because the orchestrator prompt was
> monolithic with no per-tool timeouts. **See
> `references/hermes-times-v4-architecture.md` for the active Pattern B
> implementation** that fixed v3's failures: 4 parallel beat reporters
> in one batch, hybrid real-photo + generated image strategy, 30-min
> cron budget with `deliver=telegram`, manifest schema first.
>
> This v3 file is preserved for historical context (what was tried,
> what broke, what carried forward). Do not use as a template.

---

A concrete implementation reference for the agent-driven editorial mode.
Built for Aditya's daily AI/ML briefing in New Delhi. The v3 build at
`~/.hermes/scripts/hermes-times-v3/` is paused; the v2 deterministic
script at `~/.hermes/scripts/hermes_times_v2.py` is the production
paper. This file documents the v3 design + state so the next iteration
can pick up cleanly.

## File layout

```
~/.hermes/scripts/hermes-times-v3/
├── style.css              # CSS component library (cover, cards, charts, quote)
├── charts.py              # matplotlib helpers (bar, horizontal_bar, timeline, heatmap, scatter_with_labels)
├── render.py              # manifest -> HTML -> Chrome-PDF -> PNG. Pure press.
├── MANIFEST_SCHEMA.md     # the contract between the agent and the press
├── prompts/
│   ├── orchestrator.md    # the cron-entry brief. Reads MANIFEST_SCHEMA, outputs a manifest.
│   └── correspondent.md   # template for one beat — news / research / systems / personal / chart
├── cron_jameson.py        # wrapper that injects today's date into orchestrator.md and calls hermes chat
└── .venv/                 # clean Python 3.13 venv with matplotlib + pillow

~/.hermes/data/hermes-times-v3/
├── issues/                # archive of past issues (HTML + PDF + PNG)
├── assets/<date>/         # generated art and chart PNGs, per issue
└── state.json             # seen URLs for dedup

~/.hermes/config.yaml
└── agent.personalities.jameson  # the editorial voice (used in chat via /personality jameson)
```

## Components

### 1. The press (`render.py`)

Pure transformation. Reads a manifest JSON, writes HTML → Chrome-PDF → PNG.
No editorial judgment, no LLM calls. Gates:

- `pdfinfo` page count is informational (the manifest designer's chosen page count is the contract)
- artifact size gates (PDF ≥ 50 KB, PNG ≥ 30 KB)

```python
render({"issue_date": "2026-07-26", "issue_number": 207, "theme": "...", "cover": {...}, "sections": [{...}, ...]})
# -> {html, pdf, png, page_count}
```

### 2. Chart helpers (`charts.py`)

Each helper returns `(path, title, caption)`. Visual style: black ink, single accent (`#e36b45`), paper-tan background, no 3D, no gradients. Designed to print at 96 dpi and survive Telegram PNG compression.

```python
from charts import bar, horizontal_bar, timeline, heatmap, scatter_with_labels
chart_path, title, caption = bar(labels, values, "My title", "Source: ...", Path(...))
```

Available types: `bar`, `horizontal_bar`, `timeline`, `heatmap`, `scatter_with_labels`. New types are added by the orchestrator when a story needs a chart the library doesn't have.

### 3. CSS library (`style.css`)

A4 page primitives: `.page`, `.cover`, `.layout`, `.cards`, `.card`, `.paper-card`, `.feature`, `.chart`, `.pull`, `.quote`, `.folio`, `.artwide`, `.artside`, `.editorial-bar`. The agent composes these on demand, not from a fixed template.

Pitfall: do not absolutely-position `.quote` to the page bottom — it overlaps content above. Use flow layout with `margin-bottom: 25mm` to clear the folio.

### 4. Orchestrator (`prompts/orchestrator.md`)

The Jameson brief. Reads `MANIFEST_SCHEMA.md`, reads yesterday's issue, parallel-collects sources, decides structure, spawns 4-8 correspondent sub-agents, calls charts.py, generates art via `mmx image generate`, vision-QAs the rendered pages, delivers via `hermes send`.

Key directives the agent must internalize:
- "Never invent a benchmark, a date, a paper, a person, or a release."
- "Never ship a paper without art on the cover."
- "Charts use real data. If the data doesn't exist, drop the chart."
- "Identity is sacred. The source record owns the truth."

### 5. Correspondent template (`prompts/correspondent.md`)

Each beat — news / research / systems / personal / chart — gets a leaf sub-agent with this brief + their source pack slice. Output is strict JSON with `{source_id, title, body, url, source, date, authors?}` items. The editor rebinds identity fields from the source pack after collection.

### 6. Personality (`jameson` preset)

Editor voice. Defines the editorial standards, the "a beaut / nothing-burger" cadence, the relationship to Aditya. Used in chat via `/personality jameson`. **Do not** set as the default chat personality — Aditya explicitly wants Jameson for the paper only.

## cron wiring

```bash
hermes cron create "30 5 * * *" \
  "You are J. JAMESON, editor of THE HERMES TIMES. Read the orchestrator brief at ~/.hermes/scripts/hermes-times-v3/prompts/orchestrator.md and execute it for today (IST). Press: ~/.hermes/scripts/hermes-times-v3/. Yesterday: ~/.hermes/data/hermes-times-v3/issues/. Read MANIFEST_SCHEMA.md first. Use /home/arctic/.hermes/scripts/hermes-times-v3/.venv/bin/python for charts. Deliver via hermes send to Telegram." \
  --name "hermes-times-v3" \
  --skill automated-editorial-briefings \
  --deliver local
```

Note: schedule and prompt are *positional* — the CLI has no `--prompt` flag. Flags come after the prompt.

## Hardening checklist (next iteration)

The v3 force-run hangs silently for 8+ minutes with no output. The next iteration must add:

1. **Per-tool timeouts in the orchestrator prompt**: `mmx image generate` capped at 60s, web fetches capped at 20s. The agent should `timeout=` its subprocess calls.
2. **Hard `max_turns` cap**: `agent.max_turns: 30` in config.yaml, or `--max-turns 30` on `hermes cron create`. Today the cron has no cap.
3. **Partial-manifest heartbeat**: the agent writes `/tmp/manifest-<date>.json` after each iteration, so a stuck run can be inspected from outside the loop.
4. **Fast-path fallback**: if news is sparse OR elapsed > 15 min, ship a 2-page paper (cover + briefing) instead of 4 pages.
5. **Investigation commands for a stuck run**:
   - `hermes cron runs <job_id>` — show execution status
   - `tail -50 ~/.hermes/logs/gateway.log` — last gateway activity (note: Discord reconnect spam dominates the log; grep for the right thing)
   - `ls -lat ~/.hermes/cache/delegation/live/` — sub-agents spawned
   - `find ~/.hermes/data/hermes-times-v3/ -newer <marker>` — file activity from the agent

## What worked

- ✅ Manifest schema + render.py separation: the renderer is dumb and testable (smoke test in `/tmp/v3-smoke.py` ran in <2s).
- ✅ Chart helpers as pure functions returning `(path, title, caption)` — easy to test, easy to add to.
- ✅ outlet.sql of editorial copy via the orchestrator prompt — agent decides section count + chart use per issue.
- ✅ CSS component library gives the agent visual primitives to compose, not a fixed template.

## What didn't work

- ❌ First agent force-run hung 8+ minutes. The orchestrator prompt is too ambitious; 4-6 sub-agents + image gen + vision QA is too much for the current model timing.
- ❌ Per-issue art is a runtime cost. The v3 brief calls for `mmx image generate` 3-4 times per issue. Image generation is slow, network-flaky, and serial. Consider caching cover art for 7 days keyed by theme + issue_number.
- ❌ Pure-Chrome rendering is fast (`<2s`) but breaks when art URIs are `file://` paths and Chrome's `--no-sandbox` + `--disable-gpu` mode has quirks. The smoke test produced clean PNGs; the v3 first run hasn't validated yet.

## Continuity

The agent reads yesterday's issue from `~/.hermes/data/hermes-times-v3/issues/` on each run. Until v3 ships its first issue, the orchestrator can fall back to the v2 issue at `~/.hermes/data/hermes-times/issues/`.

## Open questions

- Should the manifest include a `cover_art_prompt` field that the agent populates, or should the agent generate art before writing the manifest? (Currently: after, but unverified.)
- Should chart-helper failures abort the issue or skip the chart? (Currently: skip — the orchestrator's "if the data doesn't exist, drop the chart" rule.)
- How do we handle `--deliver telegram` when the agent is also calling `hermes send`? (Resolved in v4: `deliver=telegram` works because the agent's `MEDIA:` send and the scheduler's telegram output carry different payloads — not duplicates.)