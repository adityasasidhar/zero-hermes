# Local-project notes — this user's staging convention

When the task is to generate notes for *local-only* projects under
`/home/arctic/projects/<name>/` (i.e., repos with no GitHub remote, no
`gh repo clone` source, and `visibility: local`), the canonical
"deep enrichment" wave pattern (`/tmp/enrich-wave<n>/<slug>.md` +
`Part of: [[Public Repos]]` / `[[Private Repos]]`) does **not** apply.
This user has a different staging convention. Defaulting to the
deep-enrichment path produces files in the wrong location with the wrong
vault group, and a future merge step that looks for `Github/Repos/...`
files will not find them.

## Detected staging directory

```
/home/arctic/projects/kg-sync-tmp/local-notes/
```

This directory already contains prior notes
(e.g., `messaging_app.md`, `gradients_for_fun.md`, `openresearch.md`,
`rocky_code.md`, `fast_qwen.md`) that follow the convention. **Read one of
those first** when picking up a new local-project note — the format is
intentionally consistent across the file set, and the existing notes
are the canonical template.

## Convention differences vs. the deep-enrichment brief

| Aspect             | Deep-enrichment default          | Local-projects convention                      |
|--------------------|----------------------------------|------------------------------------------------|
| Output path        | `/tmp/enrich-wave<n>/<slug>.md`  | `/home/arctic/projects/kg-sync-tmp/local-notes/<slug>.md` |
| Vault group footer | `- Part of: [[Public Repos]]` / `[[Private Repos]]` | `- Part of: [[Projects]]`        |
| Frontmatter        | Adds `tags: [...]` concept list  | No `tags:` array; uses `topics:` for what GitHub would call topics |
| Extra frontmatter  | —                                | Adds `local_path: /home/arctic/projects/<name>` and `github: none` + `visibility: local` |
| ## Stats section   | Emoji bullets (⭐ 🍴 🔒 📅 🔄 🏷️ 📊) | Emoji bullets but adapted for local: ⭐ Local-only / 📅 date / 🔄 git state / 📊 size stats / 🏷️ category |
| ## Links section   | GitHub + Concepts + Themes + Related + Part of | Local path + sibling projects + Part of + Concepts + Themes + Related (different order than the brief's default; mirror what existing notes do) |
| Verification       | `git status --short -- Github/Repos/<slug>.md` (empty) | No vault file to verify — the staging file IS the artifact; verify `kg-sync-tmp/local-notes/<slug>.md` is new content, not an overwrite of an existing curated note |
| Input source       | `gh api repos/OWNER/SLUG` + cloned `~/projects/<slug>/` | Local-only; no GitHub API call possible. Inspect `ls`, `read_file`, `search_files`, and `terminal` directly under `/home/arctic/projects/<slug>/` |

## Workflow

1. **Check the staging dir first.** `ls
   /home/arctic/projects/kg-sync-tmp/local-notes/` — if a `<name>.md`
   already exists, read it before drafting. Many local projects have
   been picked up before; preserve any existing good prose.
2. **Inspect the local project root** with `search_files(target="files")`
   plus `git ls-files` if `.git/` exists, plus direct `read_file` on
   source files. No `gh api`, no remote metadata.
3. **Draft the note to** `/home/arctic/projects/kg-sync-tmp/local-notes/<name>.md`
   following the existing-note schema. Do NOT write to
   `/tmp/enrich-wave<n>/` — the user will look for the file under
   `kg-sync-tmp/local-notes/`.
4. **Footer must end with `- Part of: [[Projects]]`** — never
   `[[Public Repos]]` or `[[Private Repos]]`.
5. **When the source has no GitHub remote** (the common case for
   `/home/arctic/projects/<name>/`), set `github: none` and
   `visibility: local` in frontmatter, and quote the local path under
   both `local_path:` and a `Local path:` line in `## Links`.
6. **Skip obvious scratch / dependency-smoke-test scaffolds.** Projects
   that are an uncommitted `uv init` hello-world with a single declared
   dependency they don't even use (e.g., the canonical `testing` pattern
   in `/home/arctic/projects/testing/`) are not worth a note. State the
   skip explicitly in the session summary so the user sees the decision.

## Validation recipe (post-write)

```python
from pathlib import Path
import re, json, hashlib
p = Path(f'/home/arctic/projects/kg-sync-tmp/local-notes/{name}.md')
s  = p.read_text()
fm = s.split('---', 2)[1]
fields = {ln.split(':',1)[0].strip() for ln in fm.splitlines() if ':' in ln}
markers = [
    '## Overview','## Status','## Inside the Codebase',
    '### Architecture / How it works','### File structure',
    '### Key files','### Notable details','### Tech stack',
    '### How to run it','## What this project is about',
    "## Use cases / When you'd reach for this",
    '## Key Features','## Getting Started','## Links',
]
pos = [s.find(m) for m in markers]
assert {'repo','description','local_path','github','visibility','language',
        'topics','git_initialized','status','created','last_updated',
        'type','vault_group'} <= fields, fields
assert all(x >= 0 for x in pos) and pos == sorted(pos)
assert s.rstrip().endswith('- Part of: [[Projects]]')
assert not re.search(r'^(?:\d+\|)+', s, re.M)   # no read_file artifact
assert f'local_path: /home/arctic/projects/{name}' in s
```

If any check fails, fix before declaring done — the user has an
existing corpus that follows this convention and a stray file breaks
the pattern.

## Known footguns

- **The skill's brief says "never touch the vault note."** For
  `kg-sync-tmp/local-notes/`, the staging file IS the note — there is no
  vault sibling to be read-only about. The protection applies to the
  Obsidian vault proper (`~/Documents/Obsidian Vault/...` or wherever the
  user's actual Obsidian library lives), not to this staging dir.
- **The `Part of: [[Projects]]` footer is exact-match.** A trailing
  newline or stray character breaks grep verification. Use
  `s.rstrip().endswith(...)` not `... in s` for the footer check.
- **The `local_path:` frontmatter line must use the exact project
  directory name** (`hardcore`, `hf`, `rqlite`, not `hardcore_project`,
  not the casing from the README). The existing notes also quote this
  same path in a `Local path:` line under `## Links` — keep both in
  sync.
- **Existing local notes may already be at 6–13 KB** with substantial
  hand-written prose. Don't blindly rewrite them; treat the existing
  file as the ground truth of the last reading and patch what the new
  source actually adds. The "deep enrichment preserve existing good
  prose" rule applies here too.