#!/usr/bin/env bash
# Himalaya passwd-cmd helper — read a provider's App Password from a chmod 600
# file and print it WITHOUT a trailing newline.
#
# Usage: have the himalaya config point at this script:
#     backend.auth.cmd = "/home/arctic/keys/bin/get_imap_passwd.sh"
#
# Copy, rename per provider (get_vit_passwd.sh, get_outlook_passwd.sh, etc.),
# and edit CRED_FILE.

set -euo pipefail

CRED_FILE="${CRED_FILE:-/home/arctic/keys/gmail_app_password.txt}"

if [[ ! -r "$CRED_FILE" ]]; then
    echo "passwd-cmd-helper: cannot read $CRED_FILE" >&2
    exit 1
fi

# CRITICAL: strip trailing newline or himalaya authenticates against
# `the-password\n` and the IMAP server returns a misleading
# "Invalid credentials" — don't waste time debugging it.
tr -d '\n' < "$CRED_FILE"
