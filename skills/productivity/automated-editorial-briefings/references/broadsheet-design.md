# Broadsheet Newspaper Design

Reference for the visual language of a personalised broadsheet-style
morning paper, condensed from a visual-rebuild session where the user
asked to "make it look more like a newspaper." The earlier deck-style
layout (cards on near-empty pages, generous whitespace, sans-serif
display) read as a blog or dashboard, not a paper. The redesign below
produces a broadsheet feel without committing to a specific vintage.

## Trigger language

- "make it look more like a newspaper"
- "it looks too modern / too web / too SaaS"
- "it looks like a blog post, not a paper"
- user provides a vintage broadsheet as a reference

When the user says any of these, this is a *composition failure*, not a
palette swap. Don't recolour the existing layout — replace the layout
with the broadsheet grammar below.

## What changes

### Typography

| Token | Before (deck-style) | After (broadsheet) |
|---|---|---|
| Display font | Helvetica / Arial / sans-serif bold | Georgia / serif, *italic* at large sizes |
| Headline weight | 600–700 | 700, italic, very tight line-height (0.86–0.92) |
| Headline case | Title case | Title case is fine; uppercase tracked sans is wrong |
| Body font | Sans-serif | Serif, justified, with `hyphens: auto` |
| Body size | 13–14px | 10–11px (denser; mock newspaper column density) |
| Eyebrows | Sans-serif, lowercase | Sans-serif, **uppercase**, 0.16–0.22em letter spacing |
| Section headers | Single line of text | Two-rule band: top 1.5px, bottom 0.5px, masthead left + meta right |

The italic display headline is the single biggest perceptual shift — it
cues "broadsheet" before anything else.

### Layout

- **Justify body text.** Newspapers hyphenate and justify. With `hyphens: auto` and a 10px serif, narrow columns read denser and more newspaper-y than left-aligned.
- **Use thin rules instead of cards/borders.** Top 1.5px + bottom 0.5px bands separate sections. No box outlines, no rounded corners, no shadows.
- **Narrow gutters.** 5–6mm between columns, not 12–16mm.
- **Two-column split with a feature panel on the left** is the canonical interior layout: bold typography panel + 2-col card grid on the right.
- **Pull quote** centred with bold top rule + thin bottom rule, italic display, 22–24px.
- **Banner** (italic centred text inside a top/bottom rule) for section openers — "Editor's Note: Five papers worth opening before lunch."

### Imagery

- **High-contrast B&W treatment** on every image via CSS `filter: grayscale(0.45–0.55) contrast(1.08–1.18) saturate(0.7)`. Even full-colour photos become monochromatic.
- **Framed by thin top/bottom rules** (`border-top: 0.5px solid; border-bottom: 0.5px solid`). No rounded corners, no shadows.
- **Cover photo** is the dominant visual; set to `opacity: 0.55` with a left-to-right black gradient veil so headline text over the photo stays legible.
- **Hero artwide** (full-width banner under the lead) is at 56–60mm height with 16:9 cover-crop. Same B&W treatment.

### Palette

| Token | Value | Use |
|---|---|---|
| Paper | `#ede8dc` (warm ivory) | page background |
| Paper warm | `#e1d6c0` | feature panel background |
| Paper deep | `#d5c0a1` | strong feature panel |
| Ink | `#171510` | body text, rules |
| Ink soft | `#3e3330` | secondary text |
| Rule | `#aa9a82` | standard hairlines |
| Rule bold | `#665f55` | top rules, folio |
| Rule faint | `#c8bba2` | very subtle dividers |
| Accent | `#c84a25` (deep red) | age bar, divider, accent type |
| Accent deep | `#8a2c12` | rare, emphasis |

Two-tone palette only. No gradients, no decorative colour fields.

### Paper texture

A subtle dot-grain via stacked CSS gradient backgrounds:

```css
background:
  radial-gradient(circle at 20% 30%, rgba(120, 100, 70, 0.04) 0, transparent 1px),
  radial-gradient(circle at 70% 60%, rgba(120, 100, 70, 0.04) 0, transparent 1px),
  radial-gradient(circle at 40% 80%, rgba(120, 100, 70, 0.03) 0, transparent 1px),
  var(--paper);
background-size: 7px 7px, 11px 11px, 13px 13px, auto;
```

Reads as newsprint grain without resorting to a heavy noise PNG.

## Folio pattern

The bottom-of-page footer with three spans separated by `justify-content: space-between`:

```css
.folio {
  position: absolute; bottom: 9mm; left: 12mm; right: 12mm;
  display: flex; justify-content: space-between;
  border-top: 0.5px solid var(--rule-bold);
  padding-top: 2.5mm;
  font: 700 8px/1 var(--sans);
  letter-spacing: 0.18em; text-transform: uppercase;
}
```

Three spans: `["The Hermes Times", "The Briefing", "02"]`.

## What you don't do

- No card backgrounds other than a single feature panel.
- No rounded corners anywhere.
- No shadows or drop-shadow filters.
- No gradients in body content (only on the cover veil).
- No gold accents, no neon, no cyberpunk imagery.
- No oversized hero icon, no oversized KPI numbers.
- No "designed by a dashboard tool" grid alignment.

## CSS scaffolding snippet

A drop-in CSS file structure for a broadsheet render (request the
`manifest-driven-pdf`/`newspaper-css` companion if you want a 
copy-paste template):

```css
:root {
  --paper: #ede8dc; --ink: #171510; --ink-soft: #3e3330;
  --rule: #aa9a82; --rule-bold: #665f55; --accent: #c84a25;
  --serif: Georgia, "Times New Roman", "DejaVu Serif", serif;
  --sans: "Helvetica Neue", Arial, sans-serif;
}
* { box-sizing: border-box; margin: 0; padding: 0 }
a { color: inherit; text-decoration: none }
/* page, masthead, title, standfirst, pull, cards, feature, artwide, artside, chart, folio, quote ... */
```

Cover the seven core building blocks in this order:
1. `.page` — primitive + paper texture
2. `.mast`, `.meta`, `.eyebrow` — typography tokens
3. `.folio` — bottom-of-page footer
4. `.layout` — page padding + flex column with `.grow`
5. `.cover` — dark hero with photo+veil
6. `.section-meta` — top rule band with masthead+meta
7. `.title`, `.standfirst`, `.pull`, `.banner` — interior typography
8. `.split`, `.feature`, `.cards`, `.card` — composition blocks
9. `.artwide`, `.artside`, `.chart` — visual placements
10. `.quote` — closing italic

## Density sanity check

A correctly dense interior page should have:
- ~110–140 words per narrow column before the next section
- Eyebrows + titles + body stacked tightly, no min-height gaps
- One full-width art image or chart, not both
- One pull quote or banner, not both
- Folio at the bottom with 8–10mm of clearance

If a page overflows into the folio, the agent (not the renderer) is
responsible for splitting — never let CSS hid the overflow or shrink
the body. Drop the section.

## Verification

Visual QA on every page after a redesign, not just page 1:
- `pdftoppm -f N -l N -png -r 110` for each page
- vision_analyze for layout, hierarchy, typography, density
- Specifically check: any clipping into the folio, any empty boxes in
  the left column of a split, any "modern"/"web" language creeping
  back in (e.g. an unbordered card sitting on flat white)
