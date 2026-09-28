---
name: daily-memory-backfill
description: Fill Daily Memory stubs from session-history helper output.
platforms: [linux, macos, windows]
tags: [obsidian, journal, daily-memory, backfill, wikilinks]
metadata:
  hermes:
    tags: [Obsidian, Journal, Backfill, Wikilinks, DailyMemory]
    related: [obsidian, bulk-vault-enrichment]
---

# Daily Memory Backfill

Use this skill when a dated daily-memory note (`Daily Memory/<YYYY>/<YYYY>-MM-DD.md`) needs to be filled with the actual session log for that day, replacing a stub the user pre-creates with frontmatter and empty section headers.

This is a recurring workflow in this vault: a parent dispatcher pre-creates the stub and per-day subagents fill each one. The hard part is not the structure (the schema is fixed by the user's instructions) — it's the discipline around **byte budget** and **not inventing**.

## Workflow (verified 2026-07-17)

1. **Get the session data.** Run the user's helper script first:
   ```bash
   python3 "<vault>/Daily Memory/scripts/daily_memory_helper.py" YYYY-MM-DD
   ```
   The script returns a human-readable dump of all sessions that day (titles, sources, first/last user messages, msg/tool/token counts). Use this verbatim; don't re-query the session DB.

2. **Read the stub.** Open `<vault>/Daily Memory/<year>/<year>-MM-DD.md` with `read_file`. The stub has frontmatter (do NOT change) plus fixed section headers (`## At a glance`, `## What I worked on`, `## Decisions / outcomes`, `## Sessions today`, `## People`, `## Projects touched`, `## Concepts / tools`, `## See also`). Some sections may say `*(pending backfill)*`.

3. **Use the source counts the parent computed.** The parent typically runs a diagnostic before dispatch (e.g. "cli=1, desktop=6, subagent=18"). Put those exact numbers in the **Sources** line — do NOT re-derive them from the helper output, because subagent counts can differ between the parent's diagnostic moment and the helper's view.

4. **Verify the frontmatter is intact before saving.** Any parent typically expects type/tags/provenance/created fields unchanged. Verify with `head -12` after writing.

5. **Fill the sections in the schema's order.** Don't reorder. The `See also` block must contain the two anchor links the user's MOC pattern expects.

6. **Save by overwriting** the stub path (the file already exists and the user wants it replaced, not appended-to).

7. **Verify post-write:**
   - `wc -c` to confirm byte count is in the user's target band (typically 1.5 KB – 5 KB for a single day).
   - `head -12` to confirm frontmatter is intact.
   - If over budget, trim the cheapest prose first (one-liner summary, parenthetical repo lists), NOT the sessions table.

## Critical pitfalls

### Byte budget is binding — sessions table alone uses ~3.5 KB

The `Sessions today` table has one row per session. With 25 sessions, just the `| Time (IST) | Source | Session | Title | Link |` rows consume ~3.5 KB of the 5 KB cap. That leaves only ~1.5 KB for all other sections combined. Plan for this:

- Keep one-liner summaries tight (no clauses you can move).
- Trim bullet lists with verbs you can compress ("checked and updated" → "check + update" works only if the user is OK with terser phrasing; otherwise condense by removing fill words like "explicitly", "specifically").
- Use a representative sample + "and more" in long parenthetical enumerations (e.g. 5–8 repo names from a 55-repo batch).
- The `(none named in session titles or first user messages)` People line should be trimmed to the shortest form that still reads natural.

### Don't invent people, projects, or wikilinks

- **People:** only list someone if their name appears in a captured session title or first user message. Otherwise write `(none named in session titles or first user messages)` — that is the correct, non-invented answer.
- **Wikilinks:** only link notes that exist in the vault. Before adding a `[[...]]` link, verify the target note actually exists with `search_files`. If you don't know whether a note exists, prefer a bare phrase over a wikilink that would be dead on graph view.
- **Quotes/numbers:** don't quote token counts, msg counts, etc. that aren't in the helper output's provenance.

### Mirroring the user's link convention is non-negotiable

The user's vault uses **relative-path wikilinks with aliases** for its MOC navigation (`[[../Daily Memory|Daily Memory]]`, `[[../../Me|Me]]`), and **folder-path wikilinks with display aliases** for topic-area MOCs (`[[Projects/Projects|the Projects hub]]`, `[[Github/github|the GitHub index]]`).

Before writing the note, glance at an already-filled same-day file (or the `Daily Memory/Daily Memory.md` MOC) to confirm the exact phrases. Style drift here breaks the graph view's `[[...]]` parsing and the join with the MOC. Wrong:
- `[[Me]]` alone (the user's style uses the relative-path-with-alias form)
- `[[Projects hub]]` (the actual note is `Projects/Projects.md`, not `Projects hub.md`)

### Don't touch anything outside the target file

- Don't run `build_index.py --check` — the parent does that after the batch.
- Don't edit `_template.md` or the MOC (`Daily Memory.md`).
- Don't touch any other dated file, even if it has the same problem.

### When session data is thin, write a thin note

If the helper returns only 2 sessions with short messages, write a 1.5 KB note. Don't pad to look impressive. The schema is fixed; the content doesn't have to fill it.

## Schema (canonical, as of 2026-07-17)

```
## At a glance
- Sessions: N
- Messages: N
- Sources: cli=N, desktop=N, subagent=N
- One-line summary: <real one-liner, never placeholder>

## What I worked on
- 3-8 bullets drawn from session titles + first user messages

## Decisions / outcomes
- durable artifacts (notes written, repos cloned, configs changed) — only items evidenced by session data

## Sessions today
| Time (IST) | Source | Session | Title | Link |
|---|---|---|---|---|
| ... | cli/desktop/subagent | <id> | <title> | @session:default/<id> |

## People
- (none named in session titles or first user messages)

## Projects touched
- [[<path>|<alias>]] — reason

## Concepts / tools
- bare phrases or [[wikilinks]]

## See also
- [[../Daily Memory|Daily Memory]] — the MOC
- [[../../Me|Me]] — root MOC
```

The `Link` column must use the exact `default/<session_id>` format with the `@session:` prefix — Hermes renders this as a clickable link to the original session.

## Quick-decision table

| Symptom | Fix |
|---|---|
| `wc -c` shows >5 KB after first write | Trim prose, never the sessions table. Cheapest targets: one-line summary, repo enumerations, "(none...)" People line. |
| Session has no title | Use literal `(no title)` in the table — this is correct, not invented. |
| Subagent count in helper differs from parent's diagnostic | Trust the parent's counts (they're usually pre-frozen). |
| Title contains repo paths / file paths | Reproduce verbatim — don't re-flow. |
| Wikilink target doesn't exist in vault | Use a bare phrase instead of inventing `[[...]]`. |
