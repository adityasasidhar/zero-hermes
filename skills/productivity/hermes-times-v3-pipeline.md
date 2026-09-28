---
name: hermes-times-v3-pipeline
description: "Continue building THE HERMES TIMES v3 — the agent-driven newspaper that supersedes hermes_times_v2.py. Use when iterating on the renderer, chart helpers, orchestrator prompt, or Jameson personality wiring."
version: 0.1.0
author: Hermes Agent
license: MIT
platforms: [linux]
metadata:
  hermes:
    tags: [newspaper, jameson, cron, agent-orchestration, editorial]
    related: [automated-editorial-briefings]
---

# HERMES TIMES v3 — Pipeline

Status: **paused**. v2 deterministic script (`d5cf6c040d3e`) is the
production cron. v3 needs orchestrator hardening before resuming.

## What v3 is

Agent-driven version of THE HERMES TIMES. The editor *is* the agent
(J. Jameson personality). The agent decides the structure each issue
based on the news, spawns correspondent sub-agents, calls chart
helpers, generates art, and renders. Cron entry
`01530de3ca1f` (paused).

## What's built (verified)

- `~/.hermes/scripts/hermes-times-v3/render.py` — manifest → HTML → PDF → PNG
- `~/.hermes/scripts/hermes-times-v3/style.css` — CSS component library
- `~/.hermes/scripts/hermes-times-v3/charts.py` — 5 chart helpers (bar, horizontal_bar, timeline, heatmap, scatter_with_labels)
- `~/.hermes/scripts/hermes-times-v3/MANIFEST_SCHEMA.md` — agent↔press contract
- `~/.hermes/scripts/hermes-times-v3/prompts/orchestrator.md` — editor brief
- `~/.hermes/scripts/hermes-times-v3/prompts/correspondent.md` — beat templates
- `~/.hermes/scripts/hermes-times-v3/cron_jameson.py` — wrapper script
- `~/.hermes/scripts/hermes-times-v3/.venv` — Python 3.13 with matplotlib + pillow
- `agent.personalities.jameson` — the editor's voice (3 selectors not flipped; default chat remains `professor`)

## What's verified end-to-end

- Smoke test (`/tmp/v3-smoke.py`) renders a 2-page fixture with cover + chart + cards + pull quote + closing quote. PDF + PNG produced, gates pass.
- Visual QA on cover — clean, all elements aligned.
- Visual QA on interior page — chart title render fixed (single, not duplicated), layout flex fixed (closing quote no longer overlaps cards).
- Cron's `python3 .venv/bin/python` invocation works with `env -u PYTHONPATH` (the hermes-agent venv pollutes the default PYTHONPATH).

## What's broken / not verified

- **Agent orchestrator run hangs.** First force-run at 2026-07-25 15:03 IST was still "running" after 8+ minutes with no output files, no manifest, no spawned sub-agents. Likely stuck in a slow tool call (mmx image generation? a blocking web call?). Needs hard timeouts.
- **No visual QA on the agent-produced issue yet** — see above.

## Cron state

- v2 (deterministic): `d5cf6c040d3e` — **active**, 08:00 IST daily
- v3 (agent): `01530de3ca1f` — **paused**, was 05:30 IST

## Hardening checklist (next session)

1. Add per-tool-call timeouts in the orchestrator prompt (e.g. `mmx image generate` capped at 60s, web fetches capped at 20s).
2. Add a "fast path" fallback: if the news is sparse OR the agent's been running >15 min, ship a 2-page paper instead of 4.
3. Add a hard `max-iterations` cap on the agent loop (cron-wise: model the agent in `hermes chat --max-turns 30`).
4. Add a `/tmp/manifest-PREFIX.json` heartbeat — the agent writes a partial manifest every iteration so we can see progress.
5. Re-test force-run with `--accept-hooks` and 600s timeout.
6. Once force-run produces a paper, do visual QA, fix issues, then resume the cron job.

## Continuity

- v2 paper issues live at `~/.hermes/data/hermes-times/issues/`
- v3 paper issues live at `~/.hermes/data/hermes-times-v3/issues/`
- Yesterday's paper is the working memory for tomorrow's — orchestrator reads the most recent `.html` + `.png` from the v3 dir. Until v3 ships its first issue, the orchestrator can fall back to v2's most recent issue for continuity.

## Pitfalls (already documented)

- `PYTHONPATH` from `hermes-agent` venv conflicts with the v3 `.venv`. Always `env -u PYTHONPATH ~/.hermes/scripts/hermes-times-v3/.venv/bin/python …` for charts and render.
- Config.yaml patch attempts can corrupt the file. The auto-backup at `~/.hermes/config.yaml.corrupt.<timestamp>.bak` is the recovery artifact. The current `jameson` preset is a single-line escaped string (JSON-ish), not a block scalar — it parses but isn't pretty.
- Jameson personality is in the preset dict but the three selectors (`agent.personality`, `personality`, `display.personality`) are intentionally set to `professor`. Aditya does not want Jameson as the default chat voice — only for the editorial briefings.
