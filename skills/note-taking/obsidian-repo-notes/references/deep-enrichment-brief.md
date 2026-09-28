# Deep Enrichment Brief — per-repo code-reading subagent workflow

This is the workflow for the **second pass** of repo-note enrichment. Where the bulk
generator (`scripts/generate_repo_notes.py`) writes a single-paragraph stub from
metadata + README, the deep pass reads the actual source code and produces a
~10–20k char note that's audit-ready.

## When this triggers

A parent agent has already bulk-generated stubs and now wants each note substantively
richer. Typical prompt:

> "Deeply enrich the Obsidian vault note for the repo `<slug>` (slug: `<slug>`).
> Write the result to `/tmp/enrich-wave<n>/<slug>.md` — NOT to the vault.
> Read the existing note, examine the cloned repo at `~/projects/<slug>/`,
> fetch GitHub metadata, and produce a detailed enriched note. Follow the
> FULL BRIEF at `/tmp/enrich-wave<n>/BRIEF.md` exactly."

The parent supplies `/tmp/enrich-wave<n>/BRIEF.md` with the field list, output path,
and verification checklist. You are a leaf subagent — do NOT spawn further subagents.

## Hard rules (from the brief)

1. **Never touch the vault note.** The path `~/Documents/.../Repos/<slug>.md` is
   read-only for the duration of the wave. Other parallel agents are writing to
   sibling repos; touching a vault file mid-wave corrupts the parent's verify+merge
   step. Write only to `/tmp/enrich-wave<n>/<slug>.md`.
2. **No fabrication.** Every claim must trace to (a) the README, (b) the actual
   code you read, or (c) the GitHub API. Don't paraphrase the README's "built with
   X" — paste the real dep line from `package.json`. If you can't find it, write
   "Not specified" — never invent.
3. **Don't downgrade.** If the existing note says X and your code-reading
   disagrees, trust the CODE. The parent agent reconciles.
4. **Wikilink exact names.** `[[LLMs and SLMs]]` for themes (spaces matter, exact
   match to `Themes/<Name>.md`). For new concept links, use bare names
   (`[[RoPE]]`) — the parent creates the concept notes after the wave.
5. **No comment-only code blocks** in Getting Started. Skip fences where ≥half
   the lines are `# comments` or none match command keywords.

## Inputs (in priority order)

1. **BRIEF** at `/tmp/enrich-wave<n>/BRIEF.md` — the parent agent's spec; read first.
2. **Existing vault note** at `~/Documents/.../Repos/<slug>.md` — read to learn
   schema (frontmatter fields, section order) and what's already been said. This
   is read-only.
3. **GitHub API metadata:** `gh api repos/OWNER/SLUG` — full JSON, parsed in
   Python (don't use `.{name,language}` jq object constructor; it errors in the
   bundled jq).
4. **README:** `gh api repos/OWNER/SLUG/readme --jq .content | base64 -d`. If 404,
   no README — fall back to the existing note's description + cloned repo contents.
5. **Cloned repo** at `~/projects/<slug>/` — explore freely. Use `ls`, `find`,
   `read_file`, `search_files`, `terminal cat`. The repo is auth'd via `gh`.

## What to read in the cloned repo

In order of value-per-minute for a deep note:

1. `package.json` (or `pyproject.toml`, `Cargo.toml`, `go.mod`) — exact dependency
   versions, scripts, bin entries. This is the authoritative tech-stack source.
2. The main entry point (typically `src/index.ts`, `src/main.py`, `src/lib.rs`,
   `cmd/<name>/main.go`) — read 50–200 lines to understand the dispatcher/loop
   pattern. Count the actual tools/commands/endpoints registered.
3. The top-level source layout — `find src -type f` or `ls src/`. List each
   meaningful directory with a 1-line purpose.
4. `git log --oneline | head -30` and `git log --since="6 months ago" --oneline`
   — commit history shape (sprint vs slow burn vs abandoned).
5. README code blocks for Getting Started commands.
6. The largest non-test source files (often the security/validation layer or the
   main algorithm) — these are the meat of the "Inside the Codebase" section.

## Required body sections (in this exact order)

1. `# <Display Name>` + blockquote tagline: "<repo> is a <category> that <does what>."
2. `## Overview` — 2–4 sentences: what the project is, who it's for, what makes
   it interesting. Pull from README intro + actual code.
3. `## Status` — 1–3 lines: development status, last pushed, deps pinned? tests?
   CI wired?
4. `## Inside the Codebase` with these H3 sub-headings in order:
   - `### Architecture / How it works`
   - `### File structure` — every meaningful directory + 1-line purpose
   - `### Key files` — most important files with 1–2 line descriptions
   - `### Notable details` — design choices, gotchas, hidden gems
   - `### Tech stack` — concrete list of libraries/frameworks with versions
   - `### How to run it` — actual commands from README (clone, install, run)
5. `## What this project is about` — the problem it solves, the motivation, the
   "why does this exist" angle. Be specific, not generic.
6. `## Use cases / When you'd reach for this` — 2–5 realistic bullets.
7. `## Key Features` — emoji-lead bullets. Keep the existing feature bullets if
   they're good; add new ones you discover.
8. `## Getting Started` — code blocks with actual run commands. Include prereqs.
9. `## Stats` — emoji bullets mirroring the existing pattern (⭐, 🍴, 🔒, 📅, 🔄,
   🏷️, 📊, plus license, package name, LOC count, commit count).
10. `## Links` — required: `Part of: [[Public Repos]]` or `[[Private Repos]]`.
    Also include `GitHub:`, `Concepts: [[C1]], [[C2]], ...` (4–6, or 8–12 for
    ML-research repos where the codebase IS a composition of named
    architectural techniques — see SKILL.md pitfall), `Themes: [[T1]]`
    (1–2, exact wikilink match), `Related: [[r1]], ...` (1–3).

11. **(Optional, ML-research / paper-track repos only)** `## Headline numbers`
    between `## Stats` and `## Links`. Results tables (validation loss,
    BLiMP/GLUE/SuperGLUE, paired-bootstrap CIs) sourced from
    `paper/figures/*.json`. Always include the comparison protocol in the
    header. See SKILL.md pitfall for the rationale and the exact template.

## Frontmatter (verbatim from existing note + upgrades)

```yaml
---
repo: <slug>
description: <one-line from README or gh description, cleaned of markdown>
github: https://github.com/<owner>/<slug>
visibility: public|private
language: <primary language from gh>
topics: [<github topics>]
stars: <int>
forks: <int>
status: active|experimental|inactive|archived   # derived from pushed_at age
created: YYYY-MM-DD
last_pushed: YYYY-MM-DD
type: <auto-category>
vault_group: Public Repos|Private Repos
tags: [<expanded list of concept tags you discover>]
---
```

Upgrade frontmatter from the existing note only if (a) the README now has a
description and the previous was empty, or (b) `last_pushed`/`stars` have
changed since the previous note was written. Don't gratuitously rewrite fields
that are already correct.

## Output protocol

After writing `/tmp/enrich-wave<n>/<slug>.md`:

```
Path: /tmp/enrich-wave<n>/<slug>.md
Bytes: <wc -c>
Summary:
- <enrichment line 1>
- <enrichment line 2>
- <enrichment line 3>
```

Then STOP. Don't try to also copy the file to the vault — the parent owns the
verify+merge step. Don't write any other files. Don't edit the BRIEF.

## Verification checklist (parent runs, you self-check)

- [ ] File written to `/tmp/enrich-wave<n>/<slug>.md` (NOT the vault)
- [ ] Frontmatter preserved (or cleanly upgraded)
- [ ] `Part of: [[Public Repos]]` or `Part of: [[Private Repos]]` in `## Links`
- [ ] All **11** body sections present in order (H1+tagline, Overview, Status, Inside the Codebase with 6 H3s, What this project is about, Use cases, Key Features, Getting Started, Stats, Links)

**Note on section count:** older drafts of this checklist said "10". The canonical
count is **11** — H1+tagline, Overview, Status, Inside the Codebase (with 6 H3s),
What this project is about, Use cases, Key Features, Getting Started, Stats, and
Links. `grep -Fqx` confirms presence but not order; the parent verifier should
also do a `[s.index(h) for h in markers] == sorted(...)` self-check.
- [ ] No comment-only code blocks leaked into Getting Started
- [ ] No fabricated versions or features
- [ ] No touched vault file (verify with `ls -la` on the vault path before and after)

## Common pitfalls (apply these to avoid rework)

1. **Tool counts from the README are often wrong.** "~25 tools" in marketing copy,
   20 actually registered in `src/index.ts`. Count from the code, not the prose.
2. **`package.json` repository URL may differ from the canonical slug.** Capitalisation,
   dashes, "MCP" suffix — common drift. Note it in `### Notable details`.
3. **Deleted test files are signals.** A `test-*.js` removed in commit history
   (check `git log --diff-filter=D --name-only`) tells you the author tested once
   and dropped the harness. Capture this in Status.
4. **Commits all in one date window = sprint, not steady-state.** `git log
   --since="6 months ago" --oneline | wc -l` vs total commits. If the ratio is
   ~100% in 4 days, the project is dormant regardless of "stars" count.
5. **No `node_modules`/`dist` in working tree ≠ "doesn't build."** The repo may
   build clean from `npm install && npm run build`. Check the `prepare` script
   in `package.json` — many projects auto-build on publish.
6. **The vault file size is a hint.** A 1.1k-char existing note means you have
   room to grow to 15–20k. A 5k-char note means you're enriching in place.
7. **The brief's char-budget target is soft.** 7–12k chars is the *target*;
   15–20k is acceptable when the substance warrants it. Flag overage in the
   final reply. The recent wave briefs (wave 10+) explicitly target 7–12k
   chars; the structural floor is ~12k bytes when all 11 sections are
   substantively populated, so plan for the upper end of the window from
   the start. **When the brief explicitly names depth dimensions** (e.g.
   "add depth on the data mix percentages, the eval, the position encoding,
   the inference path, and the broader research context"), treat each named
   dimension as a checklist item and verify coverage in the final note. See
   the SKILL.md pitfall "the 5 research-depth dimensions" for the full
   checklist and concrete examples.