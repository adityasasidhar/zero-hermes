---
name: obsidian-github-sync
description: "Bulk-generate schema-consistent Obsidian notes for a GitHub user's repositories (real metadata + READMEs), ideal for a personal 'knowledge graph' vault. Covers the data-pipeline, note schema, and extraction pitfalls."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Obsidian, GitHub, gh-cli, knowledge-graph, vault, notes, automation]
    related_skills: [obsidian, github-repo-management, github-auth]
---

# Obsidian GitHub Sync

Turn a GitHub account into a structured, browsable **knowledge graph** inside an Obsidian vault — one detailed note per repository, with consistent frontmatter + body sections, auto-extracted from each repo's real metadata and README.

Use this when the user has (or wants) an Obsidian vault that tracks their GitHub projects as notes — e.g. a `Github/Repos/<repo>.md` layout with `Github/Public Repos.md` / `Github/Private Repos.md` index notes, linked via `[[wikilinks]]`.

## When to use
- "Document all my repos in my vault", "build a knowledge graph of my GitHub projects", "create detailed info for each project .md", "sync my repos into Obsidian".
- Any task needing structured, **repeatable** repo→note generation instead of hand-writing dozens of notes.

## Prerequisites
- `gh` CLI authenticated (`gh auth status`). See `github-auth` skill.
- An Obsidian vault with a target folder for repo notes (e.g. `Github/Repos/`).
- Python 3 (stdlib only — no pip needed).

## The schema (full spec + example in references/note-schema.md)
Each note has a **YAML frontmatter** block (machine-readable, Dataview-friendly) followed by a **human-readable body**:

- **Frontmatter:** `repo, description, github, visibility, language, topics, stars, forks, status, created, last_pushed, type, vault_group`.
- **Body:** `# Title`, Overview, Key Features (from README), Tech Stack (language + detected libs/services + topics), Getting Started (real commands from README), Stats, Links (`Part of: [[Public Repos]]` / `[[Private Repos]]`).

`status` is derived (active / experimental-inactive / archived) from `pushed_at`. `type` is auto-categorized: LLM/Agent, ML Research/Model, Learning/Educational, Web App, CLI Tool, Game, Application/Utility, Config/Profile.

## Workflow
1. **Resolve context.** Owner = `--owner` arg or `gh api user --jq .login`. Vault = `--vault` arg or `~/Documents/fun/Github/Repos`. Confirm the folder exists.
2. **Fetch metadata.** `gh api "users/<owner>/repos?per_page=100&type=owner&sort=updated"` → `json.loads` the **full JSON**. (See Pitfalls: do NOT use `--jq '.{...}'` object constructors.)
3. **Fetch READMEs.** For each repo: `gh api "repos/<owner>/<repo>/readme" --jq .content`, base64-decode. ~70% will have one.
4. **Generate.** Build each note from the schema (extraction logic in the script). Fallback description = GitHub `description` → README intro → `''`.
5. **Write deterministically.** Overwrite each `<vault>/<repo>.md`. Idempotent — safe to re-run after README edits.
6. **Validate.** Grep for `![` (badge residue) and confirm every note has the required frontmatter fields + a `Part of: [[...]]` link.

## The script (scripts/generate_repo_notes.py)
Self-contained, re-runnable generator implementing the full pipeline + extraction + validation. Run:
```bash
python3 ~/.hermes/skills/note-taking/obsidian-github-sync/scripts/generate_repo_notes.py \
  --owner adityasasidhar --vault ~/Documents/fun/Github/Repos
```
Supports `--dry-run` and `--limit`. Regenerating after README edits is safe (overwrites).

## Pitfalls (learned the hard way)

- **Alternate working trees of the user's own repos are NOT new repos.** When
  scanning `/home/arctic/projects/` for repos that need new `Github/Repos/<X>.md`
  notes, a directory whose `git remote get-url origin` matches another local
  working tree *and* whose HEAD is byte-identical to that tree is an alternate
  clone, not a new project. Verified 2026-07-31: `~/projects/marie/` had the
  same remote as `~/projects/Gappy/` (`https://github.com/adityasasidhar/Gappy.git`)
  and the same HEAD `d178a0d`; `~/projects/babylm/` matched `~/projects/recursive-babylm/`
  at HEAD `f2ff97c`. **Don't write a new repo note for these.** Instead, add a
  `Local working trees:` line to the canonical note's `## Links` section:
  ```markdown
  - Local working trees: `~/projects/Gappy/` (canonical) and `~/projects/marie/` (alternate; same HEAD `d178a0d`, untracked `panchai_flowchart.html`)
  ```
  The uncommitted extras in the alternate tree (the `panchai_flowchart.html`
  in marie, the `eval/` and `.env`/`.env.example` in babylm) deserve a parenthetical
  so a future reader knows which tree to look at for the in-flight work.
- **When the user says "make sure we add all"**, the right read is "audit every
  root under `/home/arctic/projects/` AND `/home/arctic/Documents/fun/Github/Repos/`,
  don't selectively skip on the first pass, and re-audit at the end before
  declaring done". Verified 2026-07-31: an initial pass that classified each
  project root individually missed two alternate working trees that turned out
  to need reconciliation against their canonical notes. The fix is a single
  pre-declare check (`find /home/arctic/projects -maxdepth 4 -type d -name '.git'
  -exec sh -c 'cd $(dirname $1) && git remote get-url origin' _ {} \; | sort | uniq -c`)
  to surface duplicate remotes, plus a re-audit pass after the MOCs are wired.
- **`gh api --jq '.{name, description}'` FAILS** with `failed to parse jq expression (line 1, column 2)`. The bundled jq does **not** support object-constructor syntax. Either **omit `--jq` and `json.loads` the full response in Python**, or use a valid field filter like `--jq '.login'`. Fetch raw and parse in Python — never rely on `.{...}`.
- **README badge residue.** READMEs contain `[![CI](img)](link)`, `![...](...)`, and HTML comments. Strip these before extracting prose (see `clean()` in the script) or they leak into the Overview.
- **Comment-only code blocks.** READMEs sometimes have ``` blocks that are *only* comments (e.g. a malformed-tool-call example). Exclude blocks where ≥ half the lines are comments, and require at least one real command token (`pip`, `npm`, `git clone`, `python`, `uv`, `docker`, …) before putting a block in **Getting Started**.
- **Private repos included via `type=owner`.** Authenticated `gh` returns private repos too — keep them; set `visibility` from the API and link to `[[Private Repos]]`.
- **Forks excluded by `type=owner`.** The vault tracks *own* repos; forks live in a separate index. Don't import forks unless asked.
- **Tech Stack is keyword-detected** from README text — a best-effort signal, not authoritative. `language` from the API *is* authoritative. Don't over-trust detected libraries.

## Conventions to honor (from the obsidian skill)
- Use `[[Wikilink]]` syntax, not Markdown relative links, for in-vault links.
- Follow the vault's MOC pattern: link repos from an index note (`[[Public Repos]]`) rather than only from root.
- Never edit `.obsidian/` or plugin state — that's editor config, not content.

## Related
- `obsidian` — manual note read/search/create/edit + vault-path resolution.
- `github-auth` — ensure `gh` is authenticated first.
- `github-repo-management` — repo lifecycle (clone/fork/create).
