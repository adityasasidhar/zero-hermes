# Graph-sync burn playbook

End-to-end recipe for "sync `/home/arctic/projects/` and the user's GitHub
repos into the vault's `Projects/` and `Github/Repos/` folders". Session-evidenced
2026-07-31 (52 GitHub repos, 10 local projects, 30 concept stubs, 12 new notes).

## Pre-flight

1. Confirm `gh auth status` works for the target owner.
2. Confirm `OBSIDIAN_VAULT_PATH` is set or `~/Documents/fun` resolves.
3. Read `wiki/CLAUDE.md` (or `AGENTS.md`) for the target vault's rules.
   Vaults frequently declare out-of-band policies (e.g. "empty stub files
   are deliberate placeholders, don't fill them in unless asked").
4. Run `python3 wiki/build_index.py --check` once to capture the baseline
   `broken` count — that is your **zero-new-broken budget** for the burn.

## Inventory

5. Pull all owner repos:

       gh api 'user/repos?per_page=100&affiliation=owner&sort=pushed&direction=desc'

   Stash to `/tmp/kg-github-repos.json`. Filter `fork:false` for note generation.
6. Walk `/home/arctic/projects/` (depth 0 and containers like `website/`,
   `paper-reproduction/`). Skip `.git`, `.venv`, `node_modules`, `target`,
   `__pycache__`, `aiml-burn-tmp`, `kg-sync-tmp`.
7. For each top-level dir, classify into one of five buckets:

   | Signal | Bucket |
   |---|---|
   | git remote URL matches a known GitHub repo *and* HEAD matches the canonical clone | `github-alt-working-tree` → one-line addition to the canonical note |
   | git remote URL points at adityasasidhar but is a *different* GitHub repo name | `github-already-noted` (canonical note exists) |
   | git remote URL is an external upstream (docling, doclang, opencode, gemini-cli, claude-code) | `external-upstream-clone` → skip |
   | no git remote, has source files | `LOCAL-PROJECT` → new `Projects/<name>.md` note |
   | no git remote, no manifest, no README, empty or container | skip |

8. For each `github-alt-working-tree`, add to the canonical note's
   `## Links` section: `- Local working trees: \`~/projects/<alt>/\`
   (alternate; same HEAD \`<sha>\`, <extra-files>)`. Do not create a
   duplicate note.

## Subagent dispatch

9. Dispatch one subagent per `LOCAL-PROJECT` with self-contained brief:
   exact source path, schema fields, required body sections, "never edit
   the vault", "return absolute path + byte count". Stage outputs to
   `/tmp/kg-sync-tmp/{github-notes,local-notes}/<slug>.md`.
10. Do NOT run multiple subagents against the same target file. If two
    projects need similar handling, dispatch separately with disjoint
    output paths.
11. Poll the staging dir for finished files. Apply each one the moment
    it lands (`shutil.copyfile(staged, vault_target)`). End-of-wave
    apply can lose files to `/tmp` wipes.
12. After each apply, run `python3 wiki/build_index.py --check` and
    confirm `broken` did not increase.

## Frontmatter discipline

13. The schema for `Github/Repos/<name>.md` is fixed: `repo, description,
    github, visibility, language, topics, stars, forks, status, created,
    last_pushed, type, vault_group` (14 fields). One-line blanks on
    `topics` are OK; blanks on `stars/forks/last_pushed` are not.
14. The schema for `Projects/<name>.md` is fixed: `repo, description,
    local_path, github (="none"), visibility (="local"), language, topics,
    git_initialized, status, created, last_updated, type, vault_group`
    (13 fields).
15. `status` values: `active | experimental / inactive | archived` for
    repos; `active | experimental | scaffold / in-progress | scaffold`
    for local. `experimental / inactive` is for repos idle >180 days;
    a 1-day-old repo is `active`, not `experimental / inactive`.
16. `type` must match the canonical categories: `LLM / Agent,
    ML Research / Model, Learning / Educational, Web App, CLI Tool,
    Game, Application / Utility, Config / Profile`.

## Concept-stub policy

17. New project notes introduce wikilinks to concepts that may not
    exist yet (`[[Bun]]`, `[[Modal]]`, `[[Tavily]]`, `[[Sub-Agents]]`,
    etc.). For each unique missing target, create a one-line placeholder
    in `Concepts/<Name>.md` with `type: concept, status: stub,
    created: <today>` frontmatter. This honors the `CLAUDE.md` rule
    that empty stubs are deliberate placeholders. After creating them,
    re-run `build_index.py --check` and confirm `broken` did not increase.

## Drift audit

18. After the apply pass, run `wiki/build_index.py --check` and read its
    output. **Do not write a hand-rolled audit script** — `build_index.py`
    already handles case-insensitive basename resolution, path-with-pipe
    alias forms (`[[path/note|alias]]`), and backticked `[[example]]`
    prose correctly. A hand-rolled script will report false positives
    on all three.
19. Re-pull the GitHub API and compare frontmatter on every repo note:

       python3 -c '
       import json, yaml
       from pathlib import Path
       api={x["name"]:x for x in json.load(open("/tmp/kg-github-repos.json")) if not x.get("fork")}
       for name in sorted(api):
         p=Path(f"/home/arctic/Documents/fun/Github/Repos/{name}.md")
         if not p.exists(): continue
         d=yaml.safe_load(p.read_text().split("\n---\n",1)[0][4:]) or {}
         x=api[name]
         if d.get("stars") != x.get("stargazers_count",0) or \
            str(d.get("last_pushed",""))[:10] != (x.get("pushed_at") or "")[:10]:
           print(f"DRIFT {name}  stars {d.get(\"stars\")}->{x.get(\"stargazers_count\",0)}  pushed {d.get(\"last_pushed\")}->{(x.get(\"pushed_at\") or \"\")[:10]}")
       '

    Drift on `stars`, `forks`, `last_pushed`, `language` is real and
    should be fixed. Drift on `description` is usually a *user-curated
    body description* that improved on the GitHub one-liner — leave the
    body alone and update only the objectively verifiable fields.

## Verification

20. `python3 wiki/build_index.py --check` reports:
    - `broken == baseline` (zero new broken links)
    - `ororphans == 0`
    - `unknown_tags` reflects pre-existing tag taxonomy gaps (acceptable)
    - `ambiguous == N` matches `wiki/AMBIGUOUS-LINKS.md` (intentional
      cross-system bridges)
21. `git -C <vault_root> status --short` should show the new files as
    untracked, the modified MOCs as modified, and **nothing else**.
    If files outside the expected `Projects/`, `Github/Repos/`, and
    MOC paths appear, you've drifted.
22. `/tmp/kg-sync-tmp/` cleanup is optional but recommended — `/tmp` is
    tmpfs and was wiped by snap/systemd recovery on 2026-07-28.

## Anti-patterns to avoid

- **"I wrote a note, the schema looks right, ship it."** — verification
  of negative claims (declared surface > actual surface, code
  doesn't compile) is cheap *now* and expensive later. Verify before
  propagating.
- **"Multiple subagents can write the same staging file."** — they
  can't; partition by target or run sequentially.
- **"End-of-wave batch apply."** — `/tmp` is tmpfs; apply per file.
- **"Audit script from scratch."** — `wiki/build_index.py --check`
  already handles the cases that fool hand-rolled scripts.
- **"Silently update the user-curated body description with the
  GitHub one-liner."** — the body is the more valuable artefact;
  surface the drift and let the user decide.