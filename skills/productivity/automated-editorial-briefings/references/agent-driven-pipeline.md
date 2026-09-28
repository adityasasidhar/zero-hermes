# Agent-Driven Editorial Pipeline (Pattern B)

The orchestrator-agent pattern for personal newspapers where the *agent* — not a script — decides structure, sections, visuals, and correspondent voices.

## When to use

- The user wants the agent to "do the whole thing" / "decide based on the news" / "not be deterministic".
- The brief calls for **real charts** generated from the day's data (matplotlib, not decorative art).
- Sections should be optional, mergeable, or invented by the agent.
- The user wants correspondent sub-agents writing individual articles in their own voice.
- Continuity from yesterday's issue matters (e.g. "yesterday's lead was X, today contrast with Y").

## Architecture

```text
cron (no_agent=false, deliver=local, prompt-based)
  -> orchestrator agent (orchestrator role)
       -> read previous issue (HTML/PDF) for continuity
       -> run parallel collectors (RSS, arXiv, gh, vault, sessions, GitHub contribs)
       -> DECIDE today's structure:
            page count, sections, whether charts warranted (and which),
            whether image-art is needed (and where)
       -> spawn 4-8 leaf correspondent sub-agents in parallel
       -> collect articles, write manifest.json
       -> generate any charts (matplotlib inline, no API)
       -> generate any art (mmx image generate)
       -> render HTML via thin render.py -> Chrome -> PDF -> PNG
       -> vision-QA the rendered pages, re-render if needed
       -> hermes send attachments
```

`render.py` from the deterministic pipeline becomes a thin composer: it takes a manifest + a CSS-component library and produces HTML. All editorial judgment moves to the agent.

## Correspondent sub-agents

Each correspondent gets a **beat**, a **source pack slice**, and a **strict output contract**. Spawned in parallel via `delegate_task([])` (same fan-out pattern used for vault enrichment).

### Beat templates

```text
TO: research-correspondent (leaf)
IN:  source pack slice + yesterday's lead + this issue's theme
OUT: 1 markdown article -> /tmp/issue-YYYY-MM-DD/research.md
     schema: { title, dek, body, sources[], chart_request? }

TO: chart-correspondent (leaf)
IN:  data CSV/JSON bundle from orchestrator
OUT: 1-3 PNGs -> /tmp/issue-YYYY-MM-DD/charts/
     schema: { path, title, caption, data_source }

TO: model-correspondent / systems-correspondent / personal-correspondent
    (one per beat, each leaf)
```

The orchestrator's editor pass runs *after* collection — it reads all correspondent articles, decides the layout, and writes the final manifest.

## Manifest schema

```json
{
  "issue_date": "2026-07-26",
  "theme": "...",
  "pages": [
    {
      "page_number": 1,
      "role": "cover",
      "articles": [
        { "id": "lead", "source": ".../lead.md", "layout": "hero" }
      ]
    },
    {
      "page_number": 2,
      "role": "briefing",
      "articles": [
        { "id": "brief-1", "source": ".../brief-1.md", "layout": "card" },
        { "id": "brief-2", "source": ".../brief-2.md", "layout": "card" }
      ],
      "charts": [
        { "path": "/tmp/issue-.../charts/foo.png", "caption": "..." }
      ]
    }
  ]
}
```

`render.py` reads this, walks the pages, applies the matching CSS component for each `layout` value, and writes HTML.

## Chart triggers (concrete examples)

The orchestrator should generate charts **only when the news warrants them** — not on every issue. Examples:

| Trigger in source pack | Chart |
|---|---|
| Multi-paper arXiv day on transformers | Bar: paper count by sub-field this week |
| Several agent releases in one day | Timeline: release dates of watched frameworks |
| User's own work-heavy day | Heatmap: commits by hour + repo, last 7 days |
| Model pricing/market news | Comparison: $/M tokens across providers |
| Personal-work ledger is empty | Skip — no chart forced |
| Single big lead, no clusters | Word cloud of all titles today |

Generated with matplotlib inline, saved as PNG, embedded by manifest reference. No API call — local Python.

## Continuity from yesterday

The orchestrator's first tool call should be reading the previous issue's HTML (or PDF if HTML is too stale). This enables:

- "Yesterday's lead was X — today's lead is the natural follow-up."
- "Two papers on the same topic ran in consecutive days; treat them as a series."
- "You haven't run a personal-work day in a week — today's theme is your own build."

Without this, the agent re-decides structure from scratch each day and the paper feels like a series of one-offs.

## Risks

1. **Cron 3-min limit.** Agent-driven generation is slow. Either extend cron budget, run at 06:00 instead of 08:00, or accept partial issues with scheduled retries. Current 47s deterministic dry-run is fine; agent version will likely be 2-4 min.
2. **Sub-agent cost.** Each correspondent is a full LLM turn. 6 correspondents ≈ 6× the editorial cost. Worth a "minimum viable" mode (3 correspondents) for low-news days.
3. **Visual QA surface.** Agent-rendered HTML is more variable than templated → more bug surface. Need a vision-based QA loop the agent runs on its own output before delivering.
4. **Article quality variance.** Correspondents won't write as tightly as a single editorial pass. Need a strong beat-brief template + an editor pass the orchestrator does after collecting.

## Implementation order

| Step | What | Time |
|---|---|---|
| 0 | Pause today's cron job (per skill rules) | 30s |
| 1 | `render.py` — manifest -> HTML/PDF/PNG with CSS library | 1h |
| 2 | `STYLE.md` — CSS component library (cards, hero, chart-frame, image-strip, etc.) | 30m |
| 3 | Correspondent beat templates (prompts + JSON contracts) | 45m |
| 4 | Orchestrator agent prompt (the cron entry) | 45m |
| 5 | First dry-run, visual QA, iterate | 1h |
| 6 | Force-run, swap cron entry, resume | 15m |

Total: ~4 hours of focused work for a working agent-driven edition.
