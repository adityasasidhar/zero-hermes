---
name: personal-blog-authoring
description: Write first-person technical blog posts for a personal Astro/static site about the user's own research projects or papers. Covers voice matching, pre-flight verification of the live repo (multi-checkout footgun), figure integration via Astro Image, mermaid patterns, and the no-commit-by-default convention.
trigger: |
  Use this skill when the user asks to write a blog post about their own work
  — a paper, a research project, a coding experiment, a model architecture
  study — for their personal site. Signals: "write a blog on my work", "blog
  about my X project", "post about the paper", "first-person writeup", or
  any reference to specific repos/papers/data the user owns. Also fires when
  the user asks to improve an existing personal blog post (figures, sizing,
  mermaid rendering).
---

# personal-blog-authoring

Class-level skill for authoring first-person technical blog posts on a
personal Astro / static site, in the user's own voice, about their own
research / engineering work. Built from a real session where the user
submitted a BabyLM 2026 paper and wanted a blog post that explained it
the way they would explain it to a smart friend.

## Hard rules (in priority order)

1. **Verify the live repo BEFORE writing a single byte.** Multi-checkout
   / mirror / fork setups are common. The user runs `npm run dev` (or
   equivalent) from one specific working tree; if you write into a
   different copy, your changes never reach the served site. This is the
   single biggest footgun in this workflow — see `Pre-flight checklist`
   below. Getting this wrong causes visible user frustration.

2. **Match the user's voice exactly.** Read at least one existing post
   before writing. Match register (casual vs. measured), paragraph length,
   em-dash usage (the user in this profile removes em-dashes — see git
   history: `2bd475d remove em dashes from blog post`), code-block density,
   and use of phrases like "I kept asking myself..." or "I wanted to
   understand what was actually happening underneath."

3. **Pull numbers verbatim from the source paper / data.** Never
   fabricate benchmark numbers, parameter counts, or statistical
   intervals. If the source has it, copy it. If it doesn't, write
   qualitatively and say so. This is non-negotiable for research posts.

4. **No commit by default.** The user reviews and commits themselves.
   Confirm this in the task brief; do not assume. Leave untracked files,
   summarize what was written, stop.

5. **Match existing site conventions.** Read the site's AGENTS.md (if
   any), content schema (`src/content.config.ts` or equivalent), and one
   existing post before writing. Use the same indentation (tabs vs
   spaces), the same frontmatter keys, the same hero-image pattern, the
   same slug style.

## Pre-flight checklist (do this BEFORE delegating to a subagent)

```bash
# 1. Find the live working tree — whichever repo the dev server runs from.
ps -ef | grep -E "(astro|vite|next)" | grep -v grep
# or
lsof -iTCP:<port> -sTCP:LISTEN

# 2. Read that repo's AGENTS.md / README for layout + conventions.
cat <live-repo>/AGENTS.md 2>/dev/null

# 3. Confirm the content collection schema (frontmatter shape).
cat <live-repo>/src/content.config.ts 2>/dev/null

# 4. Read ONE existing post to lock in voice + frontmatter shape.
```

If the user names a project folder explicitly but it's not the live
working tree, ask before writing. If they say "write it in X" mid-
session, that is a correction — act on it immediately and copy / move
your work to X.

## Frontmatter pattern (Astro content collection)

Match whatever the schema requires. For the typical `blog` collection:

```yaml
---
title: '<post title — single line, no trailing period, conversational case>'
description: '<one-sentence summary, ≤ 200 chars, mirrors how the user pitches the post>'
pubDate: 'YYYY-MM-DD'   # coerce.date in Zod, string is fine
heroImage: '../../assets/<slug>.jpg'   # omit if no good image; never reuse a misfit
---
```

`updatedDate` is optional; only add it if the post genuinely had a
revision. `tags` is uncommon for personal research blogs — skip unless
the user has used it elsewhere.

## Narrative structure (what works for "explain my paper")

This is the template that landed well in the reference session:

1. **The question.** Open with the specific architectural / experimental
   choice being tested. One sentence. Conversational.
2. **Why this setting.** The fixed-budget framing — why BabyLM / a
   constrained setup / a particular benchmark makes the question clean.
3. **The design.** Show the architecture / variants. Mermaid diagram of
   the structure (recipe unrolling, super-block layout, etc.) when it
   actually helps — not as decoration.
4. **The controlled comparison.** Tables with exact counts. Two
   parameter-matched pairs is the canonical setup; lean on the
   "to the digit" framing.
5. **The results table.** Real numbers from the paper. Replace any
   "lower than its twin" placeholder prose with actual values — figures
   do the work; the table should be self-contained.
6. **The figures.** 3-4 paper figures, each with a one-paragraph
   caption. Place them at the narrative point where they answer the
   question the reader is currently asking, not in an appendix dump.
7. **The nuance.** Where the clean story gets messier — metric
   disagreements, late crossovers, the result you almost reported
   wrongly. This is where voice matters most.
8. **Compute cost.** Separate section if there's a real tradeoff
   (inference cost, throughput, memory). Don't bury it.
9. **Limits.** Honest list: one seed, no ablation X, no control Y. Naming
   limits is a strength, not a weakness.
10. **Where you landed.** One short paragraph. Concrete next experiment.

## Figures — copy, then generate, then place

**Copy paper figures first.** Look for `paper/figures/*.png` (or similar)
in the project repo and copy them to the blog's `src/assets/`. Don't
re-render from the matplotlib script — the paper PDF/PNGs are already
production-quality.

**Generate new figures with PIL, not matplotlib, when in a uv ephemeral
env.** See `references/figure-generation-pil.md` for the working
pattern. matplotlib + numpy often breaks with `_multiarray_umath` import
errors in `uv run --with`; PIL is portable and ships with the system
fonts you need.

**Reference figures via relative paths** — Astro's `Image` component
wants an `ImageMetadata` import, but for blog posts where the image is
already on disk under `src/assets/`, a plain markdown image with a
relative path works fine and avoids the prop-drilling:

```markdown
![Caption text.](../../assets/figure-name.png)
```

Style the caption inline: italics, sentence case, no trailing period
unless the caption ends with a sentence. Keep captions to 1-2 sentences.

## Mermaid patterns

The site renders ```mermaid blocks client-side (CDN `mermaid@11`). Two
pitfalls repeat:

1. **Node / subgraph ID collision.** A node named `SB1` AND a subgraph
   named `SB1` in the same diagram causes Mermaid 11.x to throw
   "Syntax error in text". Use distinct IDs:
   ```mermaid
   flowchart LR
       X[input x] --> A
       subgraph SB1["Super-block 1"]
           A --> A1["run 1"] --> A2["run 2"] --> A3["run 3"]
       end
       A3 --> B
       subgraph SB2["Super-block 2"]
           B --> B1["run 1"] --> B2["run 2"] --> B3["run 3"]
       end
       B3 --> O[output]
   ```
2. **Edge source with shape syntax.** `A[x] --> SB1` parses `A[x]` as a
   single shaped-node token, not as an edge from `A` to `SB1`. Use
   `X[input x] --> A` instead.

**CSS sizing** — the default Mermaid SVG is ~600px wide and uses ~11px
font, which reads as small in a blog prose column. The fix lives in
the layout file (e.g. `src/layouts/BlogPost.astro`):

```css
.prose :global(.mermaid) {
  text-align: center;
  margin: 2.5rem 0;
  overflow-x: auto;
  width: 100%;
  max-width: 100%;
}
.prose :global(.mermaid svg) {
  display: inline-block;
  min-width: 720px;   /* force usable size; wrapper scrolls on narrow viewports */
  max-width: none;
  width: auto;
  height: auto;
  font-size: 16px;
}
.prose :global(.mermaid svg text) { font-size: 14px; }
@media (max-width: 768px) {
  .prose :global(.mermaid svg) { min-width: 560px; }
}
```

`width: 100%` on the wrapper alone does NOT size the SVG up — you need
`min-width` on the SVG itself. Tested on Mermaid 11.16.

## Delegation brief template (for handing to Codex / Claude Code)

```text
Write a blog post for <user> in their Astro blog at <LIVE-REPO>.

DO NOT commit anything — user reviews and commits themselves.
DO NOT touch any other file. Create one new file at the path below.

Source material — READ:
- <paper.md or equivalent>
- <README.md for headline numbers>
- <existing-post.md for voice reference>

Create: <LIVE-REPO>/src/content/blog/<slug>.md

Frontmatter must match <LIVE-REPO>/src/content.config.ts exactly.
Length: 1500-2500 words. No heroImage unless the user said so.
Use 2-3 mermaid blocks where they help.
Pull every number verbatim from <paper>; never fabricate.
Write in first person. Match the tone of <existing-post>.
Include a short "honest limits" section — the user values this.

Report back:
1. Absolute path written
2. Word count
3. First 8 lines (frontmatter + opening)
4. Which numbers came from the paper verbatim
5. Anything you were uncertain about

Do NOT run git, do NOT install deps, do NOT build the site.
```

## What "done" looks like

- One new file created at the path the user specified (or the
  inferred live-repo equivalent)
- `git status` in that repo shows exactly one untracked file, nothing
  else
- Word count in the 1500-2500 range (or whatever the user asked for)
- All numbers traceable to source material
- Frontmatter validates against the schema
- No commits, no PRs, no pushes
- Brief report back with: path, word count, first 8 lines, the
  numbers you verified, anything you were uncertain about

## Reference files (deep dives)

- `references/multi-checkout-preflight.md` — the single biggest footgun
  in this workflow: detecting which of N mirror/clone directories the
  dev server is actually running from. 30 seconds of `ss -ltnp` saves a
  wasted file and a frustrated user.
- `references/mermaid-astro-sizing.md` — the CSS that actually makes
  mermaid diagrams readable in an Astro blog post (the `min-width`
  trick that `width: 100%` doesn't achieve) plus the four Mermaid 11.x
  parser pitfalls that cause "Syntax error in text".
- `references/figure-generation-pil.md` — the PIL-over-matplotlib
  pattern for generating bar charts in a uv ephemeral env, including
  the working rotated Y-axis label helper and the specific failure
  modes that bite `uv run --with matplotlib`.

## Pitfalls (don't make these mistakes)

- **Writing into the wrong repo.** This is the single biggest footgun.
  Always pre-flight verify. If the user catches you doing it, expect
  visible frustration — drop everything and copy/move the file to the
  correct repo immediately.
- **Deleting files the user wrote.** If the user has an existing
  version of a similar post, don't delete it on the way to replacing
  it. Read it first; often the user's version is better than what a
  subagent would generate.
- **Generic intro posts when the user wants about-their-work posts.**
  If the user says "I have worked on it, write about my work",
  that's an explicit pivot away from a generic topic overview toward
  the personal paper. Drop the generic post (delete the file), write
  the personal one.
- **Fabricating benchmark numbers or author names.** When a subagent
  doesn't know the exact arXiv ID or has the author list incomplete,
  the right answer is "I'll keep this qualitative" — not invented
  specifics.
- **Honouring a rename in only one place.** When the user picks a
  new title, update the frontmatter `title` field. Don't forget to
  also rename the file slug if the URL matters.
- **Generating two near-duplicate posts.** If the user has an existing
  post on the same topic, treat it as the canonical version. Refine
  it; don't write a parallel one with a different title.
- **Ignoring the AGENTS.md.** The repo's AGENTS.md is the source of
  truth for layout, conventions, and gotchas. Read it before writing.
- **Running the build before the user signs off on visuals.** For
  creative UI work, hold off on `npm run build` and lint until the
  user confirms the visuals look right. (When the system asks for
  verification, run a build to satisfy the gate — but don't claim the
  work is "fully verified" until the user has eyeballed the output.)
- **Committing on the user's behalf.** Default is "no commit". They
  review and commit. Confirm this in the brief every time.
