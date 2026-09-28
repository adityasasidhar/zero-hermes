---
name: service-monitor-telegram
description: "Periodically poll an external service (Gmail, RSS, GitHub, calendar, etc.), classify new activity, and push concise alerts to Telegram via Hermes cron. Use when the user says 'ping me when X', 'check Y every N minutes', 'tell me if anything important happens in Z', or wants a low-noise digest of an external feed without leaving their phone on a tab."
version: 1.1.0
author: hermes
license: MIT
metadata:
  hermes:
    tags: [monitoring, cron, telegram, alerts, imap, rss, classification, no-agent]
    related_skills: [gmail-goa-cli, gnome-goa-mail-access, himalaya, hermes-agent]
---

# Service Monitor → Telegram (via Hermes cron)

Pattern: a Python (or shell) script periodically polls an external service,
classifies new activity into IMPORTANT vs DIGEST, and pushes short messages
to Telegram. No LLM, no agent loop — the script is the entire cron job.

This skill captures the **pattern**; for the canonical Gmail
implementation see `gmail-goa-cli` (which has a working `mail_check` script).
Use this skill as the design template when wiring up monitors for other
services (RSS feeds, GitHub issues, calendar changes, price alerts, etc.).

**Companion files** in this skill (load with `skill_view(name="service-monitor-telegram", file_path=...)`):
- `references/cron-pitfalls.md` — the three cron-env gotchas + IMAP batching recipe.
- `templates/cron-wrapper.sh` — drop-in bash wrapper for `~/.hermes/scripts/`.
- `templates/first-run-checklist.md` — 9-step recipe for the first time you wire up a new monitor.

## When to use

- User wants Telegram/Slack/Discord alerts when something new happens in an
  external system
- User wants a periodic digest, not real-time webhooks
- The signal-to-noise ratio is low (most of the source is noise) and a
  classifier is needed
- No LLM should be on the hot path — Telegram delivery must be fast and
  cheap, not a 30-second agent turn

## User's standing preferences (apply unless told otherwise)

- **Telegram only, never chat.** When a monitor runs under cron, the user
  does **not** want its output mirrored into the active chat session. The
  script's stdout is `deliver: local` only. If the agent is asked to set
  up or debug a monitor, the agent should work in the chat and not push
  anything to the user through any other channel. "Updates on X" in the
  user's request means "alerts me on X when something happens" — it
  never means "echoes back to me in this chat".
- **Short and informative Telegram format.** When asked to show what
  alerts will look like, the user expects actual sample messages, not a
  format spec. Show 2-3 examples in the chat (one important, multiple,
  digest-only) using real classifications against real data if available.
- **Default cadence: 30 min.** Schedule `"every 30m"` unless the user
  specifies otherwise. Verify with `hermes cron list` (TUI output,
  not JSON — `hermes cron list | jq` will fail because the CLI emits a
  table) or `hermes cron status`.

## Boundary: alerts vs designed editions

Use this skill for low-noise event alerts and simple text digests. If the user asks for a **designed recurring newspaper, visual morning briefing, PDF report, or image-rich editorial edition**, load `automated-editorial-briefings` instead (and keep this skill only for its cron/Telegram delivery mechanics).

A designed edition may legitimately call an LLM and image model inside a deterministic `no_agent` script. In that architecture the script owns synthesis, rendering, and `hermes send`; cron uses `deliver=local` to avoid duplicate delivery. Do not force the alert classifier/message format onto a newspaper.

## Architecture

```
┌─────────────────┐   every 30m    ┌──────────────┐   IMAP/API    ┌──────────────┐
│ Hermes cron     │ ─────────────► │ monitor.py   │ ────────────► │ External     │
│ (no_agent=true) │                │              │               │ service      │
└─────────────────┘                └──────┬───────┘               └──────────────┘
                                         │ IMPORTANT/DIGEST
                                         ▼
                                  ┌──────────────┐
                                  │ hermes send  │ ──► Telegram
                                  │ -t telegram  │
                                  └──────────────┘
```

Key design choices:
- **Script is the entire job** (`no_agent=true`). The scheduler does not
  spin up an LLM. Cost per tick = one script run + maybe a few cents for
  outbound API calls.
- **State file at `~/.local/state/<service>_monitor/last_seen.json`**.
  Tracks IDs already announced. Resettable via `--init`.
- **Classifier decides** whether to alert or stay quiet. This is the
  single most important piece — the alert must be high-precision or the
  user mutes it.
- **Delivery via `hermes send -q -t telegram`**. No agent loop, no
  gateway dependency; reuses the configured bot token from
  `~/.hermes/.env`.

## Classification rules (the canonical IMPORTANT/DIGEST scheme)

The Gmail monitor uses this rule stack. Adapt keywords/senders per
service; the *structure* is the lesson:

1. **Promote to IMPORTANT** (any one wins):
   - Source's own importance signal (Gmail: `Important` label; Reddit:
     pinned post; GitHub: `mention` notification)
   - Calendar invite / meeting confirmation
   - Opt-in subscription that the user explicitly signed up for (job
     alerts, security alerts for their accounts) — these are IMPORTANT
     even though they look like bot mail
   - Keyword hit in title/body: career (interview, offer, admit,
     scholarship), finance (invoice, OTP, KYC, transaction), urgency
     (deadline, expires, action required), personal (visa, passport,
     booking)
   - Direct reply from a real person (subject `Re:` + sender isn't a
     bot)

2. **Demote to DIGEST** (any one wins, after the above):
   - Bot/system sender (noreply, notifications, alerts, digest,
     marketing)
   - Source's "low value" category (Gmail: `category_promotions`,
     `category_forums`; Reddit: `r/.../subscribe` digest; etc.)
   - Subject matches a known digest pattern (weekly digest, your
     daily, top stories, new follower)

3. **Default**: low-signal mail → DIGEST. Better to under-alert than
   over-alert.

Order matters: do keyword/sender checks BEFORE category demote for
opt-in subscriptions, so a LinkedIn job alert doesn't get caught by
`noreply`-substring demote.

### SELF filter (mandatory for self-creatable signals)

If the user can produce the signal themselves — sending a test email,
creating a calendar event they're invited to, opening a GitHub issue
under their own account, posting to their own social — add a **SELF**
tier that silently drops the item. The check is a literal substring
match on the configured "self" identifier (email address, user ID,
handle) against the source's author/sender field.

Without this, a single test message will trigger a real Telegram alert
that the user can't tell apart from genuine important activity, and
they'll lose trust in the monitor. With it, the only side effect is a
one-line note in the monitor's `--verbose` log: `skipped N self-sent:
subject, …`.

The SELF check is the *only* check that should be evaluated BEFORE the
importance keyword check — otherwise a self-sent "URGENT: Interview
with Anthropic" test message will fire as important before the SELF
filter gets a chance to drop it.

## Telegram format — sample (what the user actually sees)

When the user asks "what will the alerts look like?", give them real
sample messages like the ones below. Do NOT describe the format
abstractly. The user has explicitly asked for "short and informative";
this is the reference style for all monitors of this kind:

- 1 important: `📬 Sender: Subject`
- N important:
  ```
  📬 2 new important:
    • Acme Corp <careers@…: Interview slot — Senior ML Engineer
    • Sibling <adi.bro@gma…: Sunday lunch?
    +1 other: (gmail.com: 1)
  ```
- 6+ important:
  ```
  📬 7 new important:
    • Acme Corp <careers@…: Interview slot — Senior ML Engineer
    • Bank of X <alerts@ban…: Unusual login attempt detected
    • Aishwarya R <aish@…: Fwd: paper draft for review
    …and 4 more
  ```
- Digest-only: silent. Only fires if ≥3 new items or contains a real
  reply. Format: `(+5 other: (redditmail.com: 2) (substack.com: 2) (github.com: 1))`

Avoid: markdown tables (render poorly on phone), long bodies
(truncated to ellipsis), or formatting that breaks Telegram's parser
(raw newlines, unescaped underscores in names).

## Cron environment gotchas (the three things that bit during first deploy)

The full set with code samples is in
`references/cron-pitfalls.md` (this skill's own reference). Summary:

1. **PATH is bare** — wrapper sets
   `PATH="/home/arctic/.local/bin:/usr/local/bin:/usr/bin:/bin:$PATH"`.
2. **DBUS_SESSION_BUS_ADDRESS is unset** — rebuild from
   `/run/user/<uid>/bus` before any `gdbus` call. (Only matters for
   GNOME-OAuth-based monitors; not for RSS / GitHub / etc.)
3. **Cron scripts must live in `~/.hermes/scripts/`** — `cronjob`
   rejects absolute paths. Symlink, not copy, is the easiest deploy.

## Performance: batch IMAP FETCH by UID range

If the monitor uses IMAP, don't loop per-UID. A single range FETCH
(`imap.uid("FETCH", "first:last", "(UID RFC822.HEADER X-GM-LABELS)")`)
drops a 40-message scan from ~20s to ~4s. The response is a list of
tuples; group by UID, rebuild messages. Full example in
`references/cron-pitfalls.md`.

## Setting up a new monitor

Recipe (Gmail monitor as the worked example — same shape works for any
service with a CLI). **See `templates/first-run-checklist.md` for the
full 9-step procedure**; the short version is:

```bash
# 1. Drop the monitor script in ~/.local/bin
install -m 0755 scripts/<service>_monitor ~/.local/bin/<service>_monitor

# 2. First-run baseline (mark current state as already-seen)
<service>_monitor --init

# 3. Wrapper under ~/.hermes/scripts/
cp templates/cron-wrapper.sh ~/.hermes/scripts/<service>.sh
# edit the two paths inside, then:
chmod +x ~/.hermes/scripts/<service>.sh

# 4. Schedule
hermes cron create every 30m \
  --name <service>-monitor \
  --script <service>.sh \
  --no-agent
```

Verify with `hermes cron list` (TUI table — `last_status: ok` after the
first tick). Force a tick with `hermes cron run <job-id>`.

## Limitations

- Telegram delivery requires a configured gateway (`hermes send -t
  telegram` reads the bot token from `~/.hermes/.env`). If the gateway
  isn't set up, the monitor queues its message to a local file as
  fallback.
- Polling cadence is bounded by cron overhead (~1s per tick). For
  sub-minute latency, use webhooks or push-based APIs instead.
- The classifier is per-service. Adding a new service is a copy-the-
  classifier-and-tune-the-keywords job, not a generic config.
