# Reconciliation with a stale remote

When the user's vault `.git/` exists but the remote (`origin`) carries
history from a different era — typically a backup snapshot repo that was
abandoned months ago — the obvious "git push" command will fail. This
reference walks through the *exact* error sequence you'll see and the
minimum set of fixes that land a clean push.

## The diagnostic commands (run these first)

```bash
# Is .git present at all?
git -C /path/to/vault rev-parse --is-inside-work-tree
# → "true" if yes

# Any commits?
git -C /path/to/vault log --oneline -1
# → fatal: your current branch 'master' does not have any commits yet
#   (or, if there are commits, the SHA + subject)

# Remote configured?
git -C /path/to/vault remote -v
# → blank if not configured; URLs if it is
```

After this you know which path of the four (`A` / `B` / `C` / `D`) to take.
Path D is "remote has stale history that we'd be wrong to silently overwrite."

## Inspect what's on the remote

```bash
# Quick way (read-only, shallow):
git clone --depth=50 https://github.com/<owner>/<repo>.git /tmp/repo-check
cd /tmp/repo-check && git log --oneline -10 && git ls-tree HEAD
# Note the SIZE of the repo's HEAD (empty tree, or a previous snapshot?).

# Even quicker, via gh if installed:
gh repo view <owner>/<repo> --json name,description,isEmpty,defaultBranchRef,updatedAt
```

`isEmpty: false` in `gh`'s output is **not** a clean signal — it just means
the repo has commits. The tree at HEAD may still be empty if those commits
were all `delete X` operations. Re-confirm with `git ls-tree HEAD`.

## The classic error sequence

When `git push -u origin main` is run against a stale remote, the
most common failure looks like this:

```
To https://github.com/<owner>/<repo>.git
 ! [rejected]        main -> main (stale info)
error: failed to push some refs to '...'
```

The temptation is to retry with `--force-with-lease`. **Don't, yet.**

## Why `--force-with-lease` fails on first push

`--force-with-lease` compares the *cached remote ref* (in
`.git/refs/remotes/origin/main`) against the *server's actual ref*. On a
fresh repo with no prior `git fetch`, the cached ref doesn't exist —
or, more precisely, the lease check uses the local "expected" tip, and
since you've never told git what the expected tip was, the check is
broken by design.

The fix is **one fetch first**:

```bash
git fetch origin     # populates .git/refs/remotes/origin/main
git push --force-with-lease -u origin main    # now safe
```

## When `--force` (no lease) is right

In our case, the remote's HEAD is a stale snapshot from months ago,
and we're about to *replace* it with a completely new history. The
lease comparison buys us nothing — we're not "preserving" anything;
we're replacing it. So:

```bash
git push --force -u origin main
```

`--force-with-lease` would technically also work after a `fetch` here,
but `--force` is clearer about intent. Reserve `--force-with-lease`
for cases where you've been actively tracking the remote across
multiple sessions and want a safety net against someone else's
simultaneous push.

## The "actually check before claiming done" trap

After the push settles, your first instinct will be to type
"Done!" and stop. **Don't.** Run these in order:

```bash
git ls-remote origin HEAD                  # remote SHA == local SHA?
gh api repos/<owner>/<repo>/commits/main   # does GitHub see it?
```

The `gh api` call is the authoritative cross-check — `git ls-remote`
can lie if your local caching is wrong. But the more reliable check
**for the working tree** is `git status` followed by looking for any
files that the app might have written during the operation:

```bash
git status
# Look for "Changes not staged:" — those are .obsidian/ files Obsidian
# regenerated AFTER your commit. They should NOT be in HEAD now. If they
# are, git is including them somehow.
```

If `git status` shows new untracked files that *should be tracked*
(`appearance.json` is the canonical example — it didn't exist when you
staged but Obsidian wrote it during the push), commit them too and
push the follow-up commit. Don't lie about the working tree.

## A end-to-end transcript (from this session)

```
$ git branch -m master main                              # rename branch
$ git add -A && git commit -m "Initial vault snapshot"
[main (root-commit) baf07d3 Initial vault snapshot
 553 files changed, 405559 bytes]
$ git remote add origin https://github.com/<owner>/<repo>.git
$ git push --force-with-lease -u origin main
 ! [rejected]        main -> main (stale info)
$ git fetch origin
 * [new branch]      main       -> origin/main
$ git log origin/main --oneline -3     # verify what's there
ca7fb16 Delete .gitignore
b65ed9c Delete .obsidian directory
$ git push --force -u origin main
 + ca7fb16...baf07d3 main -> main (forced update)
branch 'main' set up to track 'origin/main'.
$ gh api repos/<owner>/<repo>/commits/main --jq '{sha: .sha, files_changed: (.files | length)}'
{"sha":"baf07d3...","files_changed":300}
```

The remote had 5 months of stale history that the user (implicitly)
abandoned. Force-push was the right call — there was nothing worth
preserving on that end.
