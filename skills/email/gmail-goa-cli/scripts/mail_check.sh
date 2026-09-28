#!/bin/bash
# mail_check — periodic Gmail check that pushes a short Telegram summary.
# Reuses ~/.local/bin/mail_check (the Python XOAUTH2 client). This wrapper
# exists only to satisfy the cron scheduler's "scripts under ~/.hermes/scripts/"
# requirement. Cron sessions have a minimal PATH, so we set the things we
# need explicitly.
set -euo pipefail

# Cron's PATH is bare — make sure we can find hermes (which the script
# itself calls via subprocess).
export PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

exec /home/arctic/.local/bin/mail_check --verbose

