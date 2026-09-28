# Mail fallback when GOA is unavailable

The parent skill (`gnome-goa-mail-access`) assumes the user has GNOME
Online Accounts wired up. When that's not true — headless server, fresh
install, custom-domain mail on a non-GOA provider, or just a personal
taste for config files — fall back to a `himalaya` config backed by
an **App Password** (never a raw account password) stored in a
`chmod 600` file and surfaced via `auth.cmd`.

This document captures the gotchas hit in production wiring so the
next agent doesn't re-discover them. Each section is a fix; each
fix is something a future session will save you 5–30 minutes on.

## When to use this fallback

Trigger this reference when ANY of the following is true:

- User is not on GNOME (host with no `gdbus`).
- User has a custom-domain mailbox (`user@anything-not-gmail-or-yahoo`).
- User is on M365 / Outlook (`@outlook.com`, `@<org>.onmicrosoft.com`,
  or a custom domain on Microsoft 365).
- User has GOA but wants Hermes to read a *second* account that's not
  in GOA (e.g. work email on a separate provider).
- User's edu / corporate admin disabled per-user App Passwords on
  Google Workspace (then escalate to OAuth — see bottom).

If GOA *is* available, ignore this whole document and follow the
parent SKILL.md instead.

## gmail.com / Google Workspace — App Password flow

### Diagnose first: is it really a credentials problem?

Google's IMAP server returns the same error for "wrong password" and
"this is your real password, not an App Password":

```
AUTHENTICATIONFAILED — Invalid credentials
```

If the user hands you a 16-char string with three spaces in the middle
and login fails, **do not retry**. The string is almost certainly
their actual password; Google blocks raw passwords on `imap.gmail.com`
since 2022 unless the user's admin has re-enabled "less secure apps"
(which is rare and getting rarer). Walk them through App Passwords:

1. https://myaccount.google.com/security — confirm 2-Step Verification is ON.
2. https://myaccount.google.com/apppasswords — generate one named `hermes-<purpose>`.
3. Use that 16-char string in the credential file.

For edu / Workspace accounts, App Passwords may be locked by the org
admin. If step 2 errors with "this setting is not available for your
account", jump to the OAuth2 path at the bottom of this document.

### Credential storage (do not put secrets in TOML)

Never write the literal password into `~/.config/himalaya/config.toml`.
Use `backend.auth.cmd` pointing at a helper script that reads a
`chmod 600` file. Two reasons:

1. `git status` / accidental `cat config.toml` exposure.
2. Single source of truth — easier to rotate, easier to back up
   without leaking.

Required helper behavior: **strip trailing newlines** or himalaya
authenticates against `the-password\n` and fails with the misleading
"Invalid credentials" error (and you'll chase the wrong cause).

```bash
#!/usr/bin/env bash
# /home/arctic/keys/bin/get_imap_passwd.sh
set -euo pipefail
tr -d '\n' < /home/arctic/keys/gmail_<purpose>_app_password.txt
```

Permissions: 700 on the script, 600 on the credential file, owner
`$USER`. Both files must live outside any git-tracked directory.

### himalaya config pitfall: `default = true` is required

himalaya v1.2.0 errors with `cannot find default account
configuration` if no account under `[accounts.NAME]` has
`default = true`. The error fires on `himalaya folder list` and any
per-folder command even though `himalaya account list` shows the
account as registered. Set `default = true` on exactly one account.

### himalaya config pitfall: no top-level `--account`

The example `himalaya --account work envelope list` from older docs
**does not work on v1.2.0**. v1.2.0 rejects `--account` as a
top-level flag with `unexpected argument '--account' found`.
Per-command `--account` works on some subcommands (e.g.
`himalaya account --account NAME list`), but the cleanest
cross-version pattern is: set `default = true`, omit the flag.

When in doubt: `himalaya <subcommand> --help` lists what that
subcommand actually accepts.

### gmail folder aliases (v1.2.0 only)

Gmail's sent/drafts/trash live under `[Gmail]/...` paths. Use the
**v1.2.0 plural/dotted** form, *not* the older `[accounts.NAME.folder.alias]`
singular form — the older form is silently ignored on v1.2.0 and
saves-to-Sent fails *after* SMTP succeeds, causing duplicate sends
on retries.

```toml
folder.aliases.inbox   = "INBOX"
folder.aliases.sent    = "[Gmail]/Sent Mail"
folder.aliases.drafts  = "[Gmail]/Drafts"
folder.aliases.trash   = "[Gmail]/Trash"
folder.aliases.junk    = "[Gmail]/Spam"
folder.aliases.archive = "[Gmail]/All Mail"
```

### Read-only configuration

For Hermes cron jobs that *digest* mail, do not configure a
`message.send.backend`. himalaya then refuses to send, which is the
correct behavior — if a future cron prompt ever tries to send mail,
the failure should be loud and obvious, not a silent dropped email.

A working read-only vit Google Workspace config is in
`../templates/gmail-readonly-config.toml.example` — copy, edit the
email/login lines, point `auth.cmd` at your password helper.

## Microsoft 365 / Outlook (`*.onmicrosoft.com` or custom domain on M365)

Same himalaya config shape, different hosts:

- IMAP: `outlook.office365.com:993`, TLS
- SMTP: `smtp.office365.com:587`, STARTTLS

Auth options in order of preference:

1. **OAuth2 via Microsoft Graph** — most robust. himalaya supports
   `backend.auth.type = "oauth2"` with a refresh-token flow. Requires
   a one-time Azure app registration. Build this if you have M365
   accounts you want to use long-term.
2. **App password** — only if the tenant admin has Basic Auth enabled
   (modern tenants disable this by default). Try
   https://mysignins.microsoft.com/security-info first; if the
   "App passwords" page errors with "your organization doesn't allow
   this", you must use OAuth.
3. **Resource delegation** — for enterprise multi-tenant scenarios,
   out of scope for a personal himalaya setup.

## OAuth2 escalation (when App Passwords aren't allowed)

For both Google Workspace (org-locked) and M365, the durable answer
is OAuth2 with a refresh token. himalaya supports
`backend.auth.type = "oauth2"`. The setup is heavier but produces
a credential that auto-refreshes and works against org policy. See
`../templates/oauth2-imap-config.toml.example` for the config shape,
and the `google-workspace` / `outlook-microsoft-graph` umbrella
skills for the OAuth dance end-to-end.

## Verification steps after wiring any provider

```bash
# 1. Account is registered
himalaya account list

# 2. Folders enumerate (proves auth works)
himalaya folder list

# 3. Inbox fetches without error (proves folder aliases resolve)
himalaya envelope list --page-size 5

# 4. Optional — read one message to confirm body decode
himalaya message read 1
```

If step 2 errors with `cannot find default account configuration`,
re-check `default = true` (first pitfall). If step 3 fails with
`AUTHENTICATIONFAILED`, you're on the password-vs-app-password
triage path (Gmail) or need OAuth (M365 / locked-down Google).
