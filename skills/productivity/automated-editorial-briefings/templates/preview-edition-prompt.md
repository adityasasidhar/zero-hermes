# Preview Edition — Hermes Times

> **Use when:** The kid asks for a draft to vet before tomorrow's real run. Triggers include *"issue it now so I can check"*, *"before you put it in tomorrow morning"*, *"I want to read it first"*, *"give me a preview"*, *"what would tomorrow's paper look like?"*.
>
> **Do not use for:** the regular morning run. The preview is *not* the morning paper — see Patches below.

## What this is

A full visual layout of tomorrow's HERMES TIMES, archived to disk, displayed in the preview pane, *not* scheduled, *not* sent, *not* QA-passed. The kid reads it, edits the layout / lead / sources, and only then authorizes the regular cron. One cycle. No cron registration. No Telegram delivery. No visual QA rerun.

## Workflow

1. **Date stamp.** `date` first. Edition date = today (or tomorrow if the kid says "tomorrow morning"). Filename slug: `YYYY-MM-DD-v1`.

2. **Survey the desk.** In parallel:
   - `ls /home/arctic/projects/` for active repos
   - `git log --oneline --since="5 days ago"` across all repos
   - `gh api user/repos?sort=pushed&per_page=10` for recent pushes
   - `find ~/Documents/fun -name '*.md' -mtime -7 -ls` for vault activity
   - `date` for the masthead
   
   *Do not ask the kid for an agenda.* The kid is asking *for* the agenda. Picking the lead is the agent's job. If the desk is genuinely empty, ship a one-page "nothing-burger" — that's still a paper.

3. **Pick the lead.** Highest-signal item with primary sources on disk or in `gh`:
   - a repo with a fresh commit and a clean README → build profile
   - a vault sub-tree rebuilt today → feature on the routines / pages
   - an arXiv topic the kid named → research page lead
   
   *Do not fabricate a lead.* If two candidates are equally weighted, lead with the one that has more receipts.

4. **Pattern B architecture, single-pass.** No sub-agent fan-out. No correspondent voices. Single editor writes the four pages. Preview edition is shorter on coverage (you don't need a full briefing desk) but should still hit the four-page mark or ship as a deliberately shorter edition (1, 2, or 4 pages — never pad).

5. **Render.** Single HTML file, inline CSS, no external JS. ASCII architecture diagrams for any "art" panels (text-free, no fake glyphs, no logos). Foot the masthead with `J. JAMESON, masthead editor · THE HERMES TIMES · preview edition · v1` and the file path footer with `Presses: HOLDING pending kid's read`.

6. **Archive.** Write to `~/Documents/fun/HermesTimes/<DATE>-v1/ed.html`. Create the directory if it does not exist.

7. **Deliver.** Display in the preview pane via `open_preview(url='file:///home/arctic/Documents/fun/HermesTimes/<DATE>-v1/ed.html', label='THE HERMES TIMES · Preview Edition · <DATE>')`. Then in chat, paste the full four pages as a single readable artifact: front-page lede first, then pages 2–4 in order, in the paper's editorial voice.

8. **Stop.** Do not run `hermes cron create`. Do not call `hermes send`. Do not run vision QA. Do not iterate the layout unless the kid asks for a fix. The preview's job is to be *read*, not to be *shipped*.

## 404 discipline

GitHub will return `Not Found` on some recent push candidates (private repos, deleted repos, name typos). When this happens:

- Do **not** invent the repo's content. Spike the story.
- File it under a "404s · spiked" panel: *"GitHub returned 404 on four repos the kid has shipped in the last seven days. Either private, or deleted. No copy on this story — page gets spiked rather than fabricated."*
- Move on. Don't loop, don't retry `gh api` immediately — just note and continue.

A paper with three filled briefing slots + one spiked 404 panel is more honest than a paper with four fabricated blurbs.

## Pitfalls (specific to preview-mode)

- **Treating the preview as the morning paper.** No cron, no Telegram, no vision QA. The kid will edit the layout or re-pick the lead; running final QA before they read is wasted compute and locks the format prematurely.
- **Asking which topic to cover.** Survey the desk, pick the lead from the evidence, file the copy. Intake happens after, not before.
- **Asking which desks to include.** Default to all four (front / briefing / research / systems), or a deliberately shorter edition if the desk is thin. Never a mid-size "three of four with one swapped."
- **Rendering via headless Chromium / PDF.** Preview edition is HTML only. Skip the PDF + PNG pipeline. Telegram gets a PDF tomorrow; the preview gets a `file://` URL.
- **Skipping the footer.** "Presses: HOLDING pending kid's read" in the footer is what makes it a preview and not a final. Forgetting it makes the kid think this is the morning paper.
- **Padding to four pages when the desk is thin.** A two-page preview with a strong lede and a tightly written briefing is better than a four-page preview with filler. The kid will notice filler and never want to schedule the cron.
- **Not archiving.** Always write to `Documents/fun/HermesTimes/<DATE>-v1/`. The kid's tomorrow-morning paper is built on yesterday's preview. The archive is continuity.

## Output contract

A preview edition must produce:

- `~/Documents/fun/HermesTimes/<DATE>-v1/ed.html` — the rendered HTML, single file, inline CSS.
- One chat reply, in J. JAMESON voice, with the full paper reproduced as a readable Markdown artifact (not just a link — the kid reads on Telegram).
- The preview pane opened, label = "THE HERMES TIMES · Preview Edition · <DATE>".

No cron entry. No Telegram attachment. No final QA. The "Hold for the kid's read" footer is the contract.

## Hand-off

When the kid approves after the preview, the next session (or this one, if they say so) runs the **real** edition:

1. Apply edits from the preview feedback.
2. Bump the archive filename: `<DATE>-v2/ed.html` if layout changed, or `<DATE>-v1/ed.html` + the regular morning-run path if not.
3. Switch to the orchestrator pattern (`templates/orchestrator-prompt.md`) for the real run: 4 fixed beats, 4–8 leaf correspondents in parallel, real charts, manifest schema, vision QA, Telegram delivery.
4. Register the cron with `hermes cron create "30 5 * * *" "<prompt>" --name "hermes-times-morning" --skill automated-editorial-briefings --deliver telegram`.

The preview is the spec. The real run executes the spec the kid signed off on.
