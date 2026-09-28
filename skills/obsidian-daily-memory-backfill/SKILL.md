---
name: obsidian-daily-memory-backfill
description: Backfill a Daily Memory note from Hermes session history.
---

# Daily Memory Backfill

Class: turn a templated Daily Memory stub into a real, populated day note from the Hermes session DB, without modifying any other file in the vault.

## When to use

Trigger when the user asks to backfill, fill, restore, regenerate, or "do" a `Daily Memory/YYYY/YYYY-MM-DD.md` for a past date. The day-note subsystem lives at `/home/arctic/Documents/fun/Daily Memory/` and the per-day stub headers are fixed (frontmatter + `# YYYY-MM-DD` + H2 sections). Do not use this skill for genuinely new-day journaling (live capture) — only for backfills from session history.

## Workflow

1. **Run the helper script** to get all session data for the day:
   ```bash
   python3 "/home/arctic/Documents/fun/Daily Memory/scripts/daily_memory_helper.py" YYYY-MM-DD
   ```
   Output: total session + message counts, then a block per session with title, source, message count, and first/last user messages.

2. **Read the existing stub** at `/home/arctic/Documents/fun/Daily Memory/YYYY/YYYY-MM-DD.md`. The stub preserves frontmatter (`type: DailyMemory`, `tags`, `provenance`, `created`) and a fixed H2 skeleton: `At a glance`, `What I worked on`, `Decisions / outcomes`, `Sessions today`, `People`, `Projects touched`, `Concepts / tools`, `See also`. Do **not** touch the frontmatter.

3. **Fill the sections** in order:
   - **At a glance:** sessions / messages / sources counts + one-line summary (combine the day's themes into ~20 words).
   - **What I worked on:** 3–8 bullets, each anchored to a real session title in parens. Never invent.
   - **Decisions / outcomes:** durable artifacts only — policy changes, deletions, configuration changes, deferrals. Not session summaries.
   - **Sessions today:** Markdown table with columns `Time (IST) | Source | Title | Link` — the `Link` cell is the `@session:default/<id>` rendered by Hermes. Use the source order from the helper output (already chronologically sorted).
   - **People:** any person who appeared in a session (e.g. roleplay personalities). Link to the personality note (`[[wiki/personal/index|wiki/personal]]`).
   - **Projects touched:** bullet list of project names with wikilinks; for top-level MOC pointers use `[[Me|Me]]` and `[[Concepts/Concepts|Concepts hub]]`.
   - **Concepts / tools:** short bullet list of named concepts/techniques touched.
   - **See also:** `[[../Daily Memory|Daily Memory]]` + `[[../../Me|Me]]`. These are present in the stub; keep them.

4. **Save** to the same path, overwriting the stub.

5. **Verify before finishing:**
   - `stat -c '%s' <path>` — byte size must be in **1.5–5KB** (1536–5120 bytes). This is a hard gate. See "Size budget" below for the trim path.
   - `head -12 <path>` — must show YAML frontmatter intact between `---` lines, then `# YYYY-MM-DD`. If the closing `---` is missing or frontmatter fields changed, fix it before reporting.

## Hard rules (from the user)

- **Do not fabricate.** Any claim in a section must trace back to a session in the helper output. If you can't find evidence, drop the bullet.
- **Do not modify other files.** Only the target `YYYY-MM-DD.md` is touched. No `build_index.py`, no edits to neighbors, no rewrites of `Me.md` or the MOC.
- **Do not run `build_index.py`** or any other indexer — that is explicitly excluded.
- **Preserve frontmatter** exactly. The `type`, `tags`, `provenance`, and `created` lines are fixed for the day.

## Size budget (1.5–5KB) — the trim path

The naive fill — full 20-row table with `Session` id column, each title spelled out, long bullet prose — produces ~5.1–6.2KB. You will be over the ceiling. Trim in this order, each step ≈ 20–80 bytes:

1. **Drop the `Session` id column from the table.** The id is already embedded in the `@session:default/<id>` link, so the column is redundant. Renames `Time | Source | Session | Title | Link` to `Time | Source | Title | Link` — saves ~30 bytes per row × 20 rows.
2. **Drop "(IST)" from the Time column header** once you've decided the time column is just HH:MM. Saves ~5 bytes.
3. **Shorten titles** in the table — "Running Paperclip AI Server" → keep, "Make Zen the default browser" → "Make Zen the default browser" (already short). For subagents, shorten "(untitled — beat description)" to just "**Beat name** beat". Don't shorten past recognition.
4. **Collapse "What I worked on" bullets.** Adjacent bullets that flowed together in the session ("Looked up...and confirmed the local install. Populated X via subagent and switched default browser") can be merged into one bullet ("Looked up X; confirmed Y. Populated Z via subagent; switched default browser to W.") — typically 2 bullets → 1, saves ~200 bytes.
5. **Trim wordy phrases** in Decisions / outcomes: drop articles and "was" passives ("All Hermes Times cron jobs were removed" → "All Hermes Times cron jobs removed"; "The cron run `hermes-times-v4` fired once..." → "`hermes-times-v4` fired once...").
6. **Shorten People** section: "personality in the `Roleplay with J. Jameson` session. Notes: [[wiki/personal/index|wiki/personal]]." → "personality in the `Roleplay with J. Jameson` session. Notes: [[wiki/personal/index|wiki/personal]]" (drop trailing period).
7. **Projects touched:** "— top-level MOC for the day" → " (top-level MOC)" — saves bytes.

After each trim cycle, re-check `stat -c '%s'`. Stop when size is in range. If you end up under 1.5KB, you've probably dropped a section — put it back.

## Pitfalls

- **The header row of the table is `Time (IST) | Source | Title | Link` — the `Link` column is the only place the session id appears.** Don't omit the @session link; Hermes renders it as a clickable session reference.
- **Source distribution must be accurate.** The user often specifies the breakdown (e.g. "7 desktop + 11 subagent + 1 telegram + 1 cron"). Count from the helper output, not from memory.
- **The `People` section is optional but required if a personality/roleplay session exists.** Link to [[wiki/personal/index|wiki/personal]] for personality notes.
- **Do not retrofit frontmatter onto adjacent notes.** The vault has inconsistent frontmatter — only the day-note in question has it.
- **The 2026/ path is fixed.** Do not move the file to `2026-07/` or split by month.
- **The "Projects touched" wikilinks use Obsidian syntax** (`[[Note|Display]]`), not Markdown relative links. `[[Concepts/Concepts|Concepts hub]]` style is correct for cross-folder MOC pointers.

## Verification

After saving:

```bash
stat -c '%s' "/home/arctic/Documents/fun/Daily Memory/YYYY/YYYY-MM-DD.md"
head -12 "/home/arctic/Documents/fun/Daily Memory/YYYY/YYYY-MM-DD.md"
```

Both must pass: byte size in 1536–5120, and frontmatter intact with closing `---` on line 9 followed by `# YYYY-MM-DD` on line 11.

## See also

- scripts/daily_memory_helper.py — the canonical session-data source for any backfill.
- The Daily Memory MOC at `Daily Memory/Daily Memory.md` and root MOC `Me.md` — linked from each note's `See also` section.
