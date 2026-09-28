# Proven Newspaper Pipeline Notes

Condensed implementation details from a successful recurring personal-newspaper build. Keep these as an example, not hardcoded universal paths.

## Pipeline shape

A single Python script performed:

1. parallel collection of local sessions, local Git work, GitHub contributions, vault edits, news RSS, arXiv, GitHub Trending, and Open-Meteo;
2. MiniMax-M3 editorial synthesis into strict JSON;
3. deterministic source-integrity repair;
4. optional MiniMax `image-01` art generation with a fallback asset;
5. self-contained HTML rendering;
6. Chrome print-to-PDF;
7. `pdfinfo` exact-one-page gate;
8. `pdftoppm` PNG generation;
9. Telegram delivery of PNG + PDF via `hermes send`;
10. empty stdout on success for `no_agent` cron.

Independent collectors were moved to `ThreadPoolExecutor`; the dry-run dropped to about 47 seconds, leaving room inside a three-minute scheduled execution limit.

## Useful implementation details

### Safe subprocess wrapper

Give every command a timeout, capture stdout/stderr, and construct a cron-safe PATH. Raise with the last stderr lines only when `check=True`.

### Local-day windows

Compute the previous-day window in the user's timezone, then convert timestamps to UTC only for APIs that require it. This avoids including late-night activity in the wrong edition.

### Personal-work evidence hierarchy

1. Git commits: completed and recorded work.
2. Git diff/status: real but uncommitted work; report it explicitly as such.
3. GitHub contribution API: public activity, not total activity.
4. Session titles/counts: topics and effort signals, not deliverables.
5. Vault modification times: evidence of note edits, not proof of substantive content without reading excerpts.

### Integrity repair

Build candidate maps by source class:

```python
arxiv_by_url = {p["url"]: p for p in arxiv_candidates}
repos_by_name = {r["repo"]: r for r in trending_candidates}
```

A paper is accepted only if its URL maps to an arXiv candidate; replace title/authors/summary from that same record. A repository is accepted only if name and URL match the same candidate. Overwrite weather values from the structured API after editorial generation.

### Render gate

After Chrome writes the PDF:

```python
info = subprocess.run(["pdfinfo", pdf], capture_output=True, text=True, check=True).stdout
pages = int(re.search(r"^Pages:\s+(\d+)", info, re.M).group(1))
if pages != 1:
    raise RuntimeError(f"expected one page, got {pages}")
```

Only rasterise after this passes. Also reject unusually small PDF/PNG outputs.

## Vintage broadsheet translation

The supplied reference was a late-1980s student newspaper. The successful translation used:

- centred uppercase high-contrast serif masthead;
- four-cell metadata strip with top/bottom rules;
- enormous italic uppercase lead headline at tight line height;
- three columns, centre roughly 1.8× side widths;
- one boxed overview in the centre;
- a small image floated in the right column, grayscale and high-contrast;
- small, justified serif body copy and centred italic bylines;
- thin vertical and horizontal rules;
- ivory paper plus subtle procedural noise;
- a black full-width `EDITORIAL` bar.

The user's correction "make it look more like this" required replacing the original modern ivory/gold hero layout, not merely recolouring it. Reference fidelity is chiefly structural.

## Layout QA lessons

- Render all pages before judging. The first attempt looked fine on page 1 but overflowed into two sparse extra pages.
- Fixed-height regions can approximate a historical front page, but they can create empty boxes. Prefer real content density; baseline rules/texture are a fallback, not a substitute for reporting.
- Check occupied-height ratio against the reference. Some vintage scans intentionally finish before the paper's bottom edge.
- A tiny small-print PDF can still work as a zoomable Telegram artifact, but the PNG preview needs a strong masthead and headline at phone scale.
- A region that contains three short paragraphs after a small image looks visually empty next to a real broadsheet. Pour real content into it (the day's work notes, an extended standfirst, additional briefs) before resorting to decorative ruled-line fillers. Reserve repeating-linear-gradient notebook rulings for overflow only.
- `column-count:2` with the default `column-fill:balance` is hostile to short copy. ~130 words in a 600px-tall container split into two sub-columns leaves the right sub-column half-empty and reads as "template did not render." Two fixes, used together:
  1. CSS: `column-fill:auto` so the first sub-column fills naturally and the second only takes overflow.
  2. Editorial guard: enforce a minimum body word count (~220 words for a 600px centre column) and retry the model with an expansion prompt if the first draft falls short.
- Model output on short copy is genuinely short. A one-shot `expand_lead_body` retry (~30s extra model time) is cheaper than fighting the prompt or living with empty cells.
- Always crop the rendered PNG into the major regions and inspect each with vision. The Telegram preview may look fine at thumbnail scale but reveal empty boxes at full resolution. JSON-parses, one-page PDF, plausible file sizes, no console errors — all necessary, none sufficient.
- Mandatory visual audit after every render: left briefs column, centre lead (both sub-columns), right work column, footer, weather strip. Any region where text occupies less than ~60% of the available height needs more content or a layout change.

## Field-construction pitfalls

- Empty `stars_today` + string concatenation produces broken signals like `"Star koala73 / worldmonitor Real-time global intelligence dashboard..."` in the rendered paper. Guard every optional model field with an explicit fallback string (`"Trending on GitHub today"`, `"No paper selected"`). Do not let the LLM decide what fills an empty slot — it will glue the next field over.
- Shell paths and code-style strings (e.g. `~/.hermes/data/hermes-times/issues/`) leak into the printed page if pasted verbatim into the layout. Phrase them as a hint to the reader instead of as leftover code: `"Archive on disk · see ~/.hermes/data/hermes-times"`.

## Cron ownership

When the script itself sends Telegram attachments:

```text
no_agent = true
deliver = local
script = <relative script under ~/.hermes/scripts/>
```

A successful force-run should update artifact mtimes and show `last_status=ok`. This verifies the scheduler environment, not merely the interactive shell.
