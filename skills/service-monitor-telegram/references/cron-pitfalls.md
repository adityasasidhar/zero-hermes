# Cron environment pitfalls for service monitors

Three things bit during the first Gmail monitor deployment. All three will
bite again for any new monitor that runs under Hermes cron. Capture once,
reuse everywhere.

## 1. PATH is bare

Hermes cron runs each tick in a fresh process with the default systemd
PATH (`/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`).
The `~/.local/bin` directory isn't on it. If your monitor shells out to
`hermes`, `uv`, `gh`, or any other tool in `~/.local/bin`, the wrapper
script must prepend it:

```bash
#!/bin/bash
# ~/.hermes/scripts/mail_check.sh — example wrapper
set -euo pipefail
export PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
exec /home/arctic/.local/bin/mail_check --verbose
```

**Pitfall:** forget this, and the script fails with `hermes: command
not found` after the first tick. Easy to miss in dev because your
interactive shell already has `~/.local/bin` on PATH.

## 2. DBUS_SESSION_BUS_ADDRESS is unset (GNOME OAuth monitors only)

If your monitor authenticates via GNOME Online Accounts (`gdbus` →
`org.gnome.OnlineAccounts.OAuth2Based.GetAccessToken`), the cron session
has no GUI session bus. `gdbus` errors:

```
Error connecting: Cannot autolaunch D-Bus without X11 $DISPLAY
```

The user's session D-Bus socket at `/run/user/<uid>/bus` is still
alive as long as the GNOME session is up. The token helper must rebuild
the bus address before invoking `gdbus`:

```python
import os, subprocess

def _gdbus_env() -> dict:
    env = os.environ.copy()
    if "DBUS_SESSION_BUS_ADDRESS" not in env:
        env["DBUS_SESSION_BUS_ADDRESS"] = f"unix:path=/run/user/{os.getuid()}/bus"
    return env

out = subprocess.check_output(
    ["gdbus", "call", "--session", ...],
    env=_gdbus_env(),
)
```

**What if the bus is genuinely missing?** (User logged out, server
rebooted, no display manager running.) The helper will return an empty
token. Add a startup probe in the monitor that detects this and stays
silent (don't spam errors every 30m).

**Security note:** the bus socket enforces the same UID. Cron jobs run
as the user, so the GOA daemon happily hands out the token. Cross-UID
access is not a concern.

## 3. Cron scripts must live under `~/.hermes/scripts/`

The `cronjob` tool (and `hermes cron create --script`) **rejects
absolute paths and `..` traversal** for security. The script must be a
basename under `~/.hermes/scripts/`. The typical pattern:

```bash
install -m 0755 scripts/<service>_monitor.sh ~/.hermes/scripts/
```

Then:

```bash
hermes cron create every 30m \
  --name <service>-monitor \
  --script <service>_monitor.sh \
  --no-agent
```

`--no-agent` is critical: the script IS the job. With `--no-agent` set,
the scheduler runs the script directly and delivers its stdout via the
`--deliver` target. Without it, the scheduler would also spin up an LLM
agent loop on each tick — burning tokens for no reason and adding 5-10s
of latency.

**Tried and rejected paths:**
- `script: mail_check.sh` (basename only) → "Script path escapes the
  scripts directory via traversal" — actually it was trying to enforce
  no-traversal and a bare filename confused it. The real fix is to
  place the file at `~/.hermes/scripts/mail_check.sh` first.

## Performance: batch IMAP FETCH by UID range

The naive per-UID round trip (`for uid in ids: imap.uid('FETCH', uid, ...)`)
makes N TLS round trips. For a 40-message inbox scan that's ~20s on a
decent connection. Use a single range FETCH:

```python
status, data = imap.uid(
    "FETCH", f"{first}:{last}", "(UID RFC822.HEADER RFC822.SIZE X-GM-LABELS)"
)
# `data` is a list of tuples, one per message, in range order
# Group by UID extracted from the first element of each tuple
```

This drops the same scan to ~4s.

`X-GM-LABELS` is the Gmail extension that returns the user's labels per
message. Without it, you can still classify by sender/subject keywords,
but you lose the `Important` and `category_*` signals — those
dramatically improve the IMPORTANT/DIGEST classifier.

## Telegram delivery from cron

The monitor invokes `hermes send -q -t telegram "<msg>"` to push a
message. This is fast (no LLM, no agent loop — just uses the configured
bot token) and works because `hermes send` reads the gateway's `.env`
independently of any running gateway process.

If Telegram delivery fails (e.g. bot token expired), the message goes
to a local fallback file. The cron job's `deliver: local` setting also
captures the script's stdout in the job's run log, so you can inspect
it with `hermes cron list` or `hermes cron status`.

## Other gotchas worth knowing

- **Token caching:** the GOA token is ~1h, so the helper fetching it
  on every tick is fine. Don't try to cache it across ticks — adds
  complexity and gains nothing.
- **First-run flood:** if you ship a monitor without `--init` baseline
  support, the first tick will alert on every existing item. Always
  add a baseline mode that marks current state as already-seen.
- **Gmail `X-GM-THRID`** (thread ID) can be a better "seen" key than
  UID for digests that get resent. UID is fine for inbox; consider
  THRID for `Updates` folder.
- **smtplib vs imaplib XOAUTH2 contract difference:** see
  `gmail-goa-cli/SKILL.md` if you're adding send capability.
