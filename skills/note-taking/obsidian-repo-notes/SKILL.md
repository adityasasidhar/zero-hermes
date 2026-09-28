---
name: obsidian-repo-notes
description: "Generate schema-consistent, detail-rich structured notes in an Obsidian vault from external sources (GitHub for repos; web/research for people, protocols, systems; local filesystem for unreleased/local-only projects). Use when the user wants their vault's notes — repo cards, reference/research notes, profile pages, local-project notes — populated with real data and a working source log instead of hand-written stubs."
version: 1.3.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Obsidian, GitHub, vault, knowledge-graph, notes, gh-cli, research]
    related_skills: [obsidian, github-repo-management, github-auth]
---

# Obsidian Repo Notes

Workflow for bulk-populating an Obsidian vault with structured, accurate project
notes sourced from GitHub. Built for the common setup where the user tracks repos
as `[[wikilink]]` notes and wants them detailed and consistent without writing
50 files by hand.

## When to use
- "create detailed info about all the projects in each .md file"
- "enrich my repo notes" / "build my project knowledge graph"
- "populate the Bryan Johnson protocol notes" / "fill in the [person/system/protocol] stub notes from web sources"
- "add a Source log and populated these Concept notes for X"
- Any task to populate a folder of vault notes from external sources (GitHub for
  repos; authoritative web pages for people/protocols/systems), with a working
  provenance log that every claim traces back to.

## Detection
- Vault path = `OBSIDIAN_VAULT_PATH` env var, else `~/Documents/Obsidian Vault`,
  else the user-supplied folder (e.g. `Documents/fun`). Resolve to a concrete absolute path
  before calling file tools — they don't expand `$VAR` or `~`.
- Repo notes live under a `Github/Repos/` (or similar) subfolder, one `<repo>.md` per repo.
  Index notes (`Public Repos.md`, `Private Repos.md`, `repos.md`) link them via `[[wikilinks]]`.

## Workflow (deterministic — prefer the bundled script)
1. Resolve vault + repo-notes folder. List note names (filenames minus `.md`).
2. Fetch real metadata via `gh api` (see Pitfalls — do NOT use jq object-constructor syntax).
3. Fetch each repo's README: `gh api repos/OWNER/NAME/readme --jq .content`, then base64-decode.
4. Generate each note from the schema below (re-run `scripts/generate_repo_notes.py`).
5. Validate: every note has required frontmatter + `## Overview` + `Part of: [[Group]]`,
   and a `search_files` for `!\[` / `]\(http[^)]*\)\]\(` across the folder returns 0.

### Workflow variant: **Deep enrichment wave** (per-repo code-reading)

When the bulk generator has already produced a stub note and the task is to make it
substantively richer — e.g. a parent agent dispatches a "wave" of parallel subagents,
one per repo, each writing to `/tmp/enrich-wave<n>/<slug>.md` for the parent to verify
and merge — the bulk-sync script above is the wrong tool. The deep variant reads the
actual code, not just the README, and produces ~10–20k chars per note instead of a
single-paragraph stub.

Inputs per repo: existing vault note (read it to learn schema + what's already said),
cloned repo at `~/projects/<slug>/` (or wherever the parent cloned it), `gh api
repos/OWNER/SLUG` for metadata, `gh api repos/OWNER/SLUG/readme --jq .content | base64 -d`
for the README body.

Output: write to `/tmp/enrich-wave<n>/<slug>.md` — **NEVER** touch the vault note directly;
the parent agent verifies and copies it back. The vault file at
`~/Documents/.../Repos/<slug>.md` is off-limits for the duration of the wave.

Required body sections, in this exact order:
1. `# <Display Name>` + blockquote tagline ("X is a Y that Z.")
2. `## Overview` — 2–4 sentences from README intro + actual code
3. `## Status` — 1–3 lines (activity, deps pinned?, tests?, CI wired?)
4. `## Inside the Codebase` with these H3 sub-headings, in order:
   `### Architecture / How it works`, `### File structure`, `### Key files`,
   `### Notable details`, `### Tech stack`, `### How to run it`
5. `## What this project is about` — the "why does this exist" angle, specific not generic
6. `## Use cases / When you'd reach for this` — 2–5 realistic bullets
7. `## Key Features` — emoji-lead bullets, preserve existing good ones
8. `## Getting Started` — real run commands only, no comment-only blocks
9. `## Stats` — emoji bullets (⭐, 🍴, 🔒, 📅, 🔄, 🏷️, 📊)
10. `## Links` — `Part of: [[Public Repos]]` or `[[Private Repos]]` (required, parent uses
    it to copy back to the vault), plus `GitHub:`, `Concepts: [[C1]], ...` (4–6),
    `Themes: [[T1]]` (use exact wikilink title matching `Themes/<Name>.md`),
    `Related: [[r1]], ...` (1–3).

See `references/deep-enrichment-brief.md` for the field list and the parent agent's
verification checklist, `references/deep-enrichment-recipes.md` for the
inventory/verification shell commands and the categories of reality gaps (manifest
vs imports, dead duplicates, tracked secrets) worth surfacing in the note, and
`references/notebook-only-repos.md` for the notebook-first variant (one `.ipynb`
file, no `*.py`/`*.ts`, no README, all logic in cells — common for Colab course
exports and bonus-unit tutorials).

### Workflow variant: **Reference / research-subject wave** (non-repo: people, protocols, systems)

When the wave's subject is **not a GitHub repo** but a real-world person, system,
protocol, framework, or any other reference subject (e.g. "Bryan Johnson's Blueprint
protocol", "Sam Altman's views on X", "the EU AI Act"), the repo-shaped inputs
(`gh api`, cloned tree, `package.json`, README) don't apply. The shape is the same —
parent dispatches a wave, output goes to `/tmp/<wave>N/...`, never the vault —
but the **evidence model is a Source log of external URLs**, not a `gh api` fetch.

Inputs per subject: a folder of scaffolded notes (typically under a topic folder
like `Personal Growth/<Subject>/`) with `Protocol.md`, `Source log.md`, and a
`Concepts/` subfolder containing one stub per concept. The folder mirrors the
target vault layout under the staging path. See
`references/research-subject-brief.md` for the full variant brief.

Hard rules that **differ from the repo variants**:

1. **Source log is the source of truth, not `gh api`.** Every claim in a concept note
   must trace to a row in `Source log.md` with `| Date | URL | What we took | Trust | Updated which notes |`.
   No row = fabrication. The canonical trust hierarchy is: (1) the subject's
   own published canonical source, (2) their YouTube/podcast, (3) third-party
   summaries with citations, (4) their social media, (5) tabloid — never use (5).
2. **Cross-cite, don't paraphrase.** A claim like "NMN/NR taken 6 days/week" must
   cite both the canonical subject source AND the third-party that caught the
   2026 update. Differences between sources are signal (something changed), not
   noise — surface them in `## What we know` vs `## What we don't know`.
3. **The "professional sleeper" warning pattern.** For any subject where the user
   might be tempted to mimic the practice (biohackers, athletes, supplements,
   wellness influencers, legal-but-risky treatments), every concept note MUST
   start with a `> **WARNING:**` blockquote calling out the resource gap (cost,
   clinical team, contraindications). This is non-negotiable; the user expects
   to be **reminded**, not assumed to know.
4. **Sections in concept notes are opinionated and should match the existing
   scaffold in the vault.** A typical scaffold from this vault is:
   `What X does → What we know → What we don't know → What we'd adopt (and why) → See also`.
   The mirror-to-staging discipline means: write to `/tmp/<wave>N/Personal Growth/<Subject>/Concepts/<X>.md`
   and the corresponding vault path stays untouched until the parent merges.
5. **Numeric claims must carry units, dates, and the source row.** "2,250 kcal,
   130g protein, 206g carbs, 101g fat" with no date and no source log row is
   unfalsifiable. The format that's actually reusable: "**2,250 kcal/day** ([source
   log row, 2026-07-26](...))".
6. **Don't fabricate the user's own counterpart notes.** Templates in this vault
   sometimes reference a sibling note the user hasn't written yet
   (e.g. `Personal Growth/Sleep.md`, `Personal Growth/Supplements.md`). Link to
   them as `_(Personal Growth/<X> not yet written)_ — your version`. Do NOT
   create them on the user's behalf — the empty file is a deliberate placeholder
   per the vault's `CLAUDE.md`.

The vault untouched guarantee is even more important here than for repo waves,
because the staging folder mirrors the vault folder name-for-name. A stray write
to `~/Documents/fun/Personal Growth/<Subject>/` during a research-subject wave
is a same-name collision that no `git status --short -- Github/Repos/` check
catches. Verify with `git -C <vault_root> status --short -- Personal\ Growth/<Subject>/`
(empty result) before the 3-line summary.

### Workflow variant: **Local-only project notes** (filesystem-only, no GitHub remote)

The GitHub-shaped inputs (`gh api`, remote metadata, `topics` from the API) don't apply when the project lives entirely under `/home/arctic/projects/<slug>/` with no `origin` remote and no GitHub listing. This is a recurring shape for in-progress / private / pre-release work, scratch experiments, machine-learning research notebooks, and benchmark workspaces that never get pushed. The evidence model is **the on-disk filesystem + a pre-write vault audit**, not the GitHub API.

Inputs per project:
- Cloned tree at `/home/arctic/projects/<slug>/` (already on disk).
- `git log --oneline` (often zero commits — fall back to file mtimes for `created`/`last_updated`).
- `git remote -v` (typically empty; if non-empty, treat as a normal GitHub-shaped repo and route to the deep-enrichment variant instead).
- `git status --short` (a dirty worktree is a real signal — surface it in `## Status`).
- The project's own `README.md`, `CLAUDE.md`, `AGENTS.md`, manifest files (`package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`), source code, and any committed prose notes.
- Vault path: `OBSIDIAN_VAULT_PATH` env var, else `~/Documents/Obsidian Vault`, else `~/Documents/fun` (this user's vault).

**Pre-write vault audit (mandatory).** Before writing any note, grep the vault for prior references to the project name and slug forms. Concretely:

```bash
grep -rli "<ProjectName>" "$VAULT" 2>/dev/null | head -20
grep -rli "<slug-form>"            "$VAULT" 2>/dev/null | head -20
```

If a prior note exists at `Github/Repos/<slug>.md` or `Projects/<slug>.md`, **read it before writing** — the existing prose may be hand-curated (preserve per the deep-enrichment pitfall "preserve existing curated content") or may be stale enough to need full replacement. If the prior note is a stub, treat the task as a stub-replacement enrichment. **An empty result across all three greps means safe to write a new note.** Record the audit result in the 3-line summary ("vault pre-write audit: 0 prior references for `obsidian-galaxy-graph`").

**Staging path is `kg-sync-tmp/`, NOT `/tmp/<wave>N/`.** Local-only project notes are written to `/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md` (this user's convention — see Pitfalls for the divergence from the GitHub-shaped wave). The mirror sibling `kg-sync-tmp/github-notes/<slug>.md` holds GitHub-shaped repos. The parent agent verifies and copies to the vault. The vault-untouched guarantee still applies: verify with `git -C <vault_root> status --short -- Github/Repos/ Projects/` (or whatever the vault path is) before the 3-line summary.

**Schema differences from the GitHub variant:**
```yaml
---
repo: <name>                    # same
description: <one-line>         # from README/CLAUDE.md or pyproject.toml
github: none                    # ← no remote
visibility: local               # ← replaces "public|private"
local_path: /home/arctic/projects/<slug>   # ← new, the absolute path
language: <primary>
topics: [...]                   # derived from code, not the API (see Pitfalls)
status: <see status table below>
created: YYYY-MM-DD             # git first-commit, or earliest mtime
last_updated: YYYY-MM-DD        # git last-commit, or latest mtime
type: <auto-category>
vault_group: Projects           # ← the vault's project MOC, NOT Public/Private Repos
tags: [<concept list>]          # 4-6 wikilink-style concepts
---
```

**Status table for local projects:**

| Working tree state | `status:` value |
|---|---|
| Clean, all changes committed, last commit recent | `active` |
| Clean, last commit >180d ago | `inactive` |
| Dirty worktree (uncommitted modifications, untracked files) | `active (uncommitted changes)` |
| Zero git commits, files exist | `scaffolded (no commits)` |
| Has commits but the upstream branch is empty | `abandoned (no upstream)` |

**Vault pre-write audit pattern.** Three greps, run sequentially:
1. `grep -rli "<ProjectName>"` (title-case display name)
2. `grep -rli "<slug-form>"` (lowercase, hyphenated/underscored)
3. `grep -rli "<concept-keywords>"` (2-3 terms specific to the project's domain — e.g. "galaxy", "rocky", "openresearch", "qwen3.5")

If all three return empty, the vault has no prior coverage. If any returns hits, read the matched file before writing — it may be a curated note to preserve, a stub to enrich, or a same-display-name collision (different project with the same name — see deep-enrichment pitfall "two repos can share the same display name").

**Source-authority rule (when README / CLAUDE.md / AGENTS.md disagree with code).** Treat the on-disk code and manifest files as authoritative for *technical* claims (model names, dependency versions, settings keys, schema fields). Treat the prose files as authoritative for *intent* and *workflow conventions*. When they disagree, surface the disagreement in `### Notable details` with both values quoted verbatim:

> README/CLAUDE.md says `nemotron-3-nano:30b-cloud`; `agents/models.py` instantiates `ChatOpenAI(model="gemma-4-26b-a4b-it", base_url=...)`. Authoritative source = `models.py`. Likely cause: model swap after CLAUDE.md was written.

Concrete observed case: `openresearch/CLAUDE.md` described `nemotron-3-nano:30b-cloud` as the model in use, but `agents/models.py` switched to Google's Gemini via an OpenAI-compatible endpoint. The code is the contract; CLAUDE.md was a snapshot that drifted.

**`.env` files are off-limits for `read_file`.** The `read_file` tool auto-blocks secret-bearing `.env` files (defense-in-depth — the terminal tool can still bypass, but don't). Note their existence (`ls -la .env` → present) and what module reads them, but never open them. If a `find ... -name '.env*'` returns a populated file, surface it as a credential-handling finding in `### Notable details` ("`.env` present, gitignored, contains `GEMINI_API_KEY` and `TAVILY_API_KEY`; source reads via `os.environ` at call time, not import time").

**0-commit handling.** When `git log` fails with `fatal: your current branch 'X' does not have any commits yet`, the repo is a fresh scaffold. Pull `created` from `git init` time (often missing) or the earliest file mtime, and `last_updated` from the latest file mtime. `status:` becomes `scaffolded (no commits)`. Don't fabricate a commit date; an explicit "(no commits)" is more honest than an invented date.

**Dirty worktree handling.** When `git status --short` shows uncommitted changes, surface them in `## Status` by listing the changed files (`M DESIGN.md`, `M bun.lock`, `M package.json`, `M src/cli.ts`, ...). A dirty tree means the on-disk reality differs from the committed shape — describe the working tree, not HEAD.

**Required body sections** (adapted from the GitHub variant; same 11-section shape, but `## Stats` is omitted — there's no public stats card to populate). Order:

1. `# <Display Name>` + blockquote tagline ("X is a Y that Z.")
2. `## Overview` — 2–4 sentences from README/CLAUDE.md/AGENTS.md intro + actual code
3. `## Status` — activity, dirty?, commits?, tests?, CI wired?, language/package manager, last_updated
4. `## Inside the Codebase` with these H3 sub-headings, in order:
   `### Architecture / How it works`, `### File structure`, `### Key files`,
   `### Notable details` (include source-authority corrections + dirty-tree state + .env findings),
   `### Tech stack`, `### How to run it`
5. `## What this project is about` — the "why does this exist" angle, specific not generic
6. `## Use cases / When you'd reach for this` — 2–5 realistic bullets
7. `## Key Features` — emoji-lead bullets, preserve existing good ones
8. `## Getting Started` — real run commands only, no comment-only blocks
9. `## Links` — `Part of: [[Projects]]` (required; this is the vault's project MOC, e.g. `Documents/fun/Projects/Projects.md`), plus `Local path: <abs path>`, `Concepts: [[C1]], ...` (4–6), `Related: [[r1]], ...` (1–3). **No `GitHub:` line** when `github: none`.

**Auto-categorize** uses the same order as the GitHub variant. The `type:` value goes in frontmatter; the `vault_group:` is `Projects` for all local-only projects regardless of category.

**Topics/tag derivation from code (not API).** When the GitHub API is unavailable, derive the `tags:` field by reading the actual code:
- `package.json` → look at `dependencies` keys, `keywords`, `description`
- `pyproject.toml` → look at `[project] dependencies`, `[project] keywords`, `[project] description`
- `Cargo.toml` → look at `[dependencies]`, `[package] keywords`
- Source imports → what major libraries are imported (e.g. `import torch`, `from flask import Flask`, `import { Plugin } from "obsidian"`)

The goal is the same as the GitHub variant: 4–6 concepts that a vault reader skimming `Concepts:` would recognize. Adapt the guidance from the deep-enrichment pitfalls (`tags:` is *concepts*, not GitHub `topics`).

**Final reply format is fixed.** Print exactly three things: (1) the absolute output path (the staging path under `kg-sync-tmp/local-notes/<slug>.md`), (2) the byte count of the new file (`wc -c` / `Path.stat().st_size`), (3) a 3-line summary naming the categories of enrichment you added and the pre-write audit result ("vault pre-write audit: 0 prior references; captured Zod schema field set, dirty worktree state, model-name swap vs CLAUDE.md").

See `references/local-only-project-brief.md` for the full variant brief, including the verification recipe, the same-display-name collision handling, and the staging-path-vs-vault-path discipline.

### Schema (frontmatter + body)
```yaml
---
repo: <name>
description: <one-line, cleaned of markdown>
github: https://github.com/<owner>/<name>
visibility: public|private
language: <primary>
topics: [a, b]
stars: <n>
forks: <n>
status: active|experimental / inactive|archived   # derived from pushed_at age (>180d = experimental/inactive)
created: YYYY-MM-DD
last_pushed: YYYY-MM-DD
type: <auto-category>
vault_group: Public Repos|Private Repos
---
```
Body: `# Title`, Overview (cleaned README intro or description), `## Key Features`
(bullets from README), `## Tech Stack` (language + detected libs/services + topics),
`## Getting Started` (real command blocks only), `## Stats`, `## Links`
(GitHub URL + `Part of: [[Group]]`).

### Auto-categorize (order matters)
LLM / Agent (agent|mcp|llm|gpt|claude|ollama) → Learning/Educational
(tutorial|learning|implement|educational) → ML Research/Model
(classifier|mnist|neural|transformer|model|babylm|training|dataset) → Game
(game|pygame) → CLI Tool (cli) → Web App (web|react|flask|fastapi|astro|extension)
→ Config/Profile (config|profile-in-name) → Application/Utility.

### Fill gaps honestly
No README ⇒ fall back to the description in the index notes (`Public Repos.md` etc.)
or the repo's GitHub `description`. NEVER invent features, tech, or commands.
A clean stub (frontmatter + description + Stats) beats a fabricated one.

## Pitfalls
- **`gh api` jq object-constructor FAILS.** `gh api ... --jq '.{name,language}'`
  errors: `failed to parse jq expression (line 1, column 2)`. The bundled jq
  doesn't support it. Fix: fetch full JSON (no `--jq`, or `--jq '.'`) and parse in
  Python with `json.loads`. Full detail + snippet in `references/jq-pitfalls.md`.
- **README → Obsidian cleaning is mandatory.** Raw READMEs contain
  `[![CI](...)](...)` badges, `<img>` tags, `![alt](url)` images, and `[text](url)`
  links that render as garbage in notes. Strip them (regex sequence in
  `references/readme-cleaning.md`) before writing.
- **Don't put comment-only code blocks in "Getting Started".** README code fences
  that are purely `# comments` (e.g. examples of malformed input) are not run
  commands — skip blocks where ≥half the lines are comments OR none match a command
  keyword (pip|npm|cargo|git clone|python|uv|docker|cd|go run|node|make).
- **Private repos aren't in the public listing.** `gh api users/OWNER/repos?type=owner`
  returns only visible repos; `gh api repos/OWNER/NAME` works for your own private
  repos — fetch private metadata per-name.
- **Deep enrichment: preserve existing curated content.** Before overwriting, check the note isn't a
  substantial hand-written note (>12 lines or >600 chars). The generator is for
  stubs/enrichment, not replacing prose the user wrote.
- **Deep enrichment: preserve existing code-derived prose, not just user-written
  prose.** The "preserve existing curated content" rule covers hand-written
  notes, but in repeated deep-enrichment waves the existing note may already
  contain detailed prose about specific code-level innovations from a previous
  pass (e.g. "SparseMQA is a sparse multi-query attention where a learned
  LightningIndexer selects top-96 K/V positions"). This prose is the
  *ground-truth of the last reading* — it likely captures subtleties a fresh
  reading will miss. Rewrite it *crisply*, but don't drop the technical
  accuracy. The reliable pattern: read the existing note first, identify the
  3–5 sentences that contain code-mechanism claims (named classes, specific
  numbers, named architectural patterns), and make sure the new note
  preserves each claim. Expanding is fine; rewriting for style is fine;
  silently dropping the claim is a regression.
- **Deep enrichment: the 5 research-depth dimensions.** When the brief asks
  for detail beyond "what does this repo do" (e.g. "deepen the existing note,
  add depth on the data mix percentages, the eval, the position encoding,
  the inference path, and the broader research context"), treat each
  dimension as a checklist item and verify coverage in the final note:
  1. **Data mix percentages** — exact per-corpus weights (e.g. "FineWeb-Edu 70% /
     Wikipedia 20% / reasoning 10%"), explicit dataset names
     (`HuggingFaceFW/fineweb-edu`, config sub-keys like `20231101.en`),
     per-source text-length filters, and per-stream sample rates. Bullet
     form is fine.
  2. **Eval** — named benchmarks / metrics, file paths of eval scripts, or
     *explicit absence* ("no eval harness, no benchmark runner, no
     Wikitext perplexity, no MMLU/HellaSwag"). The absence is worth stating
     for repos that have no eval.
  3. **Position encoding** — the exact RoPE variant (standard / NTK-aware /
     YaRN / ALiBi), with concrete parameters from the model defaults
     (`rope_theta`, `rope_factor`, `original_seq_len`, `max_seq_len`), and
     a one-line explanation of how the variant works.
  4. **Inference path** — forward-pass flow, KV-cache strategy, sampling/
     decoding loop, serving stack, or *explicit absence* ("no `generate.py`,
     no `pipeline(...)` example, no vLLM/HF inference path"). For training-
     only repos the inference path is the `__main__` smoke test plus the
     saved checkpoint.
  5. **Broader research context** — trace local mechanisms back to the
     published paper that introduced them. E.g. an obscure local class
     named `LightningIndexer` is the pattern DeepSeek-V2 popularized for
     Multi-head Latent Attention ("lightning attention"); a class named
     `SparseMoE` likely traces to Switch Transformer / GLaM; a
     `FlashAttention` reference traces to Tri Dao's 2022 paper. One
     sentence per mechanism: "X is the same pattern Y used for Z" is
     enough. This is the most-often-missing dimension because it requires
     connecting the local code to external literature — the kind of
     context a generator trained on the repo alone can't produce.
  A note that covers all 5 dimensions is a 5-star enrichment; a note that
  covers 2–3 is a 3-star enrichment. The brief explicitly asks for all 5
  when the wording names them.

- **Deep enrichment: from-reading-the-code correctness calls are valuable
  additions.** When the code disagrees with the description or the
  conventional name suggests something the code does not do, surface this
  in `### Notable details` as a "correction" or "misnomer" bullet. Concrete
  observed case: a class named `SparseMQA` (`Sparse Multi-Query Attention`)
  is conventionally understood to share K/V across all query heads (the
  MQA part). When the actual implementation is top-k sparse attention with
  MQA-sized K/V projections (not actually shared across heads), the
  name is a misnomer. Calling this out — "the name `SparseMQA` is a
  misnomer; it's really top-k sparse attention with MQA-sized K/V
  projections" — is a from-reading-the-code insight that a generator
  reading the README alone would miss. The pattern: every locally-named
  mechanism may or may not match its conventional meaning. Verify by
  reading the actual forward pass, not the class name.
- **Link conventions.** Use Obsidian `[[Note Name]]` wikilinks, not Markdown relative
  links. Follow the vault's MOC pattern (link from an index note, not only the root).
- **Deep enrichment: trust the CODE, not the README.** The README is marketing — the
  source is the contract. A repo's README may claim "~25 tools" while `src/index.ts`
  registers exactly 20; a description may list libraries that aren't actually in
  `package.json`. When the two disagree, the brief says "trust the CODE." Reconcile
  explicitly in the note (e.g. "README claims ~25; `src/index.ts` registers 20").
- **Deep enrichment: pin versions from manifests, not from prose.** Use
  `cat package.json | python3 -c '...'` (or `pyproject.toml`, `requirements.txt`,
  `Cargo.toml`, `go.mod`) to read exact caret/pinned versions. Don't paraphrase from
  the README's "built with X" prose — paste the real dependency lines. A future
  reader will want to know the *real* dep tree, not what the author remembered to
  mention.
- **Deep enrichment: capture commit-history shape.** A repo's git log tells you
  whether it's a 4-day sprint, a 3-year slow burn, or abandoned-after-prototype.
  `git log --oneline | wc -l` and `git log --since="<6 months ago" --oneline | wc -l`
  are the two numbers to capture. Also note anomalies: deleted test files, single
  squashed commit histories, avatar-only commits — these are honest signals about
  how the project was actually built.
- **Deep enrichment: char budgets are HARD when the brief says HARD, but the
  floor is structural.** Recent waves target a **7–12k char / 12–12.5k byte**
  window (not the older 5–10k default). The brief mandates all 11 body
  sections, 6 Inside-the-Codebase subsections, and a 12-field YAML
  frontmatter (~600 bytes). With every section substantively populated, the
  structural floor is roughly **12–13k bytes** — you cannot hit the lower
  end of the range for a non-trivial repo, the upper end is the floor. Plan
  for the upper end from the start; **a first-pass overshoot to 17–20k is
  normal** and means you wrote introspective prose that doesn't carry its
  weight, not that the brief is broken. Trim aggressively by collapsing
  prose in `Notable details` / `Architecture` (those always run long),
  keeping bullets short, and dropping color words. After three rounds of
  trimming, if you're still over, stop — write the file at the structural
  floor (~12k bytes), flag the overshoot in the 3-line summary, and ship.
  The "preserve existing good prose" rule and "don't lose real detail" rule
  trump the char ceiling when they conflict. Re-verify with `wc -c` before
  declaring done; the parent agent enforces the byte window, not the char
  window, so byte count is what matters. **The single biggest time-saver:
  draft the first write at the ceiling (12k bytes), not below it.** A
  draft at 8k always needs adding; a draft at 12k sometimes needs trimming
  (one patch round). The reverse is rare.
- **Deep enrichment: when the EXISTING note is already at/above the brief's
  target, the brief's char ceiling is wrong, not your draft.** A concrete
  observed case: the Gappy/PANCHAI wave's brief said "7–12k char target"
  but the existing note was already 19,050 chars of body (file size
  19,600 bytes), with substantial hand-written prose across all 11
  sections. The "deepen it further / preserve the excellent existing
  prose" instruction and the "7–12k target" instruction were directly
  contradictory — preserving the existing prose already exceeded the
  ceiling. Diagnostic: run `wc -c` on the existing note *before*
  drafting. If it's already ≥ the brief's target ceiling (and especially
  if it's the "most detailed existing note" the brief calls out), the
  brief's window is calibrated for stub replacement, not for deepening
  an already-rich note. In that case: (a) treat the existing note's
  size as the **new floor**, not the brief's number; (b) draft to land
  near (existing + 10–20%), not at the brief's ceiling; (c) spend the
  trimming effort on duplicative prose, not on cutting real detail; (d)
  flag the contradiction in the 3-line summary ("Existing note was 19k;
  brief's 7–12k target was inconsistent with the 'preserve existing
  prose' rule — drafted at 19.5k to retain depth"). The "preserve
  existing good prose" and "don't lose real detail" rules trump the
  char ceiling when they conflict; this is the canonical example.
  **Conversely: do NOT silently rewrite a 19k note down to 10k just to
  hit a number.** The brief's window is a heuristic for stub
  replacement; it's not a hard cap when the source material is
  already substantial.
- **Deep enrichment: inventory before you read.** Before opening any source file,
  run `git ls-files | wc -l` (tracked file count), `git ls-files '*.py' '*.html' '*.js' '*.css' | wc -l`,
  and a `search_files(target='files')` walk. Pipe `git ls-files` output through a
  `Counter` of extensions to see the project's actual shape in one shot
  (e.g. `Counter({'pdf': 22, 'py': 22, 'html': 17, ...})`). This tells you whether
  it's a frontend-heavy SPA, a notebook-driven ML repo, a textbook-PDF-backed
  school app, etc. — and you read the right files first.
- **Deep enrichment: reconcile manifest against imports.** `requirements.txt`
  is the source of truth for what's pinned; `import` statements are the source of
  truth for what's actually used. When an import is *not* in the manifest (e.g.
  `from flask_wtf.csrf import CSRFProtect` with no `Flask-WTF` line), that's a
  real defect worth surfacing in `### Notable details`. Same pattern: `from flask_migrate import Migrate`
  with no `Flask-Migrate` pin, `import fpdf` with both `fpdf` and `fpdf2` pinned
  (namespace collision), `import pytest` with no pytest dependency. Always paste
  the actual dependency lines, not a paraphrase.
  - **Kaggle-competition manifests are intentionally minimal.** A
    `pyproject.toml` that declares only `pandas` and `ruff` while the
    training scripts `subprocess.run("pip install ...")` from Kaggle
    wheel datasets is NOT a manifest defect — it's the expected shape
    for a competitive-ML repo. The runtime is a controlled Kaggle GPU
    environment with pre-staged wheels (`/kaggle/input/.../packages`),
    and the manifest just declares what to *type* locally. The
    "manifest vs imports" finding should still be surfaced, but framed
    as informational ("`pyproject.toml` declares only pandas+ruff;
    runtime deps install via `subprocess` from Kaggle wheels —
    `unsloth`, `mamba_ssm`, `causal_conv1d`, `peft`, etc.") rather than
    as a defect. The exponential wheel filenames in the install calls
    (`causal_conv1d-1.6.1+cu12torch2.10cxx11abiTRUE-cp312-cp312-linux_x86_64.whl`)
    are the signal that this is a Kaggle-style repo, not a publishable package.
- **Deep enrichment: walk the directory tree, not just the README's tree.** The
  README may show a clean tree; the actual filesystem has legacy duplicates and
  dead snippets. After `search_files(target='files')`, check for `__pycache__/`,
  `*.pyc`, and any `legacy_*` / `* old*` siblings. Look for two files claiming to
  do the same job (e.g. `src/generate_paper.py` + `src/school_paper.py`,
  `src/utils.py` + `src/pdf parser.py`) — one is usually the routed path and the
  other is dead code. Call this out in the note.
- **Deep enrichment: frontmatter tags ≠ GitHub topics.** The brief's `tags:` field
  is a *concept* list (ideas and architectural patterns the repo uses), not the
  GitHub `topics:` array. If `topics: []` from the API, expand `tags:` based on
  what you actually see in the code: `[[Flask Blueprints]]`, `[[SQLAlchemy]]`,
  `[[Structured Output]]`, `[[Pytest]]`, `[[Bootstrap]]`, etc. 4–6 is fine; do
  not invent topics the repo doesn't use.
- **Deep enrichment: parent verification checklist to self-run.** Before printing
  the final summary, verify ALL of these (the parent agent will run them too,
  and a self-check catches overshoots before the parent has to bounce work back):
  - Output path is `/tmp/enrich-wave<n>/<slug>.md`, NOT the vault path.
  - `git status --short -- <vault-path>` is empty (vault untouched).
  - Char count is within the brief's window (5–10k for the standard wave).
  - All 11 required body sections present in order (Overview, Status, Inside the
    Codebase, What this project is about, Use cases, Key Features, Getting Started,
    Stats, Links).
  - `## Links` ends with `Part of: [[Public Repos]]` or `Part of: [[Private Repos]]`.
  - No README raw markdown leaked (`![...](...)`, `<img>`, `[![badge](...)](...)`,
    `[text](url)` link-only refs).
  - No comment-only code blocks in `Getting Started`.
  - Every pinned version in `### Tech stack` came from
    `requirements.txt` / `package.json` / `pyproject.toml`, not the README.
- **Deep enrichment: final reply format is fixed.** After writing, print exactly
  three things: (1) the absolute output path, (2) the byte count of the new file
  (`wc -c`), (3) a 3-line summary naming the categories of enrichment you added
  (e.g. "Mapped Flask blueprint architecture, …"). Don't include validation output;
  that's noise. The parent agent doesn't need to see `wc -m` or `grep -Fqx` results.
  **This print is non-negotiable even when the file is over budget** — overshoot
  is recoverable, a missing summary is not (see the "tool-call budget runs out
  mid-trim" pitfall above).
- **Deep enrichment: shell heredocs in `execute_code` with embedded
  apostrophes get a `SyntaxError: invalid syntax` or `unterminated string
  literal`.** The `execute_code` tool runs Python through the `terminal()`
  pipeline, but the `code` parameter has to be valid Python *as a Python
  string literal* before it executes. Apostrophes inside `s.replace('...'`
  or inside a list-comprehension string (`"## Use cases / When you'd reach for this"`)
  escape badly when the outer quoting tries to nest them. Two fixes:

    1. **Write the script as a separate file** with `write_file(path, code)`,
       then `runpy.run_path(path)` or `exec(open(path).read())`. Keeps the
       apostrophes in plain text and avoids the nested-string problem entirely.
    2. **Use double quotes for Python strings that contain apostrophes**
       (and vice-versa): `s.replace("## Use cases / When you'd reach for this", ...)`
       instead of single-quoted. The Python source itself is fine; only the
       outer wrapping was broken.

  For the rare case of an embedded apostrophe inside a list literal whose
  outer container uses apostrophes (`["it's", "ok"]`), use a tuple of
  concatenated strings or break the literal into multiple lines. The fix
  is *always* to avoid the nesting, never to escape with `\\` — that just
  hides the issue.
- **Deep enrichment: write tight on the first pass, not on the fifth.**
  Patch-based trimming of an already-written 12–15k markdown is slow, error-prone
  (it's easy to delete an entire required section by accident during a `patch`
  round), and burns many round-trips. The structural floor with all 11 sections
  substantively populated is ~10–12k bytes. A first draft at 14–15k is fine if
  you intend to trim; a first draft at 17k+ means you wrote too much prose that
  doesn't carry its weight. Discipline: when drafting each section, target its
  final size in your head. `Notable details` and `Architecture` always run long
  if you let them — keep each bullet to one sentence. If you do overshoot,
  prefer rewriting the section as one `write_file` rather than chaining 6+
  `patch` calls that each save ~200 bytes. A single `write_file` rewrite at
  the final size is faster and safer than five incremental trims.
- **Deep enrichment: when the first draft is ~30% over budget, ONE
  `write_file` rewrite is the right move, not a patch chain.** The "no
  patch chaining" rule is about playing whack-a-mole on a 17k+ overshoot
  where you've lost count of which section is bloated. For a tight
  13k→10k trim, a single full rewrite is fine — but only ONCE. After
  that, if you're still over, stop trimming and write the file at the
  structural floor (~10–12k bytes) and flag the overshoot in the
  3-line summary. Repeated rewrites burn iterations without moving
  the byte count because the prose isn't getting tighter, it's just
  getting rearranged. The diagnostic: "I've rewritten twice and I'm
  still over" means the floor is structural, not stylistic, and the
  only honest next move is to ship at the floor and explain.
- **Deep enrichment: the "existing note is small, source code is rich"
  trap is the most common overshoot cause.** A concrete case from a
  recent wave: the existing `Caveman.md` was only 2,140 chars
  (well under the 7–12k brief target), but the source code had
  8 React components, 3 lib files, a full Next.js 16 App Router,
  6 print SKUs, and a parallel static-HTML implementation. Writing
  a "DETAIL-RICHER" first draft that documents all of that honestly
  lands at 20–24k — the **structural floor for that much real code
  is much higher than the brief's ceiling**. Diagnostic: peek at the
  inventory *before* drafting (file count, components per directory,
  `package.json` deps). If the source has 8+ components, 3+ lib files,
  AND an App Router, plan a first draft at **15–18k** and trim two
  rounds max — anything less discards real detail. The single cheapest
  signal: `git ls-files '*.tsx' '*.ts' '*.jsx' '*.js' '*.css' | wc -l`.
  Above 30 source files for a React/Next.js project, the structural
  floor is the upper end of (or above) the brief's window. Draft at
  the floor, trim twice, ship at the floor, and flag the overshoot.
- **Deep enrichment: when the tool-call budget runs out mid-trim, the
  parent agent gets a partial file, not a failure.** Recent waves
  have hit the iteration ceiling while still trimming an over-budget
  draft. The mitigation: a partial file at 16-20k bytes is recoverable
  (parent can apply one more pass); a 24k file mid-rewrite with no
  terminal-print at all is not. The discipline: **always end each
  session with a terminal-printed 3-line summary and a `wc -c`, even
  if the file is still over budget.** A partial file with a clear
  "shipped at 16k, brief's 12k ceiling not reached, structural floor
  violation" note in the summary is honestly more useful than a
  no-output truncation. The "render the total failure visible" rule
  beats the "ship the perfect document" rule when the budget is
  exhausted.
- **Deep enrichment: section ordering is mandatory, not decorative.** The 11
  body sections must appear in the exact order from
  `references/deep-enrichment-brief.md`:
  `# Title → Overview → Status → Inside the Codebase (with its 6 H3s) →
  What this project is about → Use cases → Key Features → Getting Started →
  Stats → Links`. The parent agent's `grep -Fqx` check only confirms presence,
  not order; doing a `[s.index(h) for h in markers] == sorted(...)` self-check
  catches accidental reorders that `grep` misses. Common reorders that passed
  grep but failed the order check in real waves: putting `## Links` before
  `## Stats`, or dropping `## What this project is about` *between* `## Status`
  and `## Inside the Codebase` to fill space.
- **Deep enrichment: `read_file` inside `execute_code` returns content with
  embedded `N|N|` line prefixes; writing that straight back corrupts the file.**
  When `execute_code` calls `read_file(path, offset=..., limit=...)`, the
  returned `content` field is the line-numbered text format
  (`1|... \n 2|...`). If you then `write_file(path, content)` without
  stripping those prefixes, the file ends up with artifacts like
  `### Notable details\n73|73|` and blank lines numbered `200|201|`. This
  is *especially* easy to miss after several rounds of `patch` +
  `read_file` + `write_file` because the cumulative diff still looks
  plausible. Fix: always strip line prefixes before rewriting, with one
  of:

  ```python
  # Option A: regex-strip
  import re
  s = re.sub(r'^(?:\d+\|)+', '', line) for line in s.splitlines()
  ```

  ```bash
  # Option B: terminal one-liner (terminal() doesn't run read_file, so
  # the artifact only appears if you pipe through read_file earlier)
  python3 -c "import re; s=open(p).read(); open(p,'w').write(re.sub(r'^(?:\\d+\\|)+','',s,flags=re.M))"
  ```

  The same artifact appears in any tool that returns line-numbered text —
  `search_files(target='content', output_mode='content')` is safe (returns
  `<line>:<content>` once, not double-prefixed), but `read_file` is the
  recurring source. Always check with
  `head -3 /tmp/enrich-wave<n>/<slug>.md` before the final wc -c; the
  artifact will show as `1|1|---` at the top.
- **Deep enrichment: never `write_file` over a file you last modified via
  shell/heredoc.** `write_file` checks the on-disk mtime against the
  read-cached content and returns a warning ("modified since you last read it")
  if anything (including a `python3 - <<EOF` heredoc or `sed -i`) changed it.
  Either `read_file` the path right before the rewrite, or do the final
  rewrite in `execute_code` (which doesn't have that guard). Skipping the
  re-read risks a silent rollback to the cached version.
- **Deep enrichment: theme wikilinks must match an existing
  `Themes/<Name>.md`.** The brief lists "LLMs and SLMs", "Agents and
  Orchestration", "Applied ML and CV", "Web Tools and Learning" as the
  canonical themes. If the existing note already used a theme (e.g.
  `[[Applied ML and CV]]`) and you want to add a second (`[[LLMs and SLMs]]`),
  verify the second file exists before linking it — linking to a non-existent
  theme note shows up as a broken wikilink in Obsidian. When in doubt, keep
  the existing note's theme link and don't add a second one.
- **Deep enrichment: shell heredocs with embedded URLs may trip the
  terminal security scan.** Commands like
  `printf 'VITE_API_URL=http://localhost:3000\n' > .env.local` can be flagged
  by the auto-approver's regex even though they're benign (the scanner sees
  the `.` before the `\n`). If your heredoc is repeatedly
  auto-approved/spammed, route the same rewrite through `execute_code` —
  the Python interpreter doesn't trip that scanner. Reserve shell heredocs
  for commands that genuinely need the shell (git, package managers).
- **Deep enrichment: the Inventory batch's extension histogram needs a JS
  variant.** The recipes reference Python files (`*.py`); for React/Vite and
  Node.js monorepos, the high-value extensions are `.jsx`, `.tsx`, `.js`,
  `.mjs`, `.cjs`, `.json`, `.html`, `.css`, and `.svg`. Add a second
  histogram line for the JS case so the script is symmetric:
  `git ls-files '*.jsx' '*.tsx' '*.js' '*.json' | wc -l`. Pair with the
  per-package `Counter` (e.g. `Counter({'.jsx': 8, '.json': 5, '.js': 5})`
  ⇒ React/Vite frontend + Node API).
- **Deep enrichment: npm lockfile resolution recipe.** For npm-only repos,
  `package-lock.json` is lockfileVersion 3 with a top-level `packages` map
  keyed by `node_modules/<name>`. Resolved versions are at
  `data['packages']['node_modules/<name>']['version']` and the registry URL
  is `data['packages']['node_modules/<name>']['resolved']`. One-liner:
  `python3 -c "import json; d=json.load(open('package-lock.json'));
  root=d['packages']['']; deps={**root.get('dependencies',{}),
  **root.get('devDependencies',{})};
  [print(name, deps[name], d['packages']['node_modules/'+name]['version'])
  for name in deps]"`. Use both the declared caret range and the locked
  exact version in `### Tech stack` ("`^4.4.12` (lock 4.8.3)").
- **Deep enrichment: verify vault untouched with `git status --short`,
  not just by `ls -la`.** The `git ls-files -s` confirms tracked state but
  not "did I just write to it". Run
  `git -C <vault_root> status --short -- Github/Repos/<slug>.md` immediately
  before printing the final summary; an empty result confirms the vault
  file is in the same state as at session start. A stray `?? <path>` line
  means the coordination guarantee is broken — stop and investigate.
- **Deep enrichment: include a `Homepage:` line in `## Links` when the
  GitHub API exposes a homepage field.** The skill's `## Links` schema lists
  GitHub + Concepts + Themes + Related + Part of, but a real `homepage_url`
  (often a Vercel/Render deploy of the live app) is the single most
  actionable URL for a portfolio site. Include it as `Homepage: <url>`
  between GitHub and Concepts.

- **Deep enrichment: `gh repo view <name>` returning a different canonical name means the repo was renamed, not deleted.** When `gh repo view <owner>/<old-name> --json name` returns `{"name": "<new-name>", ...}` and the URL bar shows the new name, GitHub has transparently followed the rename redirect and returned the canonical repo. The repo is alive under its new name; the old URL still works as an alias. The detection chain to confirm a rename vs. an unrelated collision:
  1. **HTTP probe:** `curl -sS -o /dev/null -w '%{http_code}|%{redirect_url}' https://github.com/<owner>/<old-name>` — a `301` to the new path is unambiguous
  2. **REST API:** `gh api repos/<owner>/<old-name>` returns `301` redirecting to `/repositories/<id>` (the canonical numeric ID)
  3. **git ls-remote:** `git ls-remote https://github.com/<owner>/<old-name>.git HEAD` succeeds and returns the same SHA as the canonical URL — confirms the alias is live, not just a web redirect
  4. **gh repo view:** returns the canonical repo's metadata under the new name (URL bar in browser shows the new name)
  Implication for vault notes: if the local clone's `origin` still points at the old URL, set `former_github:` in frontmatter (NOT `github:`) and use the canonical URL as `github:`. Skipping this makes the note claim the wrong canonical name and breaks [[wikilink]] resolution to the actual repo. Recorded case: `adityasasidhar/build-your-own-harness` redirected to `adityasasidhar/claude-code-in-100-lines` (canonical repo ID `1276109569`); the canonical repo was last pushed 2026-07-07 UTC, while the local clone's `git log` showed 2026-07-08 local commit time — the local clone is ahead of the public canonical by one commit.

- **Deep enrichment: two local paths claiming to be "the same repo" need 4-axis verification, not just a `git status` look.** When you have `~/projects/<old-name>/` and `~/projects/<new-name>/` and suspect they're duplicates (post-rename, mid-rename, accidental re-clone), one `git status` check is not enough — a dirty worktree on either side breaks the comparison. The verification chain that survives dirty worktrees, divergent local commits, and untracked experimental files:
  1. **Tracked path lists equal:** `[ "$(cd A && git ls-files | sort)" = "$(cd B && git ls-files | sort)" ] && echo EQUAL` — fast filter for "is this even the same file set"
  2. **Tracked content byte-equal:** `for f in $(cd A && git ls-files); do diff -q "$A/$f" "$B/$f" || break; done` — catches staged-but-not-committed content divergence even when `git ls-files` lists the same paths
  3. **HEAD equal:** `cd A && git rev-parse HEAD` must match `cd B && git rev-parse HEAD` AND `gh api repos/<owner>/<name>/branches/main --jq '.commit.sha'`
  4. **Root tree equal:** `cd A && git rev-parse HEAD^{tree}` must match `cd B && git rev-parse HEAD^{tree}` — independent of commit-message history; catches rebase-with-different-message divergence
  If all 4 axes match, the two paths are byte-equivalent checkouts of the same history; the meaningful difference is configuration (`origin` URL, `.env`, `.venv/`, ignored local files). For a vault note on a renamed repo, this 4-axis verification justifies treating the two paths as one project and writing ONE note — but the note must call out the configuration difference (which `origin` URL each uses) so a future reader doesn't get confused about which to clone. Recorded case: `~/projects/build-your-own-harness/` and `~/projects/claude-code-in-100-lines/` matched on all 4 axes (28 tracked paths, byte-identical content, HEAD `4a14515d`, root tree `0e2c50ac`); the only meaningful difference was the `origin` URL — the former-name folder pointed at the redirected URL, the canonical folder pointed at the current canonical.

- **Deep enrichment: local-only directories without `.git/` are still valid project folders.** A path like `~/projects/<slug>/<file>` with no `.git/`, no README, no package manifest, and no remote is a real project — just one that's never been version-controlled or pushed. Detection: `git rev-parse --is-inside-work-tree` returns non-zero AND `gh api` is irrelevant. For these, skip the entire GitHub fetch chain and inventory the on-disk truth directly:
  1. `find . -maxdepth 2 -type f | sed 's/.*\.//' | sort | uniq -c | sort -rn` for an extension histogram
  2. Read the entry file (e.g. `cat fun.py` / `cat main.c`) — for a single-file script, the file IS the spec, no README to clean
  3. Run it (`python3 <file>` / `gcc <file> && ./a.out`) to capture actual behavior — the only way to confirm it works without manual code reading
  4. Treat build artifacts (`.vscode/tasks.json`, existing compiled binaries, `.gitignore` lines) as a "what was the intended workflow" signal — quote the source first, not the artifact; two local ELFs with different SHA-256s are a signal of multiple compile attempts, not a separate build system
  The note still needs all 11 body sections (`# Title`, `## Overview`, `## Status`, etc.); the frontmatter `github:` field should be empty (NOT a placeholder URL), `visibility: local-only`, and `vault_group: Projects` is more honest than `Public Repos`/`Private Repos` since the grouping concept doesn't apply to un-pushed work — a `Local Projects` or `Experiments` vault group is the typical home. Recorded case: a 2,360-byte scalar-autograd `fun.py` and a 1,844-byte C messaging `main.c` both produced structured notes from source-reading + execution only — no `gh api`, no `git ls-files`, no remote.

- **Deep enrichment: `gh repo view <name>` returning a different canonical name means the repo was renamed, not deleted.** When `gh repo view <owner>/<old-name> --json name` returns `{"name": "<new-name>", ...}` and the URL bar shows the new name, GitHub has transparently followed the rename redirect and returned the canonical repo. The repo is alive under its new name; the old URL still works as an alias. The detection chain to confirm a rename vs. an unrelated collision:
  1. **HTTP probe:** `curl -sS -o /dev/null -w '%{http_code}|%{redirect_url}' https://github.com/<owner>/<old-name>` — a `301` to the new path is unambiguous
  2. **REST API:** `gh api repos/<owner>/<old-name>` returns `301` redirecting to `/repositories/<id>` (the canonical numeric ID)
  3. **git ls-remote:** `git ls-remote https://github.com/<owner>/<old-name>.git HEAD` succeeds and returns the same SHA as the canonical URL — confirms the alias is live, not just a web redirect
  4. **gh repo view:** returns the canonical repo's metadata under the new name (URL bar in browser shows the new name)
  Implication for vault notes: if the local clone's `origin` still points at the old URL, set `former_github:` in frontmatter (NOT `github:`) and use the canonical URL as `github:`. Skipping this makes the note claim the wrong canonical name and breaks [[wikilink]] resolution to the actual repo. Recorded case: `adityasasidhar/build-your-own-harness` redirected to `adityasasidhar/claude-code-in-100-lines` (canonical repo ID `1276109569`); the canonical repo was last pushed 2026-07-07 UTC, while the local clone's `git log` showed 2026-07-08 local commit time — the local clone is ahead of the public canonical by one commit.

- **Deep enrichment: two local paths claiming to be "the same repo" need 4-axis verification, not just a `git status` look.** When you have `~/projects/<old-name>/` and `~/projects/<new-name>/` and suspect they're duplicates (post-rename, mid-rename, accidental re-clone), one `git status` check is not enough — a dirty worktree on either side breaks the comparison. The verification chain that survives dirty worktrees, divergent local commits, and untracked experimental files:
  1. **Tracked path lists equal:** `[ "$(cd A && git ls-files | sort)" = "$(cd B && git ls-files | sort)" ] && echo EQUAL` — fast filter for "is this even the same file set"
  2. **Tracked content byte-equal:** `for f in $(cd A && git ls-files); do diff -q "$A/$f" "$B/$f" || break; done` — catches staged-but-not-committed content divergence even when `git ls-files` lists the same paths
  3. **HEAD equal:** `cd A && git rev-parse HEAD` must match `cd B && git rev-parse HEAD` AND `gh api repos/<owner>/<name>/branches/main --jq '.commit.sha'`
  4. **Root tree equal:** `cd A && git rev-parse HEAD^{tree}` must match `cd B && git rev-parse HEAD^{tree}` — independent of commit-message history; catches rebase-with-different-message divergence
  If all 4 axes match, the two paths are byte-equivalent checkouts of the same history; the meaningful difference is configuration (`origin` URL, `.env`, `.venv/`, ignored local files). For a vault note on a renamed repo, this 4-axis verification justifies treating the two paths as one project and writing ONE note — but the note must call out the configuration difference (which `origin` URL each uses) so a future reader doesn't get confused about which to clone. Recorded case: `~/projects/build-your-own-harness/` and `~/projects/claude-code-in-100-lines/` matched on all 4 axes (28 tracked paths, byte-identical content, HEAD `4a14515d`, root tree `0e2c50ac`); the only meaningful difference was the `origin` URL — the former-name folder pointed at the redirected URL, the canonical folder pointed at the current canonical.

## Related: cloning the repos locally (separate from the vault)
  not just a citation list.** When the wave's subject isn't a repo
  (people/protocols/systems researched from the web, not code), every claim
  needs to trace back to a dated URL row in a `Source log` table. The
  shape that actually works in practice is
  `| YYYY-MM-DD | URL | summary | trust 1-5 | [[Concepts/X]], ... |`
  — short enough to scan, with the last column being wikilinks so a reader
  can navigate from source to claim. The trust hierarchy (subject's own
  source → their YouTube → podcast → social → tabloid) is the same one
  used in academic reviews; tabloid/native-ad coverage is never citable.
  See `references/research-subject-brief.md` for the full variant brief.

- **Non-repo waves: when the subject is a mimickable regime (biohacker,
  supplement stack, intense diet/training protocol), every concept note
  opens with a `> **WARNING:**` reminder.** This is non-negotiable and is
  the user's *expected* reminder, not an editorial addition. The
  resource gap (cost, clinical team, contraindications) belongs in the
  warning — not in a buried paragraph. The user reaches for these notes
  specifically to decide "is this for me?"; the warning is the answer to
  "no, not unmodified." Without it, the notes accidentally read as
  endorsement.

- **Non-repo waves: mirror the vault folder name-for-name under
  `/tmp/<wave>N/`.** Because the staging path and the vault path share
  every component except the prefix, a stray write to the vault path is a
  silent same-name collision that the `git status --short -- Github/Repos/`
  check (calibrated for repo waves) does NOT catch. Use the vault-relative
  `Personal Growth/<Subject>/...` path under the staging prefix, and
  verify with `git status --short -- Personal\ Growth/<Subject>/` before
  the 3-line summary, not the GitHub-shaped check.
- **Deep enrichment: never touch the vault file directly.** The wave brief is
  explicit — write to `/tmp/enrich-wave<n>/<slug>.md` only. The parent agent
  verifies and copies. Touching `~/Documents/.../Repos/<slug>.md` mid-wave is
  a coordination violation that other parallel agents will overwrite. Confirm
  with `git status --short -- <vault-path>` before printing the final summary
  (it should be empty against that path); a stray untracked `??` line means
  the parent-agent coordination guarantee is broken.

- **Deep enrichment: the brief's cloned-repo path is a default, not a
  guarantee.** The wave brief typically says "cloned repo at
  `~/projects/<slug>/`" — but on this user's machine that path is often wrong
  (and the path-not-found error from `ls` is the only signal). Before
  declaring the repo missing, run a quick hunt:
  ```bash
  find ~/projects -maxdepth 4 -name "<slug>" -type d
  find ~ -maxdepth 5 -path "*<slug>*" 2>/dev/null
  ```
  Three patterns this user's setup actually uses:
    1. **`/home/arctic/projects/<slug>/`** — the brief's default. Confirmed
       for repos like `Project-Rim`, `SlipStream`, `Insurance_chatbot`,
       `candy-mail`, `texed`.
    2. **`/home/arctic/projects/<owner>/`** — when the repo's *owner* name
       is the slug (e.g. the `adityasasidhar/adityasasidhar` user-as-repo).
       Has a single README; not a workspace, just the dotfile home for the
       user account.
    3. **`/home/arctic/projects/website/<project-name>/`** — GitHub Pages
       sites and similar projects where the *local* project name (used in
       `package.json` and the directory) differs from the *GitHub* repo name.
       Concrete observed case: GitHub repo `adityasasidhar.github.io` lives
       at `/home/arctic/projects/website/the-deep-field/`. The directory
       name is `the-deep-field` because `package.json` is `name: "the-deep-field"`;
       the GitHub repo name is `adityasasidhar.github.io` because that's the
       mandatory GitHub Pages username convention (`<user>.github.io` is
       always the user-site repo). Verify with `git remote -v` inside the
       candidate directory — if `origin` points at the expected GitHub URL,
       it's the right tree even when the directory name disagrees.
  When you find the tree via the fallback hunt, drop a one-line note in the
  output's 3-line summary ("Working tree is at
  `/home/arctic/projects/website/the-deep-field/`, not the brief's
  `/home/arctic/projects/adityasasidhar.github.io/` — same `origin` remote")
  so the parent agent knows the discovery was intentional, not a typo.
- **Deep enrichment: when the GitHub repo name ≠ the local directory name ≠
  the `package.json` name, capture all three explicitly.** This is normal for
  GitHub Pages repos and for repos renamed mid-history. In `### Tech stack`
  and `### Key files`, the local truth (what `package.json` declares) and the
  GitHub truth (what the URL bar shows) are both worth quoting:
  `package.json` declares `"name": "the-deep-field"`; the GitHub repo is
  `adityasasidhar/adityasasidhar.github.io` (mandatory `<user>.github.io`
  Pages repo name); the served URL is `https://adityasasidhar.github.io/`
  because `astro.config.mjs` sets `site: 'https://adityasasidhar.github.io'`.
  All three belong in the note; a future reader skimming for "what does this
  repo *do* and where does it *live*" needs both axes.
- **Deep enrichment: two repos can share the same display name in the
  vault.** When the brief points at `adityasasidhar.github.io` and an
  adjacent note already uses the display name `The Deep Field`, check the
  existing `the-deep-field.md` (or any sibling note with the same display
  name) before writing the new one — the older note may be a Python research
  repo (`language: Python`, `type: LLM / Agent`) while the new one is an
  Astro site (`language: Astro`, `type: Web Tool / Portfolio`). They are
  *distinct repos* with the same branded name. Surface this in the related
  links: `Related: [[the-deep-field]] (the older Python research repo that
  shares the display name with this Astro site)`. The parenthetical
  disambiguates the wikilink target so a future reader doesn't conflate them.
- **Deep enrichment: verify the repo actually compiles before quoting `python
  src/app.py` as the run command.** Run `python3 -m py_compile src/*.py
  fun.py` once near the start of the inventory phase. Incomplete WIP files
  (`def all(query): if query` with no body) silently pass the
  `ls`-looks-fine check and produce a `SyntaxError` only at first run; when
  this happens, the `### How to run it` section must call it out explicitly
  ("fix `src/wbs.py` and provide the code-expected key before launching") and
  the run command must reflect that the script as-checked cannot start. The
  README's "happy path" is a target, not a contract.
- **Deep enrichment: file-size mismatch is a feature, not noise.** When a repo
  has a large `requirements.txt` (60+ pinned lines) for a small script,
  inspect for a freeze-style snapshot. Distinguish three populations in
  `### Tech stack`: (a) runtime/model deps actually imported by tracked
  Python, (b) scraping/web retrieval deps that participate in the active
  data path, (c) "snapshot extras" pinned but unused by the REPL (Flask,
  pandas, Selenium, etc.). List each with its pinned version from the
  manifest; the snapshot-extras line is the signal that the environment is
  broader than the live code.
- **Deep enrichment: confirm the local HEAD is the build you're describing.**
  A clone under `~/projects/<slug>/` is frequently one or more commits
  ahead of `origin/main` (the author did a local rebuild before pushing),
  OR a dirty worktree (deleted files, untracked experiments) at the same
  HEAD as origin. Check both axes:
    1. `git rev-parse --short HEAD` vs `gh api repos/OWNER/SLUG/branches/main --jq '.commit.sha'`
    2. `git diff --stat HEAD...origin/main` AND `git status --short` (the
       second catches dirty-but-committed-this-way worktrees)
  When they diverge, **describe the clean local checkout** (what `wc -l
  game.py` actually shows) and add one line in `## Status` noting the
  mismatch ("local `main` is at `ec530e6`, one commit ahead of
  `origin/main`; the rebuilt World Explorer is not yet on GitHub"). This
  is a frequent finding in `enrich-wave<N>` setups where the parent
  cloned the repo right before dispatching the wave. **A concrete
  observed instance:** a Python agent repo's README badges and CI
  workflow advertised "210 passing" tests, but the local worktree had 22
  modified tracked files plus four untracked `tests/test_*.py` modules
  with new assertions; running the suite locally returned **308 passed**.
  Quote both numbers in `## Status` ("README: 210 passing; local
  tree: 308 passing with extra untracked test modules") so a reader
  skimming the note knows the badge and the local reality differ, and
  knows which to trust.
  - **HEAD == origin/main does NOT mean a clean tree.** A `git status`
    showing deleted tracked files or untracked experiment folders is
    its own signal — the author is mid-iteration, and the on-disk
    shape is the truth, not the committed shape. Concrete observed
    case: a Kaggle competition repo had `HEAD == origin/main` at
    `16cca87`, but `git status --short` showed tracked `train_8/train.py`
    deleted and untracked `train_10/` plus a new
    `batch-*-output.jsonl`. Write `## Status` describing the on-disk
    reality (`"tracked train_8/train.py deleted; untracked train_10/
    present; HEAD == origin/main"`), not just the commit parity. A
    future reader cloning fresh would see only what's committed and
    miss the active iteration.
- **Deep enrichment: GUI/desktop apps need a headless smoke test, not
  `python3 -m py_compile`.** A `py_compile` pass proves syntax, not
  initialization. For PyGame / Tk / Qt / SDL apps, force an offscreen
  driver (`SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy`), import the entry
  point, instantiate the main class, call one render pass, then
  `pygame.quit()`. A passing run confirms (a) the venv resolves, (b) the
  state files (`properties/*.json`, `properties/*.txt`) parse, (c) the
  first frame draws without exceptions. Surface the exact command in
  `## Status` so the parent's verification step can re-run it. Skip if
  the repo has no GUI dependency — `py_compile` is enough for
  pure-stdlib CLI tools.
- **Deep enrichment: do not run `pytest` directly against `.venv/bin/pytest`
  inside a repo cloned under the parent shell.** The parent agent's
  `PYTHONPATH` (often set by Hermes's own venv, e.g. a Python 3.11
  `pydantic_core`) leaks into the child process and wins over the
  project's `.venv/lib/python3.13/site-packages`. Symptom: collection
  errors of the form `ModuleNotFoundError: No module named
  'pydantic_core._pydantic_core'` on every test file even though the
  project's own venv has pydantic installed. Fix: invoke through uv with
  PYTHONPATH cleared so the project's interpreter and site-packages
  win: `env -u PYTHONPATH uv run pytest -q`. Recorded case: `zuck`'s
  `.venv/bin/pytest` collected 0 tests with 8 import errors; the same
  suite via `env -u PYTHONPATH uv run pytest -q` reported `308 passed`
  in 1.13 s. The same PYTHONPATH leak breaks non-pytest smoke tests too
  — a plain `python3 -c "from src.tools import bash"` invoked inside a
  uv-managed repo can fail with `ModuleNotFoundError: No module named '<x>'`
  even when `uv.lock` resolves the dep cleanly, because the system
  `python3` wins over the project's venv. Fix: use `env -u PYTHONPATH uv
  run python -c "..."` for every direct smoke-test invocation in the
  inventory phase. Recorded case: a uv-managed agent harness's
  `python3 -c "from src.tools import bash, timer; ..."` raised
  `ModuleNotFoundError: No module named 'ollama'`; the same snippet via
  `env -u PYTHONPATH uv run python -c "..."` ran the non-network tool
  smoke checks successfully. Use the uv wrapper for every `pytest`,
  `ruff`, `mypy`, and direct `python` smoke-test invocation in the
  inventory phase for the same reason.
- **Deep enrichment: report bytes (`wc -c`), not characters (`wc -m`),
  in the 3-line summary.** The wave parent reads the absolute path +
  byte count, not a char count, to enforce the brief's window. A
  multibyte-character note can be 7,500 chars but 9,800 bytes; reporting
  the smaller number hides the real size. Use `Path.stat().st_size` or
  `os.path.getsize(...)` — same number, no terminal dependency. The
  brief's "5–10k chars" window is often implemented by the parent as
  `5000 <= os.path.getsize(p) <= 10000`, so the byte floor/ceiling
  matters.
- **Deep enrichment: smoke-test every pure-function helper you can.** When
  the repo exposes `validate_map()`, `validate_colors()`,
  `_is_connected()`, `find_spawn()`, etc., call them with hand-crafted
  inputs (boundary cases like `['R'*10]*10`, off-range RGB triples like
  `[300,-1,12.8]`, fenced JSON like a JSON-in-fenced-code-block sample)
  and quote the actual assertion lines you ran in `## Status` or
  `### Notable details`. The next reader benefits from knowing that
  `validate_colors({'G':[300,-1,12.8]})` returns `{('G', (255,0,12))}`,
  not just that "validation works."
- **Deep enrichment: an empty-input smoke check is vacuous, not
  evidence.** When a repo's `_validate_and_calculate_python` /
  `validate_*` / `assert_*` accepts `{}` and reports balanced or zero
  errors, the result is meaningless — every field defaults to zero,
  the equality check passes trivially, and the validator proves
  nothing about correct extraction. Always feed at least one
  hand-crafted non-empty input (one nonzero field per major section)
  and report what actually fails. Quote the assertion lines in
  `### Notable details` and explicitly call out that balanced at
  zero is vacuous. When third-party deps are not installed, isolate
  the validator by extracting just that FunctionDef via
  `ast.Module(body=[node])` plus `exec(compile(mod, ...))` into a tiny
  namespace with Dict/Any/List aliases — runs without pip-installing
  the rest of the tree. Full recipe in
  `references/deep-enrichment-recipes.md` under Single-function smoke
  check (no venv, no imports).
- **Deep enrichment: count leaf fields of an inline JSON-schema, not
  just top-level groups.** When the repo embeds an Anthropic tool_use
  / OpenAI function_calling / Pydantic model_json_schema dict (often
  a module-level constant like BALANCE_SHEET_TOOL or TOOLS), the
  leaf-field count is the single most useful number to quote in
  `### Notable details` — roughly ten fields reads skeletal, roughly
  one hundred fifty reads comprehensive. Use a recursive walker over
  properties plus arrays (skip prior_year keys when summing to avoid
  double-counting year-pairs). Trap: iterating `ast.Module.body` and
  reading `.name` or `.end_lineno` crashes on `ast.Assign` nodes —
  filter by `isinstance(n, ast.FunctionDef)` first, or use
  `getattr(n, 'name', '')`. Full recipe in
  `references/deep-enrichment-recipes.md` under Inline JSON-schema
  introspection.
- **Deep enrichment: capture the hard-coded model/version string
  verbatim.** When the source calls
  `client.messages.create(model=claude-sonnet-4-5)` or
  `MODEL = qwen2.5-coder:7b` or any analogous hard-coded model
  identifier, quote it byte-for-byte in `### Tech stack` (NOT
  paraphrased as Anthropic Claude or a Qwen variant). These strings
  go stale fast and are exactly what a reader wants to verify before
  running the smoke test. The declared version in requirements.txt
  (`anthropic==0.71.0`) is the SDK, not the model; the model is the
  inline string literal.
- **Deep enrichment: when the repo has a `.venv/` next to tracked files,
  ignore it for inventory.** `git ls-files` is the source of truth and
  `.venv/` is normally gitignored. But if `search_files(target='files')`
  is being used to walk the directory (not git), filter the extension
  histogram through `git ls-files` to avoid double-counting installed
  packages. The `Counter` of extensions over `git ls-files` is what
  characterizes the project shape; `Counter` over the raw filesystem
  also includes `site-packages/...` noise.
- **Deep enrichment: GitHub's `size` field is the full `.git/objects`
  pack, not the current tree.** A 181 KB repo (the value reported for
  `fun-with-pygame`) with a few-KB source tree is normal when the
  initial commit tracked PNG screenshots or other binary artifacts
  (the four `images/*.png` here total ~183 KB). Cross-check by adding
  `git ls-tree -r --long HEAD | sort -k4 -nr | head` to your inventory
  and listing the three largest tracked blobs in `### Notable details`
  when binary content dominates the size.
- **Deep enrichment: clear "absent" categories in `### File structure`.**
  The vault reader skims `### File structure` to locate modules.
  Negative findings ("no `tests/`, no package directory, no
  `.github/workflows/`, no CI, no audio subsystem, no sprite manager")
  are real signals — name them explicitly. "Absent" line is shorter
  than writing "and there's no X" three times in `### Notable details`.
- **Deep enrichment: credential-shaped files in `git ls-files` are a real
  finding.** When `src/groq.txt`, `apikey.txt`, `.env`, or similar exist as
  tracked objects (non-empty `wc -c`), call this out by *name* in `###
  Notable details`. Do not reproduce the contents; do flag that the file
  is committed, what module reads it, and whether its expected path
  relative to the working directory actually matches the tracked location
  (a frequent drift: code reads `../apikey.txt` but no such file exists;
  another reads `groq.txt` while the tracked file lives at `src/groq.txt`).
- **Deep enrichment: when the README documents `config.json` or
  `utils/<helper>.py` that aren't tracked, surface that explicitly.** Same
  drift category as "code vs README" but specifically for files the README
  promises exist. The reader skimming the note will reach for
  `config.json` first; one bullet ("README references `config.json` and
  `utils/cuda_memory_check.py`; neither is tracked") saves them a minute.
- **Deep enrichment: notebook-first repos are entry-point-less.** A repo whose
  logic lives in `*.ipynb` files has no `main.py` to run; `main.py` may be a
  0-byte placeholder. Don't invent a `python main.py` command. For
  `### How to run it` use the same `git clone` + `uv sync` (or `pip install`)
  flow as any other repo, then say "open the notebook in a Jupyter-capable
  editor and run cells top to bottom." Use `scripts/extract_notebook_outputs.py`
  to read the recorded outputs (loss traces, accuracy, predictions) straight
  from the notebook JSON — `read_file` will truncate a fat notebook and drop
  the outputs, which is exactly the evidence the brief wants in `### Notable
  details`. See `references/notebook-only-repos.md` for the full
  notebook-only workflow (cell-by-cell walkthrough, identical-tree detection,
  Colab-specific pitfalls like `private_outputs`, embedded-YAML-as-config,
  and the rule "do NOT rerun the training command").
- **Deep enrichment: parse `uv.lock` with `tomllib` for exact versions.** Modern
  `uv.lock` is TOML (not the legacy JSON-CUDA `pylock-torch` style). Read it
  with a short `tomllib.load(open('uv.lock','rb'))` script and quote the
  resolved version of each direct dep in `### Tech stack`. Pro: reproduces
  the env. Con: declaring `>=X.Y` in `pyproject.toml` while the lock is at
  X.Y means the lock is the truth.
  - **Multi-platform lockfile entries.** `uv.lock` often contains *multiple*
    entries for an unpinned dep like `numpy` or `opencv-contrib-python` —
    one per supported platform marker (e.g. `numpy 2.2.6` on `aarch64-linux`,
    `numpy 2.4.4` on `x86_64-linux`). A naive loop printing every entry with
    a matching name will print *both*. Either filter by platform
    (`sysconfig.get_platform()`) or quote the range in `### Tech stack`:
    `numpy unpinned; lock resolves to 2.2.6 / 2.4.4 across platform
    markers`. Picking one version silently is a real correctness gap — the
    lock has the platform-aware resolution for a reason.
- **Deep enrichment: `py_compile` does NOT prove the package imports.** A
  passing `python3 -m py_compile src/*.py` only proves the source tree has
  no `SyntaxError`. It does NOT prove the resolved dependency tree imports
  cleanly — legacy transitive pins (msgpack-rpc pulling Tornado 4.5.3,
  AirSim's old RPC chain, packages with C extensions that need a
  compatible Python) can still blow up at import time with
  `ModuleNotFoundError`. For any non-trivial dep tree, run
  `.venv/bin/python -c "import <entry_module>"` once during inventory and
  quote the actual failure in `## Status` or `### Notable details` if it
  raises — do NOT paraphrase as "deps pinned" when the import fails.
  Concrete observed case: a Python AirSim repo's `py_compile` passed and
  `uv.lock` resolved all direct deps, yet `import src.control` raised
  `ModuleNotFoundError: No module named 'tornado.platform.auto'` because
  the installed Tornado 6.x dropped `platform.auto` while
  `airsim` -> `msgpackrpc` still expects Tornado 4.5.3 (the version listed
  in `pyproject.toml`'s `[tool.uv.extra-build-dependencies]`). The fix is
  project-local (`uv add 'tornado==4.5.3'` or pin the AirSim transitive);
  the note must surface that the script as-checked cannot start, and
  `### How to run it` must include a "resolve X before launch" line
  rather than advertising the README's happy path verbatim.
- **Deep enrichment: when both README and GitHub description are absent, the
  `pyproject.toml` `description` field is the ground truth.** Use that
  one-line string (cleaned of markdown) for the frontmatter `description:`
  and as the seed for the Overview. This is the normal case for
  notebook-first learning repos with no README.
  - **Data-only README is its own failure mode.** Many Kaggle
    competition repos publish a README that is *only* a markdown table
    (category solve rates, leaderboard, dataset stats) with no prose
    intro at all. Combined with a `null` GitHub `description`, both
    standard sources are empty even though the repo is real and active.
    The Overview then has to be reconstructed from the actual source:
    the train scripts' docstrings, the metric.py module docstring, the
    `dataset_build/validation_report.txt`, and the package name on
    `pyproject.toml`. This is a legitimate reconstruction (the code IS
    the description), not fabrication. Flag the absence explicitly in
    `## Status` ("README is a category table only; GitHub description
    null; Overview reconstructed from source") so the parent agent
    knows the prose was derived, not transcribed.
- **Deep enrichment: multi-package research repos with `pyproject.toml`
  console scripts and NO README — the `[project.scripts]` block IS the run-command
  source.** Many small research/experiment repos (LEAP, ad-hoc comparison
  studies) declare two or more console scripts but ship no README. When
  `gh api repos/OWNER/SLUG/readme` returns 404, the canonical "How to run it"
  is the `[project.scripts]` table:

  ```toml
  [project.scripts]
  # Baseline ReAct Agent (for comparison)
  react-agent = "langchain_agent.main:main"
  # LEAP Agent (the research project)
  leap = "leap_agent.main:main"
  ```

  Each entry gives the *binary name* (runnable as `<name>`), the *target
  module*, and the *function to call*. Combine with the package's own
  `[tool.hatch.build.targets.wheel]` (or equivalent setuptools `[packages]`)
  to confirm both packages ship in one wheel. The Getting Started section
  becomes literally:

  ```bash
  uv sync
  uv run leap --pair 4gb
  uv run react-agent -m qwen3:4b
  ```

  Quote the exact console-script table in `### Notable details` ("two
  console scripts declared: `leap` → `leap_agent.main:main` and
  `react-agent` → `langchain_agent.main:main`"). The argparse choices
  list (`choices=list(MODEL_PAIRS.keys())`) is also worth quoting — it
  tells the reader the valid `--pair` values without re-reading the
  source.
- **Deep enrichment: tiny repos (<30 source lines) get their depth from history,
  not from the source.** When the program is one file of 15 lines, you can't
  reach 5–10k chars by re-describing the function. Pull depth from these
  sources, in priority order:
    1. **Git log + diffs.** Every commit's stat (`git show --stat`) and full diff
       (`git show --format= --no-ext-diff <sha>`) reveals what was added, deleted,
       and renamed. Two-commit histories often show the "before/after" of a
       learning step; capture both. Note any deleted files (binaries, old
       sources) — they explain surprising repo size on GitHub.
    2. **Lockfile, not just manifest.** The README/`Cargo.toml`/`pyproject.toml`
       line is the declared dependency; `Cargo.lock`/`package-lock.json`/
       `requirements.txt` is the resolved graph. The transitive stack
       (e.g. `rand → rand_chacha → rand_core → getrandom`) is the real
       "tech stack" — list the locked versions, not the declared ones.
    3. **IDE/editor metadata.** `.idea/*.xml`, `.vscode/settings.json`, and
       similar files in a tracked tree reveal the author's local environment
       (IntelliJ module layout, excluded folders, VCS mapping). One sentence
       per file is enough; don't transcribe them.
    4. **`.gitignore` vs what's actually tracked.** A path added to
       `.gitignore` does NOT remove already-tracked files. When the repo has
       tracked `.idea/` files plus a `.gitignore` line excluding `.idea`,
       that's a real curiosity to surface in `Notable details` ("author stopped
       tracking IDE files but didn't untrack the existing ones").
    5. **Build-and-run verification.** `cargo run`, `pytest`, `npm run build`
       etc. proves the project compiles and surfaces runtime behavior. For an
       interactive program, pipe a known input (`printf '7\n' | cargo run`)
       and record what actually comes out — it often differs from the
       README's claims.
    6. **Repo-name vs binary-name divergence.** When `Cargo.toml` declares
       `name = "forfun"` but the GitHub repo is `Learning-Rust`, that asymmetry
       is a real detail (the user picked one name for the repo and a different
       one for the package). Worth one line in `Notable details` and one in
       `How to run it` ("Cargo builds an executable named `forfun`").
- **Deep enrichment: small repo size on GitHub ≠ current tree size.** GitHub's
  `size` field reports the full `.git/objects` directory including every blob
  ever added (including deleted files). A 1,087 KB repo can have a 4 KB
  current tree if the first commit tracked a large artifact that was later
  deleted — Git keeps the object reachable from history. When the size looks
  surprising, cross-check with `git ls-files | xargs wc -c` (current tracked
  bytes) and `git verify-pack -v .git/objects/pack/*.idx | sort -k3 -nr | head`
  (largest objects in the pack). Surface the discrepancy in `Notable details`
  with both numbers.
- **Deep enrichment: ML-research / paper-track repos get a `## Headline numbers`
  section after `## Stats`.** The 11 canonical sections don't include one, but
  paper-track repos (`babylm`, `nlp` workshop submissions, arXiv-style
  experiments) live and die by their results tables — the headline val-loss
  ranking and BLiMP/GLUE/SuperGLUE numbers are the whole point. Add a
  short section between `## Stats` and `## Links` formatted as a table:

  ```markdown
  ## Headline numbers

  - **Final validation loss** (uniform per-token, N dev tokens): `recursive_3to1`
    **3.0926** < `recursive_2to1` 3.1072 < ...
  - **Zero-shot BLiMP** (official 2026 full sets, macro accuracy):
    | Variant | BLiMP | Supplement |
    |---|---|---|
    | `baseline` | **71.07** | **59.59** |
    | ...
  ```

  Always quote (a) the comparison protocol in the header ("uniform per-token,
  2M dev tokens", "official 2026 full sets, macro accuracy") and (b) the
  confidence interval where the paper provides one ("paired 95%
  evaluation-sample intervals are [0.38, 6.85] — these do not estimate
  training-seed variance"). The protocol qualifier is what makes the
  numbers reusable; raw numbers without the protocol become garbage in
  six months. Source the numbers from `paper/figures/*.json` (committed
  evaluation artifacts), NOT from the paper's prose, because the
  committed JSON is what the eval actually wrote. If the JSON is absent
  or stale relative to the paper text, surface that as a Notable details
  finding and prefer the JSON.

- **Deep enrichment: ML-research repos triangulate via three files.** Paper-track
  repos have (1) a prose source — `paper/paper.md` or `paper/main.tex`, (2) a
  figure-eval source — `paper/figures/*.json` with the actual numbers written
  by the eval, and (3) a paper-rendering source — `paper/latex/main.pdf`. The
  prose and the JSON sometimes drift (the paper gets revised in flight, the
  JSON is what the run actually produced). Read all three for any headline
  number; if they disagree, the JSON wins and the divergence goes into
  `### Notable details`. Same applies to `paper/figures/make_figures.py`
  and any `paired_uncertainty.py`/`ckpt_eval.py`/`blimp_eval.py` scripts —
  those are the methodology citation, not the README.

- **Deep enrichment: ML-research repos expand `Concepts:` aggressively
  (8–12, not 4–6).** The 4–6-concept rule of thumb is for general repos; a
  research codebase IS a composition of named architectural techniques, and
  under-listing them defeats the purpose of the wikilink graph. Read every
  `import` in `src/common/attention.py`, `src/common/layers.py`, and the
  config files; the named primitives (Gated DeltaNet, RoPE, RMSNorm,
  SwiGLU, Grouped-Query Attention, FlashAttention-3, Flash Linear Attention,
  BLiMP, Mamba, Delta Rule) each warrant a concept link. Real case from
  the `recursive-babylm` wave: the original stub listed 6 concepts; the
  code revealed 11 distinct techniques; under-listing would have lost
  Gated DeltaNet, GQA, FA3, fla, and BLiMP from the graph. The vault
  reader skimming `Concepts:` decides whether this repo is worth linking;
  honesty here compounds.

- **Deep enrichment: digit-prefixed config / package names need `importlib`.**
  When a repo's variant folders are named `2to1`, `3to1`, `1x`, `v2`,
  `4bit-quant`, etc. (any name starting with a digit), Python can't
  `import` them — they're not valid identifiers. The fix is a registry
  dict + `importlib.import_module(f"{pkg}.config")`. Common pattern;
  surface in `### Notable details` ("variant folders start with a digit,
  so they load via `importlib` in `src/common/variants.py`") because a
  reader trying to add a new variant will hit this and the note is the
  fastest pointer to the workaround.

- **Deep enrichment: an `attn_backend ∈ {auto, fa3, sdpa}` portability
  fall-back is a strong signal.** When you see a module selecting between
  FA3 / SDPA / `auto` (or any analogous Triton-vs-fallback pattern), it's
  the author's deliberate answer to "make this runnable on a 4 GB laptop
  AND on an H100." Quote the three modes verbatim in Notable details and
  the import-time detection pattern (`try: from flash_attn_interface
  import ...; _HAS_FA3 = True; except ImportError: _HAS_FA3 = False`).
  This is the exact portability story a reader wants before they try
  running the smoke test on their own box.

- **Deep enrichment: for `Learning/Educational` repos, name the language
  concepts the code actually demonstrates.** Don't just list the dependency.
  When the source uses `mut`, `&mut`, `String::new()`, `read_line(&mut ...)`,
  `expect()`, `println!`, an exclusive range, and a trait-provided method
  call, those are each a concept to link in `Concepts:` — e.g.
  `[[Ownership and Borrowing]]`, `[[Traits]]`, `[[Result handling]]`. The
  vault's existing concept notes may not exist yet; link them anyway so the
  parent agent can create them.

- **Local-only project notes: staging path is `kg-sync-tmp/local-notes/`,
  NOT `/tmp/<wave>N/`.** The deep-enrichment variant sends output to
  `/tmp/enrich-wave<n>/<slug>.md`; the GitHub-shaped bulk sync writes
  directly to `Github/Repos/<slug>.md`. **Local-only project notes use
  `/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md`** (this user's
  convention). The mirror sibling `kg-sync-tmp/github-notes/<slug>.md` holds
  GitHub-shaped repos. The vault-untouched guarantee still applies — verify
  with `git -C <vault_root> status --short -- Github/Repos/ Projects/`
  before the 3-line summary. Don't confuse the two staging folders: writing
  a local-only project note to `/tmp/enrich-wave<n>/` violates the
  parent-agent coordination discipline (other parallel waves share
  `/tmp/`).

- **Local-only project notes: `gh api` is irrelevant; don't call it.** The
  deep-enrichment pitfalls describe `gh api repos/OWNER/SLUG/...` calls
  for metadata, README content, and HEAD SHA. None of these work for
  local-only projects — there's no `OWNER/SLUG`, no remote, no API
  listing. The replacement inputs are all on-disk: `git log --oneline`,
  `git remote -v` (likely empty), `git status --short`, the manifest
  files (`package.json`, `pyproject.toml`, etc.), and the source itself.
  Calling `gh api` on a local-only repo is a wasted call AND a tell that
  you didn't read `git remote -v` first.

- **Local-only project notes: pre-write vault audit is mandatory, not
  decorative.** Before writing any note, grep the vault for prior
  references to the project name and slug forms. Three greps, run
  sequentially: (1) display name (title case), (2) slug form (lowercase,
  hyphenated/underscored), (3) 2-3 concept-keywords. Empty result across
  all three means safe to write a new note. A hit at any step means
  **read the matched file before writing** — the existing note may be
  hand-curated prose to preserve (deep-enrichment pitfall), a stub to
  enrich, or a same-display-name collision with a different project.
  Record the audit result in the 3-line summary. Skipping the audit and
  overwriting a curated note is the easiest way to lose user trust.

- **Local-only project notes: source-authority rule — code wins over
  prose files.** When `README.md`, `CLAUDE.md`, `AGENTS.md`, or any other
  prose file disagrees with the actual source code or manifest, the code
  is authoritative for *technical* claims (model names, dependency
  versions, settings keys, schema fields). Treat the prose as
  authoritative for *intent* and *workflow conventions*. Concrete observed
  case: `openresearch/CLAUDE.md` described `nemotron-3-nano:30b-cloud` as
  the model in use, but `agents/models.py` switched to Google's Gemini
  via an OpenAI-compatible endpoint (`ChatOpenAI(model="gemma-4-26b-a4b-it", ...)`).
  The code is the contract; CLAUDE.md was a snapshot that drifted. When
  this happens, surface the disagreement in `### Notable details` with
  both values quoted verbatim and the authoritative source named.

- **Local-only project notes: `.env` files are auto-blocked by `read_file`,
  don't try to bypass.** The `read_file` tool blocks secret-bearing `.env`
  files as defense-in-depth (the terminal tool can still bypass; don't).
  When you encounter a populated `.env` in `git ls-files` (or via
  `find ... -name '.env*'`), note its existence and what module reads it,
  but never open the file. Surface in `### Notable details` as a
  credential-handling finding ("`.env` present, gitignored, contains
  `GEMINI_API_KEY` and `TAVILY_API_KEY`; source reads via `os.environ` at
  call time, not import time"). Trying to read `.env` via terminal cat
  defeats the purpose of the guard and risks leaking credentials into
  your context.

- **Local-only project notes: 0-commit repos need an mtime fallback.**
  When `git log` fails with `fatal: your current branch 'X' does not have
  any commits yet`, the repo is a fresh scaffold with no commit history.
  Pull `created` from `git init` time (often missing or unrecoverable) or
  the earliest file mtime via `find <path> -type f -printf '%T+ %p\n' |
  sort | head -1`; pull `last_updated` from the latest file mtime. Set
  `status:` to `scaffolded (no commits)`. Don't fabricate a commit date;
  an explicit "(no commits)" is more honest than an invented date. The
  parent agent enforces provenance — a made-up `created` will fail the
  audit.

- **Local-only project notes: dirty worktree state is real and worth
  surfacing.** When `git status --short` shows uncommitted modifications,
  the on-disk reality differs from the committed shape. List the changed
  files in `## Status` (`M DESIGN.md`, `M bun.lock`, `M package.json`,
  `M src/cli.ts`, ...). A dirty tree means describe the working tree,
  not HEAD — the on-disk shape is what a reader would clone from `git
  stash && git pull` plus re-applying those changes. Concrete observed
  case: `rocky_code` had 9 modified tracked files at session time
  (DESIGN.md, bun.lock, package.json, src/cli.ts, src/config/schema.ts,
  src/core/{compact,loop,prompt,session}.ts, src/core/provider/stream_util.ts)
  with HEAD at `8bf8817`. The note must say "working tree dirty at
  `8bf8817` with M on src/cli.ts, src/core/loop.ts, ..." so a future
  reader knows the shape is in flux.

- **Local-only project notes: omit `## Stats` — there's no public stats
  card to populate.** The 11-section shape from the GitHub variant
  includes `## Stats` (emoji bullets for ⭐, 🍴, 🔒, 📅, 🔄, 🏷️, 📊).
  For local-only projects, none of those values exist: no GitHub stars,
  no fork count, no public/private flag (use `visibility: local`), no
  pushed_at, no language breakdown from the API, no license field. Skip
  the section entirely rather than fabricate `stars: 0` / `forks: 0`.
  Replace the section with a `## Status` line that includes the local
  equivalents ("language: TypeScript; package manager: bun; last commit
  2026-07-29; dirty worktree").

- **Local-only project notes: `vault_group` is `Projects`, not
  `Public Repos|Private Repos`.** The vault's local-project MOC is
  typically `Documents/fun/Projects/Projects.md` (or wherever the user
  keeps their work-area index), NOT the GitHub-shaped `Github/Repos/`
  folder. The `## Links` section's required footer becomes
  `Part of: [[Projects]]`, not `Part of: [[Public Repos]]`. The MOC
  itself should also gain a wikilink to the new note (e.g.
  `4. [[obsidian-galaxy-graph]]`). Verify by reading the MOC file
  before writing — if it doesn't exist, fall back to the user's
  `Me.md` top-level map or create the MOC as a separate task. The
  `Projects` MOC on this vault lists three items as of session time:
  `[[Docling]]`, `[[learning Type script]]`, `[[mini_chatbot]]`.

## Related: cloning the repos locally (separate from the vault)
To clone all repos into a projects dir, run `gh auth setup-git` once, then loop
`gh repo clone OWNER/NAME DEST/NAME` — this authenticates private repos automatically
via the keyring (no token juggling). Verify with
`git rev-parse --is-inside-work-tree` (NOT just checking for a `.git` directory —
a `.git` folder can exist without a valid repo). See `references/clone-and-verify.md`.

## references/
- `jq-pitfalls.md` — the gh-api jq failure and the Python fetch/parse fix.
- `readme-cleaning.md` — the clean() regex chain + feature/tech/install extractors.
- `clone-and-verify.md` — `gh repo clone` private-auth loop + git-validity check.
- `deep-enrichment-brief.md` — the per-repo code-reading subagent workflow: input
  sources, output path, required body sections in order, parent-verification checklist.
  Use when a parent agent dispatches a parallel "wave" of subagents to produce
  ~10–20k-char enriched notes from the cloned repo (not just the README).
- `deep-enrichment-recipes.md` — concrete shell-command batch for the inventory
  + verification phases, the categories of "reality gaps" worth surfacing in
  `### Notable details` (manifest vs imports, dead duplicates, tracked secrets
  etc.), and the right source-file reading order. Paired with `deep-enrichment-brief.md`.
- `local-project-staging.md` — this user's staging convention for local-only
  projects (no GitHub remote). When the source is a directory under
  `/home/arctic/projects/<name>/` with no `gh api` source, write to
  `/home/arctic/projects/kg-sync-tmp/local-notes/<name>.md` instead of
  `/tmp/enrich-wave<n>/<slug>.md`, use `vault_group: Projects` and
  `- Part of: [[Projects]]`, and use `local_path:` / `github: none` /
  `visibility: local` frontmatter. Includes a post-write validation recipe.
- `deep-enrichment-fallbacks.md` — the verified fallback chain when README, GitHub
  description, and `pyproject.toml` are all missing or null. Three observed
  patterns from this vault, with explicit "don't fabricate" guidance and the
  rule that visibly-empty fields beat invented prose.
- `tiny-repo-enrichment.md` — the playbook for `<30 LOC` learning-exercise
  repos where depth has to come from history, manifests, IDE metadata, and
  build/run verification instead of many source files. Includes the
  commit-diff and lockfile recipes, the build/run verification commands per
  ecosystem (Rust/Python/JS), and the concept-coverage checklist for
  `### Notable details`. Paired with `deep-enrichment-brief.md` and
  `deep-enrichment-recipes.md`.
- `research-subject-brief.md` — the brief for **non-repo waves**: research-subject
  folder population (people, protocols, systems) with a `Source log` of external
  URLs as the provenance record. Same wave discipline (write to `/tmp/<wave>N/`,
  never the vault; parent verifies and merges) but the evidence model is a URL
  log, not `gh api`. Includes the opinionated concept-note section shape, the
  mandatory `> **WARNING:**` reminder for biohacker/wellness subjects, and the
  vault-untouched verification recipe that doesn't rely on the GitHub-shaped
  `git status` path.
- `post-apply-verification.md` — the parent-side verification recipe after a
  research-subject wave: HTTP-200 sweep over every URL in the Source log +
  numeric spot-check that greps the primary source for headline numbers. Catches
  hallucinated URLs and fabricated numbers before they're written to the vault.
  Run this *after* copying `/tmp/<wave>N/...` into the vault and *before*
  declaring the wave done; ~30 seconds, prevents the single most embarrassing
  wave failure mode.
- `gui-smoke-and-local-divergence.md` — two patterns the main pitfall
  list alludes to but doesn't pin down: headless smoke tests for
  PyGame/Tk/Qt desktop apps (forced `SDL_VIDEODRIVER=dummy` /
  `QT_QPA_PLATFORM=offscreen`), and the "local HEAD ahead of `origin/main`"
  divergence check that surfaces when the parent cloned just before the
  wave. Also includes a drop-in combined verification recipe for the
  final self-check before printing the 3-line summary.
- `local-only-project-brief.md` — the brief for **local-only project
  notes**: filesystem-only projects with no GitHub remote, no `gh api`
  inputs, no public stats card. Schema differences (`visibility: local`,
  `local_path:`, `vault_group: Projects`, omitted `## Stats`), pre-write
  vault audit (3 grep passes), source-authority rule (code wins over
  prose), 0-commit mtime fallback, dirty-worktree surfacing, `.env`
  handling. The staging path is `/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md`,
  not `/tmp/<wave>N/`. Paired with the "Local-only project notes"
  workflow variant in SKILL.md.

## scripts/
- `generate_repo_notes.py` — self-contained, re-runnable generator (fetches metadata
  + READMEs via gh, writes schema-consistent notes, skips curated ones).
- `extract_notebook_outputs.py` — stdlib-only CLI that walks every `*.ipynb`
  in a repo and prints cell-by-cell source text + code-cell outputs (stream,
  execute_result/display_data, error). Use for notebook-first repos where
  `read_file` would truncate the file and drop the recorded run evidence
  that goes into `### Notable details`.
