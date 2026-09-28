# Relocating a vault, wiki, or any note tree to a new path

When the user asks to move a vault/wiki/folder to a new path (consolidate
two trees, move wikis into a vault root, rename a vault), this is a
**content-side migration** that goes beyond just `mv`. The hard part is
that the old path is hardcoded inside the moved files (SCHEMAs, scripts,
templates, memories, READMEs), and you have to decide which references
to update and which to preserve.

The shape of this task is "find every reference to path X, classify each
by role, update the live ones, leave history alone" — and the
classification rules below are what the agent has to get right.

## Always: backup before moving

```bash
tar -czf /tmp/<name>-backup-$(date +%Y%m%d_%H%M%S).tar.gz -C <parent> <name>
ls -la /tmp/<name>-backup-*.tar.gz   # confirm size + presence
```

Verify the backup is non-trivial in size (not 0 bytes, not just a header)
before proceeding. Don't `rm` the source until you've confirmed the
destination is fully populated and the agent can read it. Keep the
backup for at least the rest of the session — if a downstream test
fails, you want a one-step rollback.

## The three classes of in-file path reference

After `mv`, the destination contains files that internally reference the
**old** path. They fall into three classes, and the rule is different
for each:

### Class 1: Active config — UPDATE

- Scripts that hardcode the path as a default argument or constant
  (e.g. `DEFAULT_WIKI = Path("/old/path")`)
- Skill reference docs that describe the workflow using the old path
  as an example
- Memory files (USER.md, MEMORY.md) that record the current state
- User-facing config (yaml, toml) with the path

These define **current** behavior. Leaving the old path makes the agent
act on stale state. Update them.

### Class 2: Live documentation — UPDATE

- `SCHEMA.md` files that say *"this wiki lives at /old/path"*
- Top-level `README.md` routing docs with the path in a table
- Meeting-note templates and other "forward-looking" templates with
  `sources:` pointing at the old location

These are read by future agents as the current spec. Update them, but
keep the prose (e.g. don't rewrite the *meaning* of a SCHEMA — just fix
the path).

### Class 3: Dated history — PRESERVE

- `log.md` files with dated entries like `2026-07-18: created wiki at
  /old/path/aiml/`
- Session dump JSONs that captured the old path in tool inputs/outputs
- Any "audit trail" file where the path is part of a timestamped record

**Rewriting these falsifies the historical record.** A future reader
grepping for "when did we set up the aiml wiki?" should find the path
that was *actually in use at the time*. Update `last updated:` in the
file's frontmatter (if present) but leave dated entries alone.

The decision rule: if the file's primary purpose is to record *what
happened when*, the path is part of the event and should not be
retconned. If the file's primary purpose is to describe *what is true
now*, the path should be current.

## Find every reference, then classify

After `mv`, run a content-level grep across the moved tree and across
the rest of the agent config (skills, memories, config) for the old
path:

```bash
rg -l "<old/path>" <destination>/    # moved tree: Class 1+2+3
rg -l "<old/path>" ~/.hermes/skills ~/.hermes/memories ~/.hermes/config.yaml
```

Each hit gets classified. A good mental shortcut: if removing the file
would lose audit info, it's Class 3. If changing it to use the new path
doesn't change its meaning, it's Class 1 or 2.

## Update active config via the right tool

- **Skill files in `~/.hermes/skills/...`** — use `patch` (mode=replace)
  to swap the old path for the new one, scoped to the specific line
  so unrelated references don't move.
- **Memory files in `~/.hermes/memories/`** — use `patch`; memory
  captures current state.
- **Files in the moved tree** (Class 1+2) — use `patch` per file; for
  each file, confirm the old path appears as a real reference (not in a
  code block, not in a quoted comment from history).
- **MOC notes at the vault root** (e.g. `Me.md`) — if the user moved
  something *into* the vault, add a `[[new-folder]]` wikilink to the
  MOC. This is the part the user sees first; missing it means the new
  tree is invisible from the navigation root.

## Verify before declaring done

After updates, re-grep. The remaining references should be **only**
Class 3 (dated history):

```bash
rg -l "<old/path>" <destination>/ ~/.hermes/   # expected: log.md only
```

For moved wikis, run the wiki's own verification script if one exists
(e.g. `cross_vault_wiki_verify.py`) — its default-arg path now needs
to match the new location, which is itself a Class 1 fix.

For an Obsidian vault, the user-visible test is: open the vault in
Obsidian and confirm the new folder appears in the file tree and the
`[[wikilink]]` from `Me.md` resolves.

## Pitfalls specific to this class of task

- **The `log.md` temptation.** It's tempting to "modernize" log
  entries to use the current path. Don't — it changes what the log
  records. The exception: if a log entry was written AFTER the move
  (i.e. you logged a move event) it should record the new path; the
  pre-move entries should still show the old one.

- **Skills that point at a sub-path of the old location.** When the
  vault/wiki moves, scripts that default to a specific subpath (e.g.
  `DEFAULT_WIKI = /old/path/aiml`) need TWO updates: the docstring
  and the constant. Forgetting the docstring leaves a misleading
  example in the help text.

- **MOC update is the visible part.** The user opens their vault and
  the new tree is invisible if `Me.md` doesn't link to it. This is
  the highest-leverage single edit in the migration — do it last
  but don't skip it.

- **Backup before destructive moves only.** `mv` on the same
  filesystem is reversible (the inode just gets reparented), so
  technically no backup is required. But: a backup tar is the
  one-step rollback if a downstream test catches a missed reference,
  AND it gives the user a clear "before" state to diff against.
  Worth the 30 seconds.

- **Symlinks vs. moves.** If the user later wants the old path to
  keep working (e.g. a tool that hardcodes the old path), consider a
  symlink at the old location pointing to the new one. The user
  should be the one to make this call — don't symlink unprompted,
  because the goal of most moves is to *eliminate* the old path.

## Recipe: move a wiki into a vault

1. `tar -czf /tmp/wikis-backup-...tar.gz -C /home/arctic/hermes wikis`
2. `mv /home/arctic/hermes/wikis /home/arctic/Documents/fun/wiki`
3. Classify every reference (skills, memories, files in moved tree)
4. Patch each Class 1/2 reference; leave Class 3 alone
5. Update the vault's root MOC (`Me.md`) with `[[wiki]]`
6. Re-grep; remaining hits should be only `log.md` files
7. Tell the user to open Obsidian and confirm the new folder appears
8. Keep the backup for the rest of the session
