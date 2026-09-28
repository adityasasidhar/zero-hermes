# Bulk project-note enrichment (vault ↔ cloned repos)

Use this when populating or enriching many project notes in the vault from
real source (GitHub metadata + cloned code). Developed while enriching 50+
repo notes for adityasasidhar's vault.

## Note schema (YAML frontmatter + body)
Every project note carries a machine-readable frontmatter block (good for
Obsidian Dataview) followed by a readable body:

```markdown
---
repo: <name>
description: <one line>
github: https://github.com/<owner>/<name>
visibility: public | private | local
language: <GitHub primary language or "Unknown">
topics: [a, b]
stars: <n>
forks: <n>
status: active | experimental / inactive | archived
created: YYYY-MM-DD
last_pushed: YYYY-MM-DD
type: <auto-category: LLM / Agent, ML Research / Model, Learning / Educational, Web App, CLI Tool, Game, Application / Utility, Config / Profile>
vault_group: Public Repos | Private Repos | Projects | Research
---
# <Title>
<overview paragraph>
## Overview
## Key Features   (bullets, from README)
## Tech Stack     (language + detected libs/services + topics)
## Getting Started (real commands from README, if any)
## Stats
## Links           (GitHub URL + `Part of: [[Group]]`)
```

For **local-only** projects (no GitHub remote) swap `github:` → `none`,
`visibility: local`, and add `local_path:` + `git_initialized: false`.

## Pulling real data (never fabricate)
- Repo metadata: `gh api "users/<owner>/repos?per_page=100&type=owner&sort=updated" --jq '.[] | {name,description,language,visibility,stargazers_count,forks_count,topics,created_at,updated_at,pushed_at,homepage,default_branch,size,archived,fork}'`
- Private repos are excluded from the public listing; fetch each individually with `gh api repos/<owner>/<name>` (gh auth covers private).
- README: **use `scripts/fetch_readmes.py`** rather than hand-rolling the call.
  The naive `gh api repos/<owner>/<repo>/readme --jq '.content'` then
  base64-decode **silently corrupts repos that have no README**: on a 404 `gh`
  still prints an error JSON object to stdout, and base64-decoding that yields
  ~38 bytes of identical binary garbage that looks like a (tiny) valid file. The
  script guards `gh`'s exit code + stdout, decodes with `errors="strict"`, and
  writes an **empty marker file** for missing READMEs so agents skip cleanly.
- **jq gotcha:** the bundled jq rejects object-constructor syntax `.{a,b}` — fetch full JSON and parse in Python instead.
- Clean README before use: strip HTML/badges (`[![..](..)](..)`), images, empty-link remnants, and HTML comments; drop comment-only fenced code blocks from "Getting Started".
- `status` heuristic: `archived` → archived; else if (now - last_pushed) > 180 days → experimental/inactive; else active.

## Pitfalls (learned the hard way)

- **`gh api .../readme` returns 404 error JSON on repos with NO README — and
  base64-decoding that error JSON yields ~38 bytes of identical garbage** that
  looks like a valid (tiny) file. If you blindly decode `stdout` without checking,
  every no-README repo gets a junk README, and a delegated subagent that trusts
  the file may write nonsense or (if it ignores the no-fabrication rule) invent
  features. **Always guard before decoding:**
  - check `gh`'s exit code (`returncode != 0`), and
  - check `stdout.strip() not in ("", "null")`,
  - then `base64.b64decode(stdout).decode("utf-8", errors="strict")`.
  - On a missing README, write an **empty marker file** (0 bytes) so downstream
    agents can skip cleanly instead of parsing garbage.
  A robust implementation lives at `scripts/fetch_readmes.py` — reuse it.
- **`hermes_tools.terminal()` caps stdout at ~50KB.** A large `gh api` payload
  (e.g. a 63-repo owner listing ≈ 366KB) gets truncated mid-JSON →
  `JSONDecodeError: Expecting ',' delimiter`. **Fix:** run `gh api ... > /tmp/out.json`
  via the real `terminal()` tool (NOT `hermes_tools.terminal`), then `json.load`
  from disk. The cap is on the sandbox tool's returned text, not on `gh` itself.
- **The SAFE pattern protects the vault even when a source is corrupt.** In one
  run, 12 READMEs were corrupted by the 404 bug above; because each leaf subagent
  was told to copy the note verbatim when the README had nothing usable (and to
  never edit the vault directly), they all produced correct verbatim copies and
  no invented content leaked in. Trust the verify-before-apply gate; do NOT apply
  any `/tmp/enriched/<name>.md` that fails verification.

## Enrichment via delegated subagents (SAFE pattern)
Goal: add a `## Inside the Codebase` section (Architecture / File structure /
Key files / Notable details, ~250 words) sourced from the cloned repo.

1. Make a unique temp dir, e.g. `/tmp/enriched/`.
2. **One leaf subagent per repo** (role `leaf`). Each:
   - reads the original note + the cloned source,
   - writes the FULL enriched note to `/tmp/enriched/<name>.md`
     (copy original verbatim, insert the new section right after `## Overview`,
     preserve frontmatter + the `Part of: [[Group]]` wikilink),
   - **never edits the real vault note directly**.
3. **Verify each temp before applying** (a flaky agent must not blank/corrupt
   a vault note): assert `## Inside the Codebase` present, frontmatter intact,
   `Part of: [[..]]` preserved, and file size grew vs original.
4. Only after verification, copy temp → real note.

## Concurrency
`delegate_task` caps parallel children at `delegation.max_concurrent_children`
(default 3). For large batches, raise it first:
`hermes config set delegation.max_concurrent_children 10`
(The agent cannot edit `~/.hermes/config.yaml` directly — use the `hermes config`
CLI.) Then dispatch in waves of ≤ that limit, verifying each wave.

## MOC wiring
Add new notes to the relevant map-of-content with `[[wikilinks]]`
(e.g. `Projects/Projects.md`, `Research/Research.md`) and verify every
`[[link]]` resolves to a real file so nothing is orphaned.

## Reconciling vault notes ↔ local clones
- To find repos missing locally: compare vault note names (Github/Repos/*.md)
  against dirs in the projects folder; clone missing with `gh repo clone
  <owner>/<name> <dest>/<name>` (authenticates private repos automatically).
- To find local dirs not in the graph: list non-git dirs in the projects
  folder; create notes for the real ones (skip empties like an empty `coding/`
  dir, and skip containers whose only child is already a tracked repo).
