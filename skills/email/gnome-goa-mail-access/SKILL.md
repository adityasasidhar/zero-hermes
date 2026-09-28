---
name: gnome-goa-mail-access
description: "Read/send Gmail (and other GOA mail) via IMAP/SMTP reusing the OAuth2 token already stored in GNOME Online Accounts — no app password, no re-login. Covers: locating the GOA account over D-Bus, calling GetAccessToken, using XOAUTH2 with imaplib/smtplib, and the imaplib base64 double-encoding pitfall."
tags:
  - Email
  - IMAP
  - SMTP
  - OAuth2
  - GNOME
  - GOA
  - Gmail
  - D-Bus
version: 1.1.0
author: hermes
license: MIT
platforms: [linux]
metadata:
  hermes:
    tags: ["Email", "IMAP", "SMTP", "OAuth2", "GNOME", "GOA", "Gmail", "D-Bus"]
---

# GNOME Online Accounts — Reuse Stored OAuth2 Token for Mail (IMAP/SMTP)

When the user has already signed into a Google (or other) mail account through
**GNOME Online Accounts (GOA)**, you can read and send that mailbox from a
terminal/script **without** creating an app password and **without** any new
login. The live OAuth2 access token lives in the GOA daemon and is handed out
over the session D-Bus. This is the lowest-friction path for any Linux desktop
(GNOME) user whose mail is already configured in the OS.

Why this over the `himalaya` skill's password flow: GOA mail uses
`XOAUTH2` auth, not password/app-password. Tokens expire (~1h) and refresh
automatically via GOA, so call `GetAccessToken` fresh for every session.

Why this over the `google-workspace` skill for mail: `google-workspace` requires
a full Google Cloud OAuth client (Desktop app, scopes, test-user allowlist, JSON
download). This skill reuses the token GOA already stored in the user's GNOME
session. Zero Cloud Console interaction. Use `google-workspace` for
Drive/Sheets/Docs/Calendar (which GOA doesn't expose).

## Prerequisites (all checked this session, all present on a stock GNOME box)

- `gdbus` on PATH (always present on GNOME).
- A GOA account with mail enabled. Verify:
  `ls ~/.config/goa-1.0/accounts.conf` and check `MailEnabled=true`.
- The `org.gnome.OnlineAccounts` D-Bus service reachable (it is, whenever the
  user's GNOME session is active — this is the one hard dependency: it will NOT
  work from a headless/SSH session with no running GNOME).

## Account discovery (auto — never hardcode the path)

The account object path (`/org/gnome/OnlineAccounts/Accounts/account_NNNNN_M`)
is session-specific and changes per user. **Do not hardcode it.** The
reference script (`scripts/goa_imap.py`) auto-discovers accounts by parsing
`~/.config/goa-1.0/accounts.conf`:

1. Section headers look like `[Account account_1782325375_0]` — extract the
   path suffix and the `Identity=...@gmail.com` line.
2. If multiple accounts exist, prefer one matching the `GOA_EMAIL` env var,
   else the first `Provider=google` entry, else the first account.
3. Force a specific account with `GOA_EMAIL=you@gmail.com python goa_imap.py`.

Fallback discovery if the conf file is missing or stale: enumerate via the
D-Bus `GetManagedObjects` call. The `Mail` interface exposes
`ImapHost`/`SmtpHost`/`EmailAddress` properties.

⚠️ **PITFALL — placeholder constants in older script revisions.** A previous
revision of `goa_imap.py` shipped with `EMAIL = "CHANGE_ME@example.com"` and
`ACCT = "/org/gnome/OnlineAccounts/Accounts/account_0000000000_0"`. Running
that version as-is fails with `GDBus.Error:...UnknownMethod: Object does not
exist at path "...0000000000_0"`. If you see that error, upgrade to v1.1+ (or
patch the constants at the top of the script — or use the auto-discovery in
the current version).

## Steps

### 1. Discover the account object path
The script does this automatically (see "Account discovery" above). The path
looks like `/org/gnome/OnlineAccounts/Accounts/account_1782325375_0`.

Manual discovery via D-Bus:
```bash
gdbus call --session --dest org.gnome.OnlineAccounts \
  --object-path /org/gnome/OnlineAccounts \
  --method org.freedesktop.DBus.ObjectManager.GetManagedObjects \
  | grep -o '/org/gnome/OnlineAccounts/Accounts/[^"]*' | head -1
```
The same call dumps the `Mail` interface, which exposes
`ImapHost`/`SmtpHost`/`SmtpAuthXoauth2`, so you don't need to guess server
settings.

### 2. Get a live access token (fresh each run)
```bash
gdbus call --session --dest org.gnome.OnlineAccounts \
  --object-path "$ACCT" \
  --method org.gnome.OnlineAccounts.OAuth2Based.GetAccessToken
```
Returns `('ya29.a0...token...', expires_in_seconds)`. The token is a 200+ char
`ya29.` string. **Do not log the full token.** Parse it with:
```bash
TOKEN=$(echo "$OUT" | sed -E "s/^\('(.*)', .*/\1/")
```
(Note: `EnsureCredentials` is on the `Account` interface, NOT `OAuth2Based`;
`GetAccessToken` is on `OAuth2Based`. Calling `EnsureCredentials` on
`OAuth2Based` gives `UnknownMethod`.)

### 3. Build the XOAUTH2 string and authenticate
The SASL XOAUTH2 initial response is:
```
user=<EMAIL>\x01auth=Bearer <TOKEN>\x01\x01
```
Encoded as base64 when sent.

#### ⚠️ PITFALL — imaplib double-encodes (cost ~4 wasted debugging turns)
`imaplib.IMAP4.authenticate(mechanism, authobject)` calls
`base64.b64encode(authobject(response))` **itself**. So your callback must
return the **RAW** (un-base64'd) auth string as **bytes**, e.g.:
```python
def xoauth2(user, token):
    return f"user={user}\x01auth=Bearer {token}\x01\x01"
imap.authenticate("XOAUTH2", lambda x: xoauth2(EMAIL, token).encode("utf-8"))
```
Returning an already-base64'd string yields:
`AUTHENTICATE command error: BAD [b'Invalid SASL argument. ...']`.

#### ⚠️ PITFALL — `smtplib.auth('XOAUTH2', cb)` does NOT work for Gmail
Despite what older docs/examples claim, `smtplib.SMTP.auth('XOAUTH2', cb)` fails
with **`501 Cannot Decode response`** on Gmail (and likely any other modern SASL
XOAUTH2 server). The reason: the library's `auth()` wrapper encodes the initial
response via `encode_base64(cb_result)`, but for XOAUTH2 the client must send a
base64-encoded SASL payload — so the library double-encodes it and the server
rejects. The "Working pattern" with `s.auth("XOAUTH2", xoauth2_cb(EMAIL, TOKEN))`
claiming code 235 is incorrect; verified 2026-07-28 that it returns 501.

**Workaround (verified 2026-07-28, 3.7 MB PDF sent, code 235):** bypass `auth()`
and issue the AUTH command directly with `putcmd`:

```python
import base64, smtplib, subprocess

# goa-mail-token returns the SASL initial-response bytes already
# (user=<email>\x01auth=Bearer <token>\x01\x01). Just base64 and send.
payload = subprocess.check_output(["goa-mail-token"], text=True).strip()
with smtplib.SMTP("smtp.gmail.com", 587) as s:
    s.starttls()
    auth_b64 = base64.b64encode(payload.encode("utf-8")).decode("ascii")
    s.putcmd("AUTH", f"XOAUTH2 {auth_b64}")
    code, resp = s.getreply()
    assert code == 235, (code, resp)   # 235 = "2.7.0 Accepted"
    s.send_message(msg)
```

The `s.putcmd("AUTH", ...)` + `s.getreply()` path is what the SMTP RFC
specifies for XOAUTH2 and what Gmail accepts. Both `cb()` (called with no
args for initial response) and `encode_base64(cb(...))` (called on subsequent
challenges) are inside `smtplib.auth()` and are not what Gmail expects.

**Bonus: this path also works for attachments.** `EmailMessage` + `send_message`
attaches the file; the only constraint is that `From:` must match the GOA
account identity (Gmail returns 5.7.0 otherwise). The `~/.local/bin/mail`
CLI provided by `gmail-goa-cli` is plain-text-only and **cannot** do this — for
attachment-bearing sends, use the `putcmd` recipe above. See the
`gmail-goa-cli` skill's "Sending attachments" section for the full working
script.

### Parsing `GetManagedObjects` output (account path + email)
`gdbus call` emits dicts of `Variant` values inside `<...>`. For string
fields, the format is `<'the string contents'>` — note the **single quotes
inside** the angle brackets. A regex like `<'([^']+)@...>` works because the
inner quote terminates the capture, not the angle bracket. Working pattern:
```python
out = subprocess.check_output(["gdbus", "call", "--session",
    "--dest", "org.gnome.OnlineAccounts",
    "--object-path", "/org/gnome/OnlineAccounts",
    "--method", "org.freedesktop.DBus.ObjectManager.GetManagedObjects"], text=True)
paths = re.findall(r"/org/gnome/OnlineAccounts/Accounts/account_\d+_\d+", out)
for p in paths:
    # Per-account block: from this path until the next path
    start = out.find(p)
    rest = out[start:]
    others = [rest.find(x, 1) for x in paths if rest.find(x, 1) != -1]
    block = rest[:min(others) if others else len(rest)]
    m = re.search(r"'EmailAddress':\s*<'([^']+@gmail\.com)'>", block)
    if m:
        acct_path, email = p, m.group(1); break
```

### 4. Reference script
See `scripts/goa_imap.py` — a working, self-contained Gmail reader that:
- auto-discovers the account from `accounts.conf` (no hardcoded path needed),
- shells out to `gdbus` for a fresh token (no Python GOA typelib needed —
  `gi.require_version("Goa")` fails because the Goa typelib isn't installed,
  so DON'T rely on pygobject for this; use `gdbus` instead),
- authenticates via XOAUTH2,
- selects a folder and prints recent message headers.

## Verify the token exists before assuming mail works
If `GetAccessToken` errors or returns empty, the GOA account may need the
user to re-auth in Settings → Online Accounts. This skill can't fix that; tell
the user to re-sign-in there.

## Support files
- `references/non-goa-fallback.md` — complete setup guide for when GOA is
  unavailable: App Password flow for Gmail, M365 path, OAuth2 escalation, and
  every himalaya v1.2.0 pitfall (missing `default`, broken `--account`
  flag, password-vs-App-Password triage).
- `templates/gmail-readonly-config.toml.example` — known-good read-only
  Gmail himalaya config (no SMTP backend).
- `templates/passwd-cmd-helper.sh` — copy-and-edit credential helper that
  strips trailing newlines (the most common silent failure mode).
- `templates/oauth2-imap-config.toml.example` — M365 OAuth2 config for when
  Basic Auth / App Passwords are disabled by the tenant admin.
- `scripts/goa_imap.py` — main reference script (auto-discovers GOA account,
  fetches token via `gdbus`, IMAP authenticate via XOAUTH2).

## Cross-links
- Complements the `himalaya` skill (password/app-password flow). Prefer this
  GOA path when the user is on a GNOME desktop with mail already set up; fall
  back to `himalaya`'s app-password config when GOA is unavailable (e.g.
  headless server).
- **Lighter than the `google-workspace` skill for mail/calendar when GOA is set
  up.** `google-workspace` requires building a full Google Cloud OAuth client
  (Desktop app, scopes, test-user allowlist). This skill reuses the GOA token
  that's already in the user's GNOME session — no Cloud Console, no new
  credentials, no consent screen. Use it for mail whenever GOA is wired up.
  Use `google-workspace` for Drive/Sheets/Docs, which GOA doesn't expose.