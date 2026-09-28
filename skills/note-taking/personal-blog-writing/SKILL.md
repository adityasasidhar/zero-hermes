---
name: personal-blog-writing
description: Write new blog posts for Aditya's personal Astro blog (the-deep-field). Load when the user asks for a blog post, a writeup of their research, a tutorial walkthrough, or any markdown content that should land in `src/content/blog/`. Covers the live-working-tree detection, content-collection schema, tone rules, and the recurring wrong-repo trap.
---

# Personal blog writing — the-deep-field

Aditya runs his personal blog as an Astro 7 site called **The Deep Field**. Writing a new post means dropping a markdown file into the content collection and (usually) previewing it on a local `astro dev` server. The session that produced this skill (2026-07-19) hit a hard-to-spot trap: **two working trees of the same blog exist on disk, and only one of them is live.** This skill is the postmortem.

## Critical preflight: find the LIVE working tree

**Before writing any blog file**, run from `/home/arctic/projects/`:

```bash
# Which astro dev server is actually running?
ps -ef | grep "astro dev" | grep -v grep
ss -ltnp 2>/dev/null | grep -E ":4321|:3000|:5173"
```

The path in the `astro dev` command line is the live repo. **Write there.**

The known trap:

| Path | Status |
|---|---|
| `/home/arctic/projects/website/the-deep-field/` | **LIVE.** Has `AGENTS.md`, real post history, hero image assets, the user's own already-written `recursive-babylm.md`. |
| `/home/arctic/projects/adityasasidhar.github.io/` | **TRAP.** GitHub-Pages-named path; looks like "the blog" but is NOT what `npm run dev` is serving from. |

If a future session finds the file went into the wrong repo and the user reports "I can't see the changes," this is the cause. Move the file, do not re-write it from scratch unless the user wants new content.

## AGENTS.md auto-loads when you `cd` into the live repo

`/home/arctic/projects/website/the-deep-field/AGENTS.md` documents the stack, schema, and gotchas inline. The next time you write a blog post, `cd` into that path so the AGENTS.md context reaches the model automatically.

## Blog post workflow (verified 2026-07-19, updated 2026-07-19 after mermaid pitfall)

1. **Detect the live repo with `ps` and ONLY write to that path.** Run `ps -ef | grep "astro dev" | grep -v grep` and `ss -ltnp | grep -E ":4321|:3000|:5173"`. The path in the `astro dev` argv is the live repo. Surface it to the user verbatim, then write there — never into `adityasasidhar.github.io/` even if it looks like "the right path." If the dev server is NOT running, ASK the user which repo they want — do not guess.
2. **Check for duplicates first**: `ls <live>/src/content/blog/`. If a post on the same topic already exists, surface that to the user BEFORE writing — they may want to edit the existing one, not duplicate it. (Caught this in-session: Codex wrote a fresh BabyLM post into the wrong repo, ignoring the user's own already-committed `recursive-babylm.md` in the live repo.)
3. **Read the tone reference**: `<live>/src/content/blog/claude-code-in-100-lines.md` (the only other post with a comparable first-person voice). For research posts, also read `recursive-babylm.md` — Aditya's own post about his BabyLM 2026 work — to gauge how he actually writes about his own work.
4. **Verify the schema** in `<live>/src/content.config.ts`: `title` (string), `description` (string), `pubDate` (z.coerce.date), optional `updatedDate`, optional `heroImage` (must be a real file under `src/assets/`).
5. **Delegate to a subagent** for the heavy writing — this is exactly the class of work `delegate_task` handles well: reasoning-heavy, single-deliverable, no in-context back-and-forth needed. In the subagent prompt, pass:
   - The exact output path (the LIVE repo's `src/content/blog/<slug>.md` — verbatim, do not paraphrase)
   - The tone reference and any source papers/repos to mine
   - The schema frontmatter fields
   - "DO NOT commit" and "DO NOT touch any other file" rules
   - The mermaid rules below (subagents routinely ship syntax errors otherwise)
6. **If the subagent wrote any ` ```mermaid ` blocks, validate them with mmdc** before declaring done (see Mermaid pitfall below). The subagent's "I wrote it" is not the same as "Mermaid can parse it."
7. **Verify the file after the subagent reports back**: `wc -w`, check the first 8 lines, scan for any numbers the subagent might have fabricated, check that no other files in the repo were touched (`git status --short` from the live repo).
8. **Report back** with: file path, word count, frontmatter preview, list of numbers quoted verbatim from sources, anything you were uncertain about.

## Mermaid pitfall — verified 2026-07-19

Subagents (Codex especially) frequently ship mermaid blocks with parser-breaking syntax. The blog layout uses CDN mermaid@11, so blocks render client-side — by the time the user sees the page, Mermaid has already failed and you get the bomb icon. Always validate.

**Validate a block before declaring done**:

```bash
cat > /tmp/check.mmd <<'EOF'
flowchart LR
    X[input] --> A
    subgraph SB1["Super-block 1"]
        A --> A1["run 1"] --> A2["run 2"]
    end
    A2 --> O[output]
EOF
cat > /tmp.puppeteer.json <<'EOF'
{"args": ["--no-sandbox"]}
EOF
npx -y -p @mermaid-js/mermaid-cli@11 mmdc \
  -i /tmp/check.mmd -o /tmp/check.svg -p /tmp.puppeteer.json -q
echo "exit: $?"   # 0 = parses, non-zero = syntax error
```

The `--no-sandbox` puppeteer config is required on Ubuntu 23.10+ (Chromium AppArmor blocks unprivileged userns). Without it mmdc fails with "No usable sandbox" before it ever tests Mermaid syntax.

**Common syntax errors to spot in subagent output** (catch them before validating):
- Node names that look like edges: `A[x] --> SB1` parses as a single node `A` with square-bracket shape, not as an edge from x to SB1. Use `X[x] --> A` instead.
- Subgraph ID colliding with a parent node: `subgraph SB1` cannot coexist with a node also named `SB1`. Rename one.
- Unicode subscripts (`SB₁`, `SB₂`) and middle dots (`·`) trip Mermaid 11.16's parser when combined with other edge cases. Drop them unless the diagram really needs them.
- Edge labels with `|...|` that contain commas or parentheses often need quoting.

If mmdc returns non-zero, fix the block in-place (don't rewrite the whole post) and re-validate. The exact error message tells you which line.

## Mermaid sizing pitfall — verified 2026-07-19

Even when Mermaid parses successfully, the diagrams render at whatever natural width and font size the theme emits (~11px labels) inside a `.mermaid` container that has no width set. Result: tiny diagrams that look broken.

**Fix the layout's CSS once** (`<live>/src/layouts/BlogPost.astro`, under `.prose :global(.mermaid)`):

```css
.prose :global(.mermaid) {
  text-align: center;
  margin: 2.5rem 0;
  overflow-x: auto;
  width: 100%;
}

.prose :global(.mermaid svg) {
  max-width: 100%;
  height: auto;
  font-size: 16px;
}

.prose :global(.mermaid svg .nodeLabel),
.prose :global(.mermaid svg .edgeLabel),
.prose :global(.mermaid svg .label) {
  font-size: 14px;
}
```

This is a one-time layout edit that improves every existing post, not just the new one. Confirm with the user before applying — they may want diagrams scoped per-post.

## Astro dev server picks up `src/content/blog/*.md` live

No restart needed. Just refresh the browser after writing — Vite watches the content collection. Don't waste time killing and restarting `astro dev`.

## Tone rules (from Aditya's own posts)

- First-person, conversational, walks concepts through like explaining to a smart friend. Not academic-stiff.
- Phrases like *"I kept asking myself…"* and *"I wanted to understand what was actually happening underneath"* are his actual idioms — fine to mirror.
- Honest about limits (one training seed, missing ablation, no repeated-data control, etc.) — he WANTS this in research posts.
- Mermaid diagrams (```mermaid blocks) where they carry weight. The layout renders them client-side via CDN mermaid@11, so they're free. Don't pad with them.
- **No author byline in the body** — frontmatter is the only metadata.
- Indian-English phrasings are natural; don't scrub them out.
- Never invent benchmark numbers, author names, or arXiv IDs. If unsure, keep the claim qualitative and say so in the report.

## Hard rules (from the 2026-07-19 mishap)

- ❌ Do **NOT** write to `/home/arctic/projects/adityasasidhar.github.io/`. That is not the dev repo.
- ❌ Do **NOT** commit. Aditya reviews and commits himself.
- ❌ Do **NOT** duplicate an existing post — check `src/content/blog/` first.
- ❌ Do **NOT** install dependencies, run `npm run build`, or run `astro check` unless asked.
- ✅ Do **detect the live repo via `ps`** before writing.
- ✅ Do **pass exact paths and constraints to the subagent** — subagents have no memory of this conversation.
- ✅ Do **verify the file after** the subagent reports back — subagent success reports are not facts.

## References

- `references/live-repo-and-tone.md` — tone anchors, schema details, the duplicated babylm-repo trap (verified 2026-07-19).
