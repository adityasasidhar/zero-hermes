---
name: gmail-goa-cli
description: "Read & send Gmail from the terminal using a Python XOAUTH2 client that reuses the GNOME Online Accounts OAuth2 token. Use when himalaya is unavailable (no +oauth2 build feature), or when you want a minimal no-deps mail CLI for read/inbox/search/send."
version: 1.0.0
author: hermes
license: MIT
platforms: [linux]
metadata:
  hermes:
    tags: [Email, IMAP, SMTP, OAuth2, GNOME, GOA, Gmail, XOAUTH2]
    replaces: ["himalaya"]  # when himalaya is built without +oauth2
prerequisites:
  commands: [gdbus, python3]
---

# Gmail via GNOME Online Accounts (Python XOAUTH2)

When a Linux user has Gmail configured through **GNOME Online Accounts (GOA)**, you can read and send mail from the terminal without an app password or any new login — the live OAuth2 access token sits in the GOA daemon and is exposed over D-Bus.

This skill ships two scripts:

- `~/.local/bin/goa-mail-token` — fetches a fresh XOAUTH2 token from GOA. Returns the SASL initial-response payload (raw, unencoded).
- `~/.local/bin/mail` — a thin `inbox/read/search/send/folders/whoami` CLI that does IMAP/SMTP via XOAUTH2 in ~300 lines of stdlib-only Python.

## Why this and not himalaya

`himalaya v1.2.0` is the version installed at `~/.local/bin/himalaya`, but it's built **without the `+oauth2` cargo feature**. Using `auth.type = "oauth2"` in the config errors with `missing `oauth2` cargo feature`. Rebuilding himalaya with `--features oauth2` needs a Rust toolchain and 3–5 min. On a stock Linux box the Python client is faster to deploy and easier to audit.

## Prerequisites

1. `gdbus` on PATH (always present on GNOME).
2. A GOA account with `MailEnabled=true`:
   `awk -F= '/\[Account /{acc=$2}/Identity=/{print acc, $2}' ~/.config/goa-1.0/accounts.conf`
3. GNOME session is active (this is a hard dependency — won't work from a headless/SSH session).

## Setup

```bash
# 1. The token helper — auto-discovers the GOA account from accounts.conf
cat > ~/.local/bin/goa-mail-token << 'PYEOF'
#!/usr/bin/env python3
"""Emit the XOAUTH2 SASL initial-response payload for the configured GOA Gmail account."""
import configparser, os, re, subprocess, sys
from pathlib import Path

CONF = Path.home() / ".config/goa-1.0/accounts.conf"

def main():
    cp = configparser.RawConfigParser()
    cp.optionxform = str
    cp.read(str(CONF))
    target = os.environ.get("GOA_EMAIL", "").strip()
    account = None
    for sec in cp.sections():
        d = dict(cp.items(sec))
        if d.get("MailEnabled") != "true":
            continue
        if target and d.get("Identity") == target:
            account = (sec[len("Account "):], d["Identity"])
            break
        if account is None and d.get("Provider") == "google":
            account = (sec[len("Account "):], d["Identity"])
    if account is None:
        sys.exit("No GOA account with MailEnabled=true")
    out = subprocess.check_output([
        "gdbus", "call", "--session",
        "--dest", "org.gnome.OnlineAccounts",
        "--object-path", f"/org/gnome/OnlineAccounts/Accounts/{account[0]}",
        "--method", "org.gnome.OnlineAccounts.OAuth2Based.GetAccessToken",
    ], text=True)
    m = re.search(r"^\('([^']+)',", out)
    if not m:
        sys.exit("Failed to parse token")
    token = m.group(1)
    sys.stdout.write(f"user={account[1]}\x01auth=Bearer {token}\x01\x01")

if __name__ == "__main__":
    main()
PYEOF
chmod +x ~/.local/bin/goa-mail-token

# 2. The mail CLI (the reference script in `scripts/mail.py`)
install -m 0755 scripts/mail.py ~/.local/bin/mail
```

## Usage

```bash
mail whoami             # show account + token length (no email content)
mail inbox [N]          # N most recent inbox messages (default 20)
mail read <uid>         # full text body — text/plain preferred, HTML→text fallback
mail search <query> [N] # full-text search, newest N
mail send <to> <subject> <body>
mail folders            # list IMAP folders
```

## The two SASL-auth pitfalls (cost ~4 debugging turns)

**imaplib double-encodes.** `imap.authenticate("XOAUTH2", cb)` calls `base64.b64encode(cb(...))` **itself**. So your callback must return the **raw** XOAUTH2 string as **bytes**:

```python
imap.authenticate("XOAUTH2", lambda _: f"user={u}\x01auth=Bearer {t}\x01\x01".encode())
```

**smtplib does NOT match that contract.** `smtplib.SMTP.auth("XOAUTH2", cb)` calls `cb()` with **no args** for the initial response, then `cb(challenge).encode("ascii")` for subsequent challenges. Return value MUST be a `str` (it gets `.encode()`'d for you):

```python
payload = f"user={u}\x01auth=Bearer {t}\x01\x01"
s.auth("XOAUTH2", lambda _challenge=None: payload)
```

## Verifying the token exists

If `mail whoami` errors with "Empty token from GOA" or "No GOA account with MailEnabled=true", tell the user to re-auth in **Settings → Online Accounts**. GOA will refresh the OAuth2 token on next login; no app password or re-consent needed.

## Companion: `mail_check` (periodic Telegram alerts)

`mail_check` is a 5-min-cron companion that periodically scans Gmail, classifies
new mail into IMPORTANT vs DIGEST, and pushes a short message to Telegram.
Reuses `goa-mail-token` for the XOAUTH2 auth — same setup, no extra creds.

**Install:**

```bash
install -m 0755 scripts/mail_check ~/.local/bin/mail_check
~/.local/bin/mail_check --init   # mark current inbox as already-seen
```

**Schedule via Hermes cron** (wrapper script is required because cron needs
`~/.hermes/scripts/` paths):

```bash
# 1. Wrapper (cron needs a script under ~/.hermes/scripts/)
cp scripts/mail_check.sh ~/.hermes/scripts/mail_check.sh
chmod +x ~/.hermes/scripts/mail_check.sh

# 2. Schedule (creates a no-agent job — script is the entire job)
#    via Hermes CLI:
hermes cron create every 30m \
  --name mail-check \
  --script mail_check.sh \
  --no-agent
# Or via the cronjob tool in-session.
```

**Classification rules** (in `mail_check`):
- IMPORTANT: Gmail labels `Important`; `category_personal`; calendar invite
  (`text/calendar`/`.ics`); opt-in job alerts (LinkedIn, Naukri, etc.);
  keyword hits in subject/from (`interview`, `invoice`, `urgent`, `kyc`,
  `otp`, `loan`, `visa`, `selected`, `admit`, `scholarship`, `internship`,
  `recruiter`, `deadline`, `re:`, `fwd:`, etc.); real-person `Re:` to a
  non-bot sender.
- DIGEST: bot senders (noreply, notifications, alerts, substack, reddit,
  github, huggingface, skool, etc.); Gmail categories `promotions` /
  `updates` / `forums`; digest subject hints.
- **SELF**: any mail the user sent to himself (sender address ==
  `accounts.*.email`). Silently dropped — never alerted, never mentioned
  in the digest. The only way these surface is in `--verbose` logs
  (`skipped N self-sent: subject, …`). The check is a literal substring
  match on the configured email, so you don't accidentally alert on
  your own test messages.

**Format on Telegram** (super short, fits one notification):
- 1 important: `📬 Sender: Subject`
- N important: `📬 N new important:` + bullet list + `+M other: (domain: count)…`
- Only digest: quiet, only fires if ≥3 new items or contains a reply

**State:** `~/.local/state/mail_check/last_seen_uid.json` — UIDs already
announced. Resets via `--init`.

**Pitfalls when running under cron:**
- Cron's PATH is bare; the wrapper sets
  `PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:$PATH"`.
- Cron's D-Bus session is missing; `goa-mail-token` rebuilds
  `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$UID/bus` automatically.
- For Telegram delivery the script shells out to `hermes send -t telegram`.
  That works in cron because `hermes` reuses the gateway's bot token
  (no LLM, no agent loop).

## Limitations

- Plain-text send only (no HTML body, no attachments). For attachments, use a future himalaya build with `+oauth2`.
- Token lifetime is ~1h; the helper fetches a fresh token on every command, so this is invisible.
- Won't work from a headless/SSH session — needs an active GNOME session for D-Bus access.
