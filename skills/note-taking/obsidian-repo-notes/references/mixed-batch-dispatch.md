# Mixed-batch dispatch — GitHub-shape + local-only-shape notes in one parent orchestration

Companion to `obsidian-repo-notes/SKILL.md`. Load this when a single dispatch
batch contains BOTH GitHub-repo notes and local-only project notes — the
two shapes have different schemas, different staging paths, different
MOC targets, and different verification gates. The parent must orchestrate
both shapes coherently: one staging tree, one audit pass, one wiring pass.

## When this applies

- "Add my new GitHub repos AND my local projects to the vault" — the user
  wants both.
- A vault audit surfaces a mix of (a) repos missing vault notes but present
  in the public GitHub listing and (b) local-only `/home/<user>/projects/`
  directories that are not in the GitHub listing.
- The user names a list of projects without distinguishing public-GitHub vs
  local-only; the parent has to classify first.

If the batch is *only* GitHub-shape, route to `references/deep-enrichment-brief.md`.
If only local-only, route to `references/local-only-project-brief.md`. The
mixed-batch case is the third shape, and the most common real-world shape
when the user says "I added a github repo and more local projects" (the
exact phrasing from the 2026-07-31 session that produced this file).

## Pre-dispatch classification (mandatory)

Before dispatching any subagent, classify every project into one of three
buckets. The audit is read-only and runs against:

- `gh api 'user/repos?per_page=100&affiliation=owner&sort=updated'` (the full
  GitHub listing — keep non-fork repos, partition by visibility)
- A filesystem walk of `/home/<user>/projects/` (the local project roots)
- A scan of the vault (`Github/Repos/*.md`, `Projects/*.md`) for existing notes

Cross-reference to produce a classification table with three columns:

| Project | GitHub? | Vault note present? | Action |
|---|---|---|---|
| `cutiepie` | yes (public) | no | GitHub-shape dispatch to `github-notes/` |
| `obsidian-galaxy-graph` | no (local-only) | no | Local-only dispatch to `local-notes/` |
| `cutipie` | — | — | (a local-dir typo for `cutiepie`; flag as `Local dir misspelled`) |
| `build-your-own-harness` | yes (renamed) | yes (as `claude-code-in-100-lines`) | **SKIP — duplicate** |
| `claude-code` | external fork of `anthropics/claude-code` | no | **SKIP — external clone** |

Three outcomes fall out of the table:

1. **GitHub-shape** → dispatch subagent to write to `kg-sync-tmp/github-notes/<slug>.md`
2. **Local-only** → dispatch subagent to write to `kg-sync-tmp/local-notes/<slug>.md`
3. **Skip** → record reason (duplicate, external clone, empty container, typo
   of another project, file-not-a-project)

Always run the full classification *before* dispatching anything. A
subagent dispatched to write a `build-your-own-harness` note that turns
out to be a renamed duplicate wastes ~5 minutes of work plus a duplicate
note in the staging tree that the parent has to reconcile.

## Single staging tree with two sibling sub-folders

```bash
STAGE=/home/<user>/projects/kg-sync-tmp
mkdir -p "$STAGE/github-notes" "$STAGE/local-notes"
```

The `kg-sync-tmp/` folder (NOT `/tmp/`) is on durable disk and survives
across sessions. The two sibling sub-folders match the two shape categories
so the parent's apply step is one `cp -r` per sub-folder.

The folder is the *only* output path the parent tells subagents about.
Each subagent's brief ends with:

> Output: `<STAGE>/github-notes/<slug>.md` OR `<STAGE>/local-notes/<slug>.md`
> (one or the other per the brief — never both)

A subagent writing to the wrong sub-folder is the most common mixed-batch
bug — the parent's apply step will silently miss it because the MOC wiring
walks each sub-folder independently.

## Single audit pass (post-dispatch, pre-apply)

The parent runs *one* audit pass over both sub-folders, in this order:

1. **GitHub-shape audit** (per `references/deep-enrichment-brief.md`):
   frontmatter has `repo`, `description`, `github: <url>`, `visibility: public|private`,
   `language`, `topics`, `stars`, `forks`, `status`, `created`, `last_pushed`,
   `type`, `vault_group: Public Repos | Private Repos`. Body has the
   11-section schema. `## Links` ends with `Part of: [[Public Repos]]` or
   `Part of: [[Private Repos]]`.

2. **Local-only-shape audit** (per `references/local-only-project-brief.md`):
   frontmatter has `repo`, `description`, `local_path:`, `github: none`,
   `visibility: local`, `language`, `topics`, `git_initialized:`, `status`,
   `created`, `last_updated`, `type`, `vault_group: Projects`. Body has the
   10-section shape (no `## Stats`). `## Links` ends with
   `Part of: [[Projects]]`. **No `GitHub:` line.**

3. **Cross-link audit**: for every wikilink in every new note, confirm the
   target resolves to a real file in the vault. Build the union of
   unresolved targets and **create concept stubs to back them** (see next
   section).

4. **Vault untouched**: `git status --short` against the vault must NOT
   show any of the new notes written directly. Only the parent's
   copy-after-apply step should touch the vault.

## Concept-stub backfill for new cross-links (mandatory)

New project notes frequently link to `[[Concept Name]]` notes that
don't exist yet (the parent doesn't pre-create concept notes for every
project because that's a separate burn). When the audit surfaces N
unresolved concept targets:

- For each target: create `<VAULT>/Concepts/<Name>.md` with the
  minimum-viable frontmatter:

  ```markdown
  ---
  type: concept
  status: stub
  created: YYYY-MM-DD
  ---

  # <Concept Name>

  > Concept stub — placeholder for an upcoming concept note. Referenced by
  > newer project notes (`Projects/*`); a real definition will land in a
  > follow-up burn.
  ```

- 30 stubs is a typical batch (the 2026-07-31 session created 30 in one
  pass for the 12-project batch).
- These stubs are deliberate placeholders, NOT bugs to fix in the same
  session. They follow the vault's `CLAUDE.md` rule "Many notes are
  intentionally empty stub files (0 bytes) — placeholders reserved for a
  topic". The one-line body and `status: stub` frontmatter make them
  findable and machine-tagged without committing to substantive content
  the user didn't ask for.

After the stubs exist, re-run the cross-link audit; the count should
drop to 0 (or to the pre-existing broken-link baseline).

## MOC wiring — one pass, two files

The new notes go to *different* MOCs based on shape:

- GitHub-shape: `Github/Public Repos.md` (or `Github/Private Repos.md`) —
  add a one-line entry per new note under the existing list.
- Local-only: `Projects/Projects.md` — add a one-line entry per new note.

If the user has *both* MOC layers (a hand-vault root like `Projects/Projects.md`
and an agent-maintained wiki like `wiki/personal/facets/projects.md`),
also wire the new local notes into the wiki's "Side projects" / "Project
clusters" section per `vault-knowledge-graph` golden rule 5a (cluster
reciprocation: when adding new projects, the central entity + facets
should reference them so the cluster gains incoming edges).

A bug the parent must avoid: writing a single-line
`- [[github-project]]` entry into `Projects/Projects.md`. The MOC path is
shape-specific — GitHub-shape notes never go into `Projects/Projects.md`
and local-only-shape notes never go into `Github/Public Repos.md`.

## Apply step — two separate `cp -r` calls

```bash
# GitHub-shape apply
for f in "$STAGE/github-notes"/*.md; do
  slug=$(basename "$f" .md)
  cp "$f" "<VAULT>/Github/Repos/${slug}.md"
done

# Local-only apply
for f in "$STAGE/local-notes"/*.md; do
  slug=$(basename "$f" .md)
  cp "$f" "<VAULT>/Projects/${slug}.md"
done
```

Always `cp`, never `mv` — keep the staged files around as a backup until
the vault passes `wiki/build_index.py --check` with `broken=0` (or at the
pre-existing baseline).

## Mid-flight overwrite warning

When sibling subagents run in parallel, `write_file` emits a warning when
the parent's `write_file` call lands on a path a sibling modified since
the parent's last `read_file` of that path. The warning is:

> "was modified by sibling subagent 'sa-0-XXXXXXXX' but this agent never
> read it. Read the file before writing to avoid overwriting the sibling's
> changes."

This is a real coordination hazard in the mixed-batch case where the
parent and one or more subagents write to *different* sub-folders but
share the same `kg-sync-tmp/` parent. The reliable pattern:

- The parent reads the file with `read_file` immediately before its own
  `write_file`/`patch` to that path.
- If the parent is rewriting a file the parent previously staged (e.g.
  fixing a frontmatter field), re-read first.
- If two subagents write to the *same* file, that's a brief duplication
  — fix by ensuring briefs are slug-distinct and the parent doesn't
  assign the same slug to two subagents.

In the 2026-07-31 session, the parent wrote `obsidian-galaxy-graph.md`,
`rocky_code.md`, `openresearch.md`, `fast_qwen.md` while a subagent
(`deleg_79fdd30b`) was concurrently writing `hf.md`, `hardcore.md`,
`rqlite.md`. Both arrived at the same files but with distinct content;
no overwrite happened because the briefs were distinct and the parent
opted to write its files directly (the local-only notes were too detailed
to delegate without risking drift). The warning fired on three files —
the parent's writes still succeeded because `write_file` is
last-writer-wins and the parent had read the file before writing (the
sibling's most-recent read at that point was during its own write, not
a separate read).

## Skip categories — explicit reasons

When a project is classified as **Skip** in the pre-dispatch table, the
parent records the reason explicitly. Five canonical skip categories
discovered across sessions:

| Skip category | Detection | Action |
|---|---|---|
| **GitHub-renamed duplicate** | `gh repo view <owner>/<slug>` resolves to a different repo; both local checkouts share HEAD | Add a one-line cross-link from the canonical note's `Related:` instead of writing a second note |
| **External upstream clone** | `git remote -v` shows a non-`<owner>` URL (`github.com/<other-org>/<repo>`) | Skip — the user's local clone is reference material, not a project to track as theirs |
| **Empty container** | Directory exists but contains only nested projects or scratch | Skip |
| **Local-dir typo** | `/home/<user>/projects/<typo>` exists alongside `/home/<user>/projects/<canonical>` with `git remote -v` matching the typo'd URL | Note the misspelling in the canonical note's body; don't create a second note |
| **File-not-a-project** | `/home/<user>/projects/<dir>` contains only `node_modules`, `.venv`, `target`, etc. | Skip |

The 2026-07-31 session hit all five:
- `build-your-own-harness` → GitHub-renamed duplicate (renamed to `claude-code-in-100-lines`; identical HEAD)
- `claude-code`, `opencode`, `gemini-cli`, `docling`, `doclang`, `gpt-based-miniature-python-code-completion-model` (wait — that one is a real GitHub repo) → external upstream clones
- `coding/` → empty container (zero files)
- `cutipie/` → local-dir typo of `cutiepie`
- `pokemon/.libs/numpy`, `paper-reproduction/.trackio/logbook` → not projects

## Final reply format (mixed batch)

Print, in this order:

1. **Classification table** (the pre-dispatch table from §1)
2. **Staging tree state** (`ls -la` of `kg-sync-tmp/{github-notes,local-notes}`)
3. **Apply summary** (files created, files updated, files skipped — with
   the skip reason for each)
4. **Verification** (`wiki/build_index.py --check` count of new broken
   links = 0; pre-existing broken-link baseline preserved)
5. **Concept stubs created** (count + sample list — e.g. "30 stub
   Concepts/ notes to back cross-links; sample: [[Bun]], [[Modal]], ...)

## Pitfalls recap

| Pitfall | Mitigation |
|---|---|
| Dispatching a subagent for a GitHub-renamed duplicate | Pre-dispatch `gh repo view`; check `git rev-parse HEAD` against the canonical local checkout |
| Subagent writing to the wrong sub-folder | Brief explicitly says `<STAGE>/github-notes/` or `<STAGE>/local-notes/`; parent audits by sub-folder |
| New project notes break cross-link graph | Concept-stub backfill pass after the audit, before MOC wiring |
| MOC wiring a GitHub note into `Projects/Projects.md` (or vice versa) | MOC wiring pass is shape-aware |
| Concept stub becomes a real content liability | Stubs carry `status: stub` + `type: concept`; one-line body; matches `CLAUDE.md` placeholder rule |
| Mid-flight sibling-overwrite warning fires | Parent re-reads the file before its own write; briefs are slug-distinct |
| Parent writes a note directly that a subagent had also written | Pick one writer per file: either the parent or the subagent owns each slug; don't double-write |
| Counting "all GitHub repos" without filtering forks | `affiliation=owner` filters forks in the API call; verify count matches `gh repo list --no-archived` for sanity |
| Treating `/home/<user>/projects/<dir>` as a project when it's a vendored subtree (`<project>/opencode`, `<project>/node_modules`, `<project>/.libs/`) | Walk the first-level only; ignore nested vendored subtrees unless the user explicitly asks |