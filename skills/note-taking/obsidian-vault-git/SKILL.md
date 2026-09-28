---
name: obsidian-vault-git
description: Use when initializing or repairing git on an Obsidian vault.
version: 1.1.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
changelog: '1.1.0: Added "After init: schedule the daily push" section with the script-only-cron pattern, hardened the "working tree clean" rule into a TWICE check, added git-identity pre-check, added cron-prompt-hygiene pitfall + reference, added the daily-vault-push.sh template and two new reference docs. 1.0.0: Initial release with the four-path diagnostic and gitignore contract.'
---

# Obsidian Vault Git Initialization

Use this skill when the user asks to "initialize git" / "put my vault under version control" / "track this vault in git" / "wire this vault up to GitHub" / "fix the git on my Obsidian vault" / "there's already a .git but it's broken." Also fires for proactive setup: a new vault at `~/Documents/Obsidian Vault/` (or equivalent) that has no `.git`.

The most common first failure is **doing what the user said instead of what they meant.** "Initialize git" sounds like `git init`, but a `.git/` may already exist — with no commits, no remote, and a stale history on GitHub. Blindly running `git init` again would wipe the existing reflog. **Always diagnose first.**

## Golden rules

1. **Diagnose before doing.** Run these three commands in parallel before *any* mutation:
   ```bash
   git -C /path/to/vault rev-parse --is-inside-work-tree   # is .git present?
   git -C /path/to/vault log --oneline -1 2>&1 | head -5   # any commits?
   git -C /path/to/vault remote -v                         # any remote?
   ```
   The output shape determines the entire plan:
   - `.git` absent → `git init -b main` flow (path A below)
   - `.git` present, no commits, no remote → write gitignore + commit (path B)
   - `.git` present, has commits, no remote → just add remote + push (path C)
   - `.git` present, has remote, history is fresh → just push (path C)
   - `.git` present, has remote, history is stale → reconciliation flow (path D)

2. **Default branch = `main`** (not `master`). GitHub's default since 2020; new repos use it. Use `git init -b main` for fresh, `git branch -m master main` to fix existing.

2a. **Verify `git` identity is set before any commit.** `git -C /path commit` silently produces an empty commit or errors with `Please tell me who you are` if no global user.name/user.email is configured. Quick check:
   ```bash
   git config user.name && git config user.email
   ```
   If blank, set globally before committing, or pass `-c user.name=... -c user.email=...` per-command. Don't proceed and hope — a failed commit is silent and confusing.

3. **Never claim "working tree clean" until `git status` confirms it (TWICE).** Obsidian writes config files asynchronously (`.obsidian/workspace.json`, `.obsidian/appearance.json`) — if you stage everything then immediately check status, you'll see new untracked files that appeared after the stage. **Run `git status` once before commit, then a second time after.** If a `graph.json` or `appearance.json` write lands during the commit/push window, follow up with a second commit so the working tree is actually clean before you report done. This is why the verifier's tree cross-check matters — it would catch a `.obsidian/appearance.json` that exists on disk but isn't in HEAD.

4. **The gitignore is the deliverable, not the commit.** A vault whose `.gitignore` is wrong gets re-broken every time Obsidian writes a workspace file. Spend the time to make the 11-ignore / 14-track contract work; subsequent commits are trivial.

5. **Stale remotes need `--force` (with fetch first), not `--force-with-lease`.** `--force-with-lease` compares against the cached remote ref, which is *uninitialized* if you never fetched. The push fails with "stale info." The fix is one `git fetch origin` first; only use `--force-with-lease` if you've been tracking the remote across multiple sessions.

## The four paths

### Path A: Cold start (no `.git/`)

```bash
cd /path/to/vault
# Write .gitignore FIRST (see "The gitignore contract" below)
git init -b main
git add -A
git commit -m "Initial vault snapshot"
git remote add origin git@github.com:<owner>/<repo>.git   # or https://
git push -u origin main
```

### Path B: `.git/` present, empty (the most common case)

The user said "initialize git" but the repo is already half-born. Sequence:

1. Write `.gitignore` (don't stage yet)
2. `git branch -m master main` if HEAD has no commits and you're on `master`
3. `git add -A && git commit -m "Initial vault snapshot"`
4. `git remote add origin …`
5. `git fetch origin` (necessary even for first push — see golden rule 5)
6. `git push --force -u origin main`

The `--force` (not `--force-with-lease`) is correct here because the remote has stale history that's about to be replaced. See `references/reconciliation-with-stale-remote.md` for the full dance, including why `--force-with-lease` rejects on first push.

### Path C: `.git/` present, has commits, just needs a remote

```bash
git remote add origin git@github.com:<owner>/<repo>.git
git push -u origin main
```

No force needed. If this is the user's first push to an empty repo, drop the `-u` set-uptracking to verify the push first.

### Path D: `.git/` present, remote has stale history (e.g. an old backup snapshot)

Use **path B's flow** but explicitly tell the user what the remote currently has, and that you're about to nuke it. The user's request "initialize git" can mean "start tracking from today, ignore the past" — but it can also mean "add the current vault to the existing remote, keeping history." **Don't force-push without asking** when the remote has commits the user authored. Offer:

- **Force-push:** one fresh commit replacing all old history. Cleanest; loses the past.
- **Preserve history:** `git pull origin main --allow-unrelated-histories`, merge, then push. The two histories coexist.
- **New repo name:** if the old remote name is generic ("obsidian", "notes", "vault"), it may collide with another user's repo — recommend renaming before pushing.

## The gitignore contract

A correct Obsidian-vault gitignore has two zones:

**Zone 1: ignore** (per-machine, regenerate):
- `.obsidian/workspace.json` — Obsidian's main per-machine layout file
- `.obsidian/workspace.json.bak` — its backup
- `.obsidian/graph.json.bak` — graph-view backup
- `.claudian/sessions/` — Claudian plugin's session metadata
- `Untitled.md` — Obsidian's default scratch note at vault root
- OS noise: `.DS_Store`, `Thumbs.db`, `*.swp`, `*.swo`, `*~`
- Trash: `.trash/`, `*.bak/`, `*.tmp/`

**Zone 2: track** (cross-machine sync, override the absence of a parent rule):
- `.obsidian/app.json` — base app config
- `.obsidian/community-plugins.json` — list of community plugins installed
- `.obsidian/core-plugins.json` — core plugin enable/disable
- `.obsidian/hotkeys.json` — keybind map
- `.obsidian/page-preview.json` — preview behavior config
- `.obsidian/webviewer.json` — web viewer config
- `.obsidian/snippets/` — CSS snippets (often file-backed)
- `.obsidian/themes/` — custom theme dir
- `.obsidian/plugins/` — community plugin payloads
- `.obsidian/graph.json` and `.obsidian/appearance.json` — config that *looks* per-session but is identical across an owner's machines

A reference gitignore proving all 25 rules at once: `templates/vault-gitignore`.

The 25-case verifier (run after every edit to the gitignore): `scripts/verify_vault_gitignore.py`. Pass criteria: 0 failures, AND `git ls-tree -r HEAD --name-only` cross-check (no `.obsidian/workspace.json` in HEAD, no `Untitled.md` / `fun.md` in HEAD, `.gitignore` and `Me.md` *are* in HEAD). The verifier catches the category of bug where a path is in the "ignore" comment block but the file is also committed anyway — silent contradictions.

## Common pitfalls (real, observed)

- **Workspace.json appears in the commit anyway.** You wrote `.obsidian/workspace.json` in the ignore list, but then `git status` shows it modified. Cause: Obsidian re-writes it after your commit. Fix: it's truly ignored (the rule works); the modification just lives unstaged. Confirm with `git check-ignore -v .obsidian/workspace.json`.
- **Appearance.json was committed the first time but ignored the second.** You moved it between sections. Cause: git's index still has the tracked version, and the new rule now says ignore — `check-ignore` reports 0 (ignored) but the file is still in HEAD until you `git rm --cached`. Fix: `git rm --cached .obsidian/appearance.json` after changing its rule class.
- **`git init` rejects with "fatal: cannot reinitialize."** You tried `git init` on top of an existing `.git/`. Don't. Diagnose first (golden rule 1) and choose the right path.
- **`--force-with-lease` rejected with "stale info."** Fix is `git fetch origin` then re-push. See `references/reconciliation-with-stale-remote.md`.
- **`git add -A` failed with `.git/index.lock: File exists.`** A previous git process crashed or is still running. `rm -f .git/index.lock` is safe if no git process is actually running. Verify with `pgrep -af git` first.
- **Branch is `master`, GitHub default branch is `main`.** Old repos default to `master`. Rename before push: `git branch -m master main`. Push errors will cascade otherwise ("src refspec main does not match any" or "non-fast-forward").
- **The vault CLAUDE.md / AGENTS.md declares an empty stub rule.** Read it before doing bulk work — Obsidian vaults routinely say "don't fill in 0-byte files" or "wikilinks use [[Note Name]]" rules that override what your diagnostic scripts suggest.
- **Empty `.gitignore` exists at vault root.** Treat as "no rules yet" — write the full contract from the template. Don't blow it away — a present-but-empty gitignore means the author *intended* to use one.
- **Daily-memory cron says "do not run git" after the vault becomes git-tracked.** Stale rule from the pre-tracked era. Audit every active cron prompt in `~/.hermes/cron/jobs.json` whenever infrastructure state changes (vault tracked, new tools wired, etc.); otherwise the LLM obediently follows a contradicting rule. See `references/cron-prompt-hygiene.md`.

## After init: schedule the daily push

Tracking the vault is step one; keeping the remote in sync without the user thinking about it is step two. The cleanest pattern is a **script-only cron** (`no_agent=true`) that fires once a day, checks for local changes, and pushes if dirty.

**Why script-only, not an LLM-driven cron?** Push failures are the only signal worth waking the user for. The watchdog semantics of `no_agent` give exactly that:

- Empty stdout → cron gateway stays silent (no noise on a no-op day, while the user is asleep).
- Non-empty stdout → delivered verbatim as the cron message.
- Exit non-zero → error alert delivered to chat.

An LLM-driven push cron would burn tokens and could fail to push because of model API issues even when the network is fine.

**The script template:** `templates/daily-vault-push.sh`. Copy it to `~/.hermes/scripts/` (the only directory the cron scheduler accepts), `chmod +x`, then register the cron.

**Wiring:**

```jsonc
{
  "name": "daily-vault-push",
  "schedule": "15 1 * * *",            // 01:15 IST — 15 min after a 01:00 daily-memory cron
  "script": "daily-vault-push.sh",     // basename; must live in ~/.hermes/scripts/
  "no_agent": true,
  "deliver": "origin",
  "workdir": "/path/to/vault"
}
```

**Schedule offset matters.** If the user has other vault-writing crons (e.g. `daily-memory-yesterday` at `0 1 * * *`), schedule the push with a 10–15 minute offset so writes settle before the push. The push script will catch whatever landed; the buffer just prevents racy double-writes.

**Update the upstream cron prompts too.** If you wire a daily push, the upstream cron that *writes* vault content should be told explicitly not to run git (a separate cron owns it). Otherwise the writer may `git add` mid-write and the push cron sees an inconsistent state. See the daily-memory prompt example in `references/daily-vault-push-cron.md`.

See `references/daily-vault-push-cron.md` for the full cron JSON, the credential-helper requirement (HTTPS pushes need `credential.helper=store` with `~/.git-credentials` readable by the cron user), and the smoke-test recipe.

## Reference index

- **`templates/vault-gitignore`** — the ready-to-copy .gitignore (with negation rules for cross-machine config). Verified by `scripts/verify_vault_gitignore.py`.
- **`templates/daily-vault-push.sh`** — the bash script for the daily push cron (silent on no-op; alerts on failure).
- **`scripts/verify_vault_gitignore.py`** — the 25-case verifier. Run after every gitignore edit, and after every Obsidian upgrade (plugin payloads can shift).
- **`references/reconciliation-with-stale-remote.md`** — full transcript of the `--force-with-lease` failure, the `git fetch` fix, and when `--force` is the right answer vs not.
- **`references/daily-vault-push-cron.md`** — cron JSON shape, watchdog semantics, credential helper, schedule-offset reasoning, and the smoke-test recipe.
- **`references/cron-prompt-hygiene.md`** — when infrastructure state changes, audit every active cron prompt for contradicting rules; this is how the daily-memory "Do not run git" rule came to be patched.

## Relevant skills

- `obsidian` (bundled) — base vault read/write conventions for the actual content.
- `vault-knowledge-graph` — what to do *with* the vault once it's tracked.
- `github-pr-workflow` — only if the vault is hosted on GitHub and the user later wants PR-style workflows for it.
