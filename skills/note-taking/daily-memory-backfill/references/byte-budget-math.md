# Byte-budget math for single-day backfills

When the user gives a 1.5 KB – 5 KB cap on a daily-memory note, the fixed-shape parts
dominate and you have very little headroom for prose. Use this when planning the first draft.

## Approximate byte shares (UTF-8 markdown, single-day, ~25 sessions)

| Section | Approx. bytes | Notes |
|---|---|---|
| Frontmatter (9 lines) | 130 | Don't touch. |
| `## At a glance` block | 320 | One-liner summary drives most variance. |
| `## What I worked on` (7 bullets) | 530 | Tighter bullets = fewer bytes. |
| `## Decisions / outcomes` (5 bullets) | 480 | |
| `## Sessions today` table (25 rows) | ~3,500 | **The dominant cost.** Each row ≈ 140 bytes (5 columns, padded). Don't trim. |
| `## People` line (1 placeholder) | 60 | The shortest acceptable form: `(none named in session titles or first user messages)`. |
| `## Projects touched` (2 wikilinks) | 240 | |
| `## Concepts / tools` (6 bullets) | 230 | |
| `## See also` (2 links) | 90 | |
| **Total** | **~5,000** | Right at the cap. Trim if adding content. |

## Trim order (cheapest first)

1. One-line summary (compress clauses, drop conjunctions).
2. Long parenthetical enumerations (5–8 items + "and more").
3. `(none...)` People line (shortest natural form).
4. Bullet verbs (drop "explicitly", "specifically", "in particular", "very").
5. Wikilink display-aliases (shorter alias = fewer bytes — but lose readability, don't push).

## Don't trim

- Sessions table rows or column widths.
- Frontmatter.
- The two required `See also` links.
- The `Sessions: N / Messages: N / Sources: ... / One-line summary:` four-line shape in `At a glance`.
- Section heading order.

## Worked example: getting under 5 KB

Starting at 5,479 bytes with a long one-liner and full repo enumeration:

1. "First day on Hermes — desktop setup, capability exploration, settings/Gmail/Rust config, and an 18-subagent batch enriching GitHub repo notes for the knowledge graph." (148 chars)
   → "First day on Hermes — desktop setup, capability exploration, settings/Gmail/Rust config, plus an 18-subagent batch enriching GitHub repo notes for the knowledge graph." (147 chars) — trim "and" → "plus": −2 chars.
2. Trimming 9-repo enumeration to 8 examples: −35 chars.
3. "Rust toolchain was checked and updated as needed" → "Rust toolchain checked and updated as needed": −4 chars.
4. Compress a few bullet verbs: −50 chars cumulative.

Result: 5,479 → 4,984 bytes. About 80 bytes of trim buys you 500-byte headroom.
