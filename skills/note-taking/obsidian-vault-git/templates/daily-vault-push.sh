#!/usr/bin/env bash
# daily-vault-push.sh — chained after vault-writing crons (e.g. daily-memory at 01:00).
#
# Contract with the cron gateway (no_agent=true):
#   - empty stdout    → SILENT  (no user message; this is the watchdog pattern)
#   - non-empty stdout → delivered verbatim as the cron message
#   - non-zero exit   → error alert delivered to chat
#
# We deliberately stay silent on a successful push:
#   (a) the daily-memory cron's own summary already covers "the vault is updated";
#   (b) a daily "pushed OK" ping at 01:15 is noise — the user is asleep;
#   (c) failures are the only signal worth waking up to.
#
# Usage:
#   1. Copy this file to ~/.hermes/scripts/daily-vault-push.sh  (cron scheduler
#      only accepts scripts from ~/.hermes/scripts/, with basename only)
#   2. chmod +x ~/.hermes/scripts/daily-vault-push.sh
#   3. Register via cronjob create with no_agent=true, script=daily-vault-push.sh,
#      workdir=$VAULT_PATH, schedule=15 1 * * * (or whatever offset matches your cron)
#
# Required for HTTPS push: ~/.git-credentials must be readable by the cron user,
# and `git config credential.helper=store` must be set.

set -euo pipefail

VAULT="${VAULT:-/home/arctic/Documents/fun}"
DATE="$(TZ=Asia/Kolkata date +%F)"   # YYYY-MM-DD in IST — change TZ for non-IST users

cd "$VAULT"

# Sanity: are we on the right branch and is the remote wired?
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
    echo "ERROR: expected branch 'main', got '$BRANCH' — aborting push" >&2
    exit 2
fi

if ! git remote get-url origin >/dev/null 2>&1; then
    echo "ERROR: no 'origin' remote configured" >&2
    exit 3
fi

# Capture status before mutating anything.
CHANGES_BEFORE="$(git status --porcelain)"
UNPUSHED_BEFORE="$(git log --oneline 'origin/main..HEAD' 2>/dev/null || true)"

if [[ -z "$CHANGES_BEFORE" && -z "$UNPUSHED_BEFORE" ]]; then
    # Nothing to do. Stay silent (empty stdout → cron gateway stays silent).
    exit 0
fi

# Stage everything not ignored and commit. -q keeps output clean.
if [[ -n "$CHANGES_BEFORE" ]]; then
    git add -A
    git -c core.commentchar="#" commit -q -m "Daily vault snapshot: ${DATE}"
fi

# Push. Force-push is unnecessary here — the remote has no concurrent writers
# and this script is the single pusher for this vault.
git push -q origin "$BRANCH"

# Stay silent on success. Any error above already triggered an alert.
exit 0
