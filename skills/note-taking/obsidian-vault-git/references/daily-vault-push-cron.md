# Daily vault-push cron — wiring reference

Companion to `templates/daily-vault-push.sh`. Covers the cron registration,
why script-only beats LLM-driven, the silent-on-success contract, the
credential helper requirement, the smoke-test recipe, and the upstream-cron
prompt edits that have to land alongside.

## Why script-only, not LLM-driven

A "push the vault" task has three meaningful outcomes:

| Outcome       | LLM-driven cron                 | Script-only cron (`no_agent=true`) |
|---------------|---------------------------------|------------------------------------|
| No-op day     | Burns tokens to write "nothing to push" | Silent. Stays silent.       |
| Successful push | Buried under other deliverables; mostly noise | Silent. Stays silent.    |
| Push failed   | Model retries within cron window, possibly exhausts it | Non-zero exit → alert delivered immediately |

The fail-fast property of the script-only path is the whole point. Pushes
shouldn't depend on a model API being up; they should depend on the network
and GitHub being up, and wake the user only when one of those is down.

## The cron job JSON

```jsonc
{
  "name": "daily-vault-push",
  "schedule": "15 1 * * *",            // 01:15 IST — 15 min after a 01:00 daily-memory cron
  "script": "daily-vault-push.sh",     // basename; must live in ~/.hermes/scripts/
  "no_agent": true,
  "deliver": "origin",
  "workdir": "/home/arctic/Documents/fun"
}
```

**Why these specific values:**

- **`15 1 * * *`** — fires 15 minutes after a 01:00 upstream cron. The offset
  matters: if there's a `daily-memory-yesterday` cron at `0 1 * * *`, the
  push needs to land *after* it finishes writing, otherwise the snapshot
  is partial. A 10–15 minute buffer prevents racy double-writes without
  feeling laggy to a user checking GitHub the next morning.
- **`script: "daily-vault-push.sh"`** — basename, *not* absolute path. The
  Hermes cron scheduler enforces the script must live in
  `~/.hermes/scripts/`. Use just the filename; otherwise the create call
  is rejected with *"Script path must be relative to ~/.hermes/scripts/"*.
- **`no_agent: true`** — silent on success, alert on failure. See the
  watchdog semantics table above.
- **`deliver: "origin"`** — auto-route to the chat that asked for the cron;
  if the cron is set up in a Telegram session, errors route there. For
  multi-channel delivery (Telegram + a backup inbox), use `"all"` or a
  specific `platform:chat_id` tuple.
- **`workdir`** — the vault root. The script `cd`s into it but `workdir`
  sets the directory the agent-less task runs from; check that AGENTS.md
  in this directory doesn't override behavior (it won't, since
  `no_agent=true` skips the agent, but it's a clean habit).

## Credential helper requirement (HTTPS pushes)

For non-interactive `git push` over HTTPS, two things must be true:

1. `~/.git-credentials` contains the GitHub token URL, mode 600, owned by
   the user the cron runs as:
   ```
   https://<user>:<token>@github.com
   ```
2. Git's credential helper is set to `store` (so git knows to read that file):
   ```bash
   git config --global credential.helper store
   ```

To verify both work non-interactively *before* scheduling the cron:
```bash
git -C "$VAULT" push --dry-run origin main
# expect: "Everything up-to-date" (or a similar non-error message)
```

A failed credential lookup on cron fire is the single most common reason a
"daily push" stops working silently. If the user moves off `gh` CLI auth
or rotates their token, `~/.git-credentials` needs a refresh — the cron
won't tell you; you'll just notice the remote hasn't moved in a week.

## Smoke-test recipe (run BEFORE registering the cron)

This catches the three classes of failure that bite hardest later: missing
credential, wrong branch, broken script syntax.

```bash
# 1. Lint the script
bash -n ~/.hermes/scripts/daily-vault-push.sh
# expect: no output, exit 0

# 2. Run with a dirty working tree (artificial change)
TMP=$(mktemp)
echo "smoke-test-$(date +%s)" > "$VAULT/cron-smoke-test.md"
bash ~/.hermes/scripts/daily-vault-push.sh
echo "exit=$?"
# expect: exit=0, silent stdout

# 3. Confirm the push landed on the remote
git -C "$VAULT" log --oneline -2
gh api repos/<owner>/<repo>/commits/main --jq '.sha + " " + .commit.message'
# expect: latest commit is "Daily vault snapshot: $(date +%F)"

# 4. Clean up the test file and re-push (or just delete locally — it'll land on the next push)
rm "$VAULT/cron-smoke-test.md"
bash ~/.hermes/scripts/daily-vault-push.sh
# expect: another silent commit that removes the test artifact
```

If step 2 produces *any* stdout (success message) or non-zero exit, the cron
will wake you up on the wrong days. Fix before registering.

## Edits to the upstream vault-writing cron's prompt

If you wire a daily-vault-push cron and there's another cron that writes
vault content (e.g. `daily-memory-yesterday`), that writer's prompt needs
two changes:

1. **Add the fact that the vault is git-tracked**, with the remote URL.
   The LLM will infer "I shouldn't try to git push" from this; without it,
   the writer may try to push mid-write and race against the push cron.

2. **Add a hard rule against running git from inside that cron:**
   > **Do NOT run git.** The vault is tracked, but a separate
   > `daily-vault-push` cron at 01:15 IST owns commits and pushes. Don't
   > `git add`, don't `git commit`, don't `git push` from this prompt — just
   > write the note.

3. **Update the "What you CAN skip" section** to mention the push cron:
   > - Do not run `git` (the `daily-vault-push` cron handles that).

This pattern generalizes: any time you split a workflow across "writer"
and "publisher" crons, the writer's prompt needs an explicit prohibition
to prevent it from doing the publisher's job. The cron-prompt-hygiene
reference expands on this as a meta-rule.

## Updating the cron via Hermes

```python
# register
cronjob(action="create", name="daily-vault-push", schedule="15 1 * * *",
        script="daily-vault-push.sh", no_agent=true, deliver="origin",
        workdir="/home/arctic/Documents/fun",
        prompt="Daily vault backup push... (see SKILL.md for the template)")

# inspect
cronjob(action="list")  # find job_id, verify next_run_at, schedule, script

# tear down (before you delete ~/.hermes/scripts/daily-vault-push.sh)
cronjob(action="remove", job_id="<id>")
```

Crons are persisted in `~/.hermes/cron/jobs.json`. Inspecting that file
directly (it's a JSON document) is sometimes faster than `cronjob(action="list")`
for finding the prompt body of a long-running cron.

## End-to-end transcript (from this session)

```
$ bash -n ~/.hermes/scripts/daily-vault-push.sh        # lint
syntax OK

$ # clean tree (expect: silent exit 0)
$ bash ~/.hermes/scripts/daily-vault-push.sh
$ echo $?
0

$ # dirty tree (expect: commit + push, silent exit 0)
$ echo "cron-smoke-test-$(date +%s)" >> /home/arctic/.../cron-smoke-test.md
$ bash ~/.hermes/scripts/daily-vault-push.sh
$ echo $?
0

$ git -C /home/arctic/Documents/fun log --oneline -2
0614f85 Daily vault snapshot: 2026-08-03
17f9e81 Track .obsidian/appearance.json and graph.json updates

$ gh api repos/adityasasidhar/obsidian/commits/main --jq '.sha + " " + .commit.message'
0614f852427b96f0691739180a4cb2cada827b74 Daily vault snapshot: 2026-08-03

$ # clean up the test marker
$ rm /home/arctic/.../cron-smoke-test.md
$ bash ~/.hermes/scripts/daily-vault-push.sh
$ # second commit landed; remote HEAD now holds the cleanup
```
