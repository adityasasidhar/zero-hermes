# Local-only project notes — full variant brief

Companion to the "Workflow variant: Local-only project notes" section in
`SKILL.md`. Load this when the wave's subject is a project that lives entirely
under `/home/arctic/projects/<slug>/` with no GitHub remote and no public
listing — in-progress / private / pre-release work, scratch experiments,
machine-learning research notebooks, benchmark workspaces that never got
pushed, etc. The deep-enrichment variant's `gh api` inputs are unavailable;
the evidence model is **the on-disk filesystem + a pre-write vault audit**.

## When this variant applies

- `git remote -v` returns empty (or only a placeholder like `git://`).
- The project is not in the public GitHub listing for the owner.
- The user's task says "local project", "in-progress repo", "uncommitted
  workspace", or names a project under `/home/arctic/projects/<slug>/` that
  you've verified has no remote.

If `git remote -v` returns a real `origin` URL, route to the deep-enrichment
variant instead — the GitHub inputs are available and the schema differences
in this variant don't apply.

## Pre-write vault audit (mandatory)

Before writing any note, grep the vault for prior references to the project
name and slug forms. Three greps, run sequentially:

```bash
grep -rli "<ProjectName>" "$VAULT" 2>/dev/null | head -20   # display name
grep -rli "<slug-form>"            "$VAULT" 2>/dev/null | head -20   # slug
grep -rli "<concept-kw-1>\|<concept-kw-2>" "$VAULT" 2>/dev/null | head -20   # 2-3 keywords
```

Empty result across all three means safe to write a new note. A hit at any
step means **read the matched file before writing** — it may be:

- A hand-curated note to preserve per the deep-enrichment "preserve existing
  curated content" pitfall.
- A stub to enrich (treat as stub replacement, draft at the structural floor).
- A same-display-name collision with a different project (disambiguate with a
  parenthetical in `Related:`).

Record the audit result in the 3-line summary:

```
vault pre-write audit: 0 prior references for `obsidian-galaxy-graph`;
captured 6 Inside-the-Codebase subsections, dirty worktree state at HEAD=8bf8817,
model-name swap vs CLAUDE.md (gemma-4-26b-a4b-it, not nemotron-3-nano:30b-cloud).
```

## Staging path

**`/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md`** — NOT
`/tmp/enrich-wave<n>/<slug>.md`. The GitHub-shaped wave uses `/tmp/`; the
local-only wave uses the project-side `kg-sync-tmp/` folder. The mirror
sibling `kg-sync-tmp/github-notes/<slug>.md` holds GitHub-shaped repos.

The vault-untouched guarantee still applies:

```bash
git -C "$VAULT" status --short -- Github/Repos/ Projects/   # must be empty
```

before the 3-line summary. A stray `?? <path>` line means a previous step
touched the vault — stop and investigate.

## Schema (frontmatter)

```yaml
---
repo: <name>
description: <one-line>                # from README/CLAUDE.md or pyproject.toml
github: none                           # no remote
visibility: local                      # replaces public|private
local_path: /home/arctic/projects/<slug>   # absolute path, new field
language: <primary>
topics: [...]                          # derived from code, not the API
status: <see status table below>
created: YYYY-MM-DD                    # git first-commit, or earliest mtime
last_updated: YYYY-MM-DD               # git last-commit, or latest mtime
type: <auto-category>                  # see SKILL.md "Auto-categorize"
vault_group: Projects                  # NOT Public/Private Repos
tags: [<concept list>]                 # 4-6 wikilink-style concepts
---
```

### Status table

| Working tree state | `status:` value |
|---|---|
| Clean, all changes committed, last commit recent | `active` |
| Clean, last commit >180d ago | `inactive` |
| Dirty worktree (uncommitted modifications, untracked files) | `active (uncommitted changes)` |
| Zero git commits, files exist | `scaffolded (no commits)` |
| Has commits but the upstream branch is empty | `abandoned (no upstream)` |

### Topics/tag derivation (code, not API)

The GitHub API's `topics` field is unavailable. Derive `tags:` from:

- `package.json` → `dependencies` keys, `keywords`, `description`
- `pyproject.toml` → `[project] dependencies`, `[project] keywords`, `[project] description`
- `Cargo.toml` → `[dependencies]`, `[package] keywords`
- Source imports → what major libraries are imported (e.g. `import torch`,
  `from flask import Flask`, `import { Plugin } from "obsidian"`)

4–6 concepts, wikilink-style (`[[Concept Name]]`). The deep-enrichment
pitfall "`tags:` is *concepts*, not GitHub `topics`" applies identically.

## Required body sections (10, not 11 — no `## Stats`)

The 11-section shape from the GitHub variant includes `## Stats` (no public
stats card exists for local-only projects — no stars, forks, pushed_at,
etc.). Skip `## Stats`. Replace with a `## Status` line that includes the
local equivalents ("language: TypeScript; package manager: bun; last commit
2026-07-29; dirty worktree at HEAD=8bf8817").

Order:

1. `# <Display Name>` + blockquote tagline
2. `## Overview` — 2–4 sentences from README/CLAUDE.md/AGENTS.md intro + actual code
3. `## Status` — activity, dirty?, commits?, tests?, CI wired?, language/package manager, last_updated
4. `## Inside the Codebase` — 6 H3 sub-headings: `### Architecture / How it works`,
   `### File structure`, `### Key files`, `### Notable details`,
   `### Tech stack`, `### How to run it`
5. `## What this project is about` — the "why does this exist" angle
6. `## Use cases / When you'd reach for this` — 2–5 bullets
7. `## Key Features` — emoji-lead bullets
8. `## Getting Started` — real run commands only
9. `## Links` — `Part of: [[Projects]]` (required), `Local path: <abs path>`,
   `Concepts: [[C1]], ...` (4–6), `Related: [[r1]], ...` (1–3). **No `GitHub:` line.**

## Source-authority rule

When README / CLAUDE.md / AGENTS.md disagrees with the on-disk code or
manifest, the code is authoritative for *technical* claims (model names,
dependency versions, settings keys, schema fields). Treat the prose as
authoritative for *intent* and *workflow conventions*. Surface the
disagreement in `### Notable details` with both values quoted verbatim:

> README/CLAUDE.md says `nemotron-3-nano:30b-cloud`; `agents/models.py`
> instantiates `ChatOpenAI(model="gemma-4-26b-a4b-it", base_url=...)`.
> Authoritative source = `models.py`. Likely cause: model swap after
> CLAUDE.md was written.

## `.env` files

`read_file` auto-blocks secret-bearing `.env` files. Don't bypass via
terminal cat. Note existence (`ls -la .env` → present) and what module
reads them, but never open them. Surface in `### Notable details` as a
credential-handling finding.

## 0-commit handling

When `git log` fails with `fatal: your current branch 'X' does not have
any commits yet`, the repo is a fresh scaffold. Pull `created` from earliest
file mtime and `last_updated` from latest file mtime:

```bash
find "$LOCAL_PATH" -type f -printf '%T+ %p\n' | sort | head -1   # earliest
find "$LOCAL_PATH" -type f -printf '%T+ %p\n' | sort | tail -1   # latest
```

Set `status:` to `scaffolded (no commits)`. Don't fabricate a commit date.

## Dirty worktree handling

When `git status --short` shows uncommitted changes, surface them in
`## Status` by listing the changed files. A dirty tree means describe the
working tree, not HEAD.

## Verification recipe (self-check before the 3-line summary)

```bash
# 1. Output path is correct
ls -la /home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md

# 2. Vault untouched
git -C "$VAULT" status --short -- Github/Repos/ Projects/

# 3. Required sections present in order
grep -Fqx "## Overview"        <slug>.md
grep -Fqx "## Status"          <slug>.md
grep -Fqx "## Inside the Codebase" <slug>.md
grep -Fqx "### Architecture / How it works" <slug>.md
grep -Fqx "### File structure" <slug>.md
grep -Fqx "### Key files"      <slug>.md
grep -Fqx "### Notable details" <slug>.md
grep -Fqx "### Tech stack"     <slug>.md
grep -Fqx "### How to run it"  <slug>.md
grep -Fqx "## What this project is about" <slug>.md
grep -Fqx "## Use cases / When you'd reach for this" <slug>.md
grep -Fqx "## Key Features"    <slug>.md
grep -Fqx "## Getting Started" <slug>.md
grep -Fqx "## Links"           <slug>.md
grep -Fqx "Part of: [[Projects]]" <slug>.md

# 4. No GitHub line (github: none)
! grep -q "^GitHub:" <slug>.md

# 5. No raw README markdown leaks
! grep -E '!\[[^]]*\]\(http[^)]*\)' <slug>.md   # no image refs
! grep -E '\[\!?\[' <slug>.md                   # no badge links

# 6. Byte count
wc -c <slug>.md
```

## Final reply format

Print exactly three things: (1) the absolute output path
(`/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md`), (2) the byte
count of the new file, (3) a 3-line summary naming the categories of
enrichment added + the pre-write audit result.

## Same-display-name collision handling

When the vault already has a note with the same display name but a
different language/type (e.g. two repos both branded "The Deep Field" — one
Python research, one Astro site), disambiguate in `Related:`:

```
Related: [[the-deep-field]] (the older Python research repo that shares the
display name with this Astro site)
```

The parenthetical disambiguates the wikilink target so a future reader
doesn't conflate them. Same pattern applies to display names like
"agent", "router", "toolkit" — common across many projects.

## Common pitfalls recap

| Pitfall | One-line mitigation |
|---|---|
| Writing to `/tmp/` instead of `kg-sync-tmp/local-notes/` | Use the project-side staging folder; verify with `ls` |
| Calling `gh api` on a local-only repo | Don't — `git remote -v` first; if empty, use on-disk inputs |
| Skipping the pre-write vault audit | Always run all 3 grep passes; record result in 3-line summary |
| Trusting README/CLAUDE.md over code | Code wins for technical claims; surface disagreements |
| Reading `.env` via terminal cat | Don't — note existence, don't open |
| Fabricating commit dates for 0-commit repos | Use mtime fallback; set status `scaffolded (no commits)` |
| Hiding dirty-worktree state | List modified files in `## Status` |
| Including `## Stats` with `stars: 0` | Skip `## Stats` entirely |
| Using `vault_group: Public Repos` | Use `vault_group: Projects` |
| Including a `GitHub:` line in `## Links` | Skip the GitHub line when `github: none` |