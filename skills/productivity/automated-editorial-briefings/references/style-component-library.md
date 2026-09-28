# THE HERMES TIMES — style.css component library

Reference for the broadsheet component library shipping with `style.css`
in the v4 renderer. These are the building blocks a leaf or orchestrator
can call via the body-type fields in `render.py`. If you add a component,
add the row here at the same time so the next agent doesn't reinvent
the wheel.

## Quick index

| `body.type` | Component | Use for |
| --- | --- | --- |
| `feature` | Feature block (eyebrow + h2 + body) | Standard story lead. |
| `cards` | 2- or 3-column card grid | Secondary stories. Supports `card_art`. |
| `split` | Two-column feature split | Side-by-side comparison. |
| `quote` | Pull quote with attribution | 1 bold italic line + sig below. |
| `artside` | Float-right image + text column | Magazine-style interleaving. |
| `rank_list` | Numbered list with source eyebrow | "The Wire" / leaderboard. |
| `stat_block` | Big-number callout with accent rule | Headline statistics ($5B / 2 GW). |
| `wire_table_v2` | Two-tone table with right-aligned numerics | Race results, calendars. |
| `score_line` | One-line result (teams · score · meta) | Match results. |
| `opinion` | Two-column editorial column | Closing/op-ed page. |
| `weather` | Compact weather strip | Top of closing page. |
| `tomorrow_list` | Numbered preview list | "Tomorrow's edition" panel. |
| `paper_grid` | Three-column paper card grid | arXiv/HF desks. |
| `mini_chart` | CSS bar chart with `pct` widths | The Wire / data page. |
| `editorial_aside` | Thin-rule editorial sidebar | Editor's notes / opinion. |

## Per-component schema

### `stat_block`

```json
{
  "type": "stat_block",
  "stat": "$5B / 2 GW",
  "label": "AMD ↔ ANTHROPIC",
  "body": "Up to $5B in equity. Up to 2 GW of Instinct MI450. ..."
}
```

Renders with left accent rule, big bold italic display number, uppercase
tracked label, body prose. Sets a reading anchor without dominating a page.
Use once per page for the headline stat; never stack.

### `rank_list`

```json
{
  "type": "rank_list",
  "items": [
    {
      "source": "WIRE · AMD IR",
      "title": "AMD ↔ Anthropic strategic partnership",
      "sub": "Up to $5B equity, 2 GW MI450, H1 2027",
      "url": "https://ir.amd.com/..."
    }
  ]
}
```

Counter-reset CSS, each item gets a leading-zero ordinal (01, 02, ...)
in red italic display. Source eyebrow above the title in tiny sans.
5-7 items fills ~120mm.

### `wire_table_v2`

```json
{
  "type": "wire_table_v2",
  "headers": ["Pos", "Driver", "Team", "Time / Gap"],
  "num_col": 0,
  "rows": [
    ["1", "Norris", "McLaren", "1:38:21.241"],
    ["2", "Verstappen", "Red Bull", "+15.080"]
  ]
}
```

`num_col` (int, optional) is the column index to render right-aligned in
the accent deep color. Negative or missing disables. Header row uses
warm-paper backdrop with tracked uppercase cells. ~5-7 rows is the sweet
spot for an A4 page.

### `mini_chart`

```json
{
  "type": "mini_chart",
  "title": "ANTHROPIC COMPUTE STACK · GW · 2026",
  "rows": [
    {"label": "Nvidia", "pct": 95, "value": "3.8 GW"},
    {"label": "Google TPU", "pct": 50, "value": "2.0 GW"}
  ]
}
```

CSS-only bar chart with accent-deep bars, label on the left, numeric
value on the right. `pct` is the bar width as a percentage of the
container (not the actual value). 4-5 rows render in ~50mm. Pair with a
`wire_table_v2` for the same dataset when you want both visual + tabular.

### `score_line`

```json
{
  "type": "score_line",
  "teams": "Inter Miami at CF Montréal",
  "score": "1-0",
  "meta": "MLS · Sat · Suárez 81' pen"
}
```

Single-row flex layout. Teams left, score center in red, meta right.
Use sparingly — one per match story, max two per page.

### `quote` (with attribution)

```json
{
  "type": "quote",
  "text": "It's a milestone for the Helios platform.",
  "attribution": "Lisa Su, AMD CEO"
}
```

Attribution is OPTIONAL. When present, renders below the italic quote
in tracked uppercase sans. Without it, just the italic block.

### `opinion`

```json
{
  "type": "opinion",
  "items": [
    {
      "eyebrow": "Column · The Lead",
      "title": "AMD is the story, not the silicon",
      "body": "...",
      "sig": "J. Jameson, Masthead Editor"
    }
  ]
}
```

Two-column grid (exactly 2 items — designed for that). Each gets a
red eyebrow, italic display title, body prose, sig line. The default
component for the Closing page opinion column.

### `weather`, `tomorrow_list`

`weather` takes `city`, `temp`, `condition`. `tomorrow_list` takes
`items[]` with `num`, `title`, `sub` each. Both are narrow purpose-built
components for the closing-page masthead; don't reuse outside P10.

### `paper_grid` (arXiv desk)

```json
{
  "type": "paper_grid",
  "items": [
    {"arxiv_id": "2607.21557", "title": "...", "authors": "Xiao Yu et al.", "sub": "..."}
  ]
}
```

3-column grid, each card: red arxiv_id eyebrow → h4 title → italic authors
(subdued) → body prose. Designed for 3 papers; 4 forces one card to wrap.

### `editorial_aside`

```json
{
  "type": "editorial_aside",
  "eyebrow": "Editor's Note",
  "title": "The week's actual story",
  "body": "...",
  "sig": "J. Jameson"
}
```

Top + bottom thin-rule sidebar. Used for editor's notes or column
boxes. Sits well next to a `cards` grid or a `rank_list`.

## Layout rules of thumb

- **One anchor image per page.** A cover hero at the top, or a section
  `art_uri` at the top, but not both. Cards can carry their own
  `card_art` images.
- **One stat_block per page** unless it's a data page (P9).
- **One mini-chart per page**, two on a dedicated data page.
- **One pull quote per page.** Stack quotes and the page reads as a
  bulletin board, not a paper.
- **Cards are narrow and dense.** ~9.5px body, 24mm card_art, two-up.
- **The closing opinion column expects exactly 2 items.** It is a
  2-column grid by design.

## Pitfalls

- **`card_art` overflow on P5 (Hugging Face):** the original CSS set
  `font-size: 10px` and `card-art height: 28mm` which made the body text
  overlap the date eyebrow. Fixed: smaller body font (9.5px),
  `min-width: 0; overflow: hidden` on `.card`, `card-art` height reduced
  to 24mm. The HtmlOutput still touches the folio edge by ~1mm —
  acceptable given card body limit of ~220 chars.
- **Two `wire_table` body types in render.py:** `wire_table` (uses
  per-row `num` array — older, less elegant) and `wire_table_v2` (uses
  `num_col` to mark the numeric column — preferred). Always use
  `wire_table_v2` for new manifests.
- **`stat_block` font scaling:** the `.stat-num` is `font-size: 28px`.
  Stats above ~6 chars overflow. Limit `stat` to 6-8 chars; longer
  numbers go in a `wire_table_v2` or the body prose.
