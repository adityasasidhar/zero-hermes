# Telegram delivery via `hermes send`

Quick reference for the `hermes send` CLI flags the pipeline actually uses.

## Basic text-only message

```bash
hermes send --to telegram -q "🗞 THE HERMES TIMES — 2026-07-26"
```

`-q` is `--quiet` (the script suppresses stdout on success).
`--to telegram` routes to the home channel; use `telegram:-1001234567890:17585`
for a specific chat/thread.

## Sending an image or PDF

`MEDIA:<path>` is part of the **message text**, not a flag. The whole
message must be passed as a single quoted string so the shell doesn't
split it on spaces.

```bash
hermes send --to telegram -q 'Caption here.

MEDIA:/home/arctic/.hermes/data/hermes-times-v3/issues/2026-07-26.png'
```

The blank line before `MEDIA:` is optional. Telegram treats the file
after the `MEDIA:` marker as an attachment; the rest of the text is the
caption.

## Sending a PNG + PDF in two separate calls

Telegram does not allow multiple attachments in one bot message. Send
two `hermes send` calls back-to-back:

```bash
hermes send --to telegram -q "🗞 Cover PNG

MEDIA:/path/to/issue.png"
hermes send --to telegram -q "📄 Full PDF

MEDIA:/path/to/issue.pdf"
```

## Reading from a file

```bash
hermes send --to telegram -q --file /tmp/caption.txt
```

The message body is read from the file (text only). For attachments,
use `MEDIA:` in the body even when reading from a file.

## Common mistakes

- `hermes send -t telegram "MEDIA:/path"` — `-t` is short for `--to`,
  but the CLI rejects the message *if it has unrecognised arguments*;
  the right invocation is `--to telegram -q "..."`.
- Splitting `MEDIA:/path` from the rest of the message across two
  shell arguments — the path gets treated as a flag and the CLI errors
  with `unrecognized arguments: MEDIA:/path`.
- Forgetting `-q` causes the JSON delivery receipt to flood stdout,
  which is fine in chat but noisy in cron scripts.

## From a cron script

```bash
# cron owns the schedule; the script owns the message
hermes send --to telegram -q "🗞 THE HERMES TIMES — ${DATE}
A MiniMax-edited, image-rich four-page brief.

MEDIA:${PNG_PATH}"
hermes send --to telegram -q "Full edition · live source links

MEDIA:${PDF_PATH}"
```

Empty stdout on success is the contract the cron scheduler expects
when `no_agent=true` and `deliver=local` — the script does the
delivery, not the scheduler.

## When `hermes send` silently drops the message

Observed multiple times across 2026-07-27 and 2026-07-31: `hermes send`
returns exit 0, the CLI prints nothing (because of `-q`), and the cron
DB records `last_status: completed, error: None` — but
`~/.hermes/logs/gateway.log` shows **zero** outbound Telegram activity
for the send window (no `sendDocument`, `chat_action`,
`uploadDocument`, or `Sending ... MEDIA:` line). The user never
receives the paper. The Telegram bot is alive on the wire (the gateway
connected at the previous boot), but `hermes send` → telegram_platform
→ bot API path drops the upload silently. Root cause not yet
investigated.

**Detection pattern** (works for any paper > 1 MB):

```bash
PDF=path/to/issue.pdf
LOG=~/.hermes/logs/gateway.log
# snapshot line count BEFORE the send
BEFORE=$(wc -l < "$LOG")
hermes send --to telegram -q "caption

MEDIA:${PDF}"
# wait up to 5s for an outbound line containing our PDF filename or
# any of the standard send verbs
sleep 5
NEW=$(tail -n +$((BEFORE + 1)) "$LOG" | grep -E "$(basename "$PDF")|sendDocument|sendMedia|chat_action|Sending.*MEDIA|uploadDocument" || true)
if [ -z "$NEW" ]; then
  echo "WARNING: hermes send returned 0 but no gateway upload log" >&2
  exit 2   # loud failure
fi
```

The send.sh script at `~/.hermes/scripts/hermes-times-v4/send.sh`
implements this and exits 2 on silent drop.

**Recovery: don't retry, pivot to mail.** A retry of `hermes send` may
queue indefinitely without the gateway ever flushing. The reliable
fallback is Gmail SMTP + GOA XOAUTH2 — a single `python3` call:

```bash
python3 ~/.hermes/scripts/hermes-times-v4/send_pdf_mail.py \
    "<recipient>@gmail.com" \
    "THE HERMES TIMES · Issue N · 2026-07-31" \
    /tmp/hermes_times_body.txt \
    /home/arctic/.hermes/data/hermes-times-v4/issues/2026-07-31.pdf
```

A working template ships at `templates/send_pdf_mail.py`. Three
gotchas that bit the first attempt:

1. **Do NOT import `~/.local/bin/mail`** — it's `__main__`-guarded and
   `importlib.util.spec_from_file_location` returns `None` for it
   (raise: `'NoneType' object has no attribute 'loader'`). Inline the
   two helpers (`get_token_and_email`, `xoauth2_string`) into the
   script.
2. **Use `s.auth("XOAUTH2", lambda _c=None: payload)`**, NOT
   `s.docmd("AUTH", "XOAUTH2", payload)`. `SMTP.docmd` takes 2-3
   positional args; the 4-arg form raises `TypeError`.
3. **`smtplib.SMTP.timeout=120`** for PDFs > 4 MB. 30s/60s fails mid
   `send_message` → `sendmail` → `data(msg)` with
   `SMTPServerDisconnected("Server not connected")` after the raw
   PDF body has already been streaming.

**Honest reporting rule.** When the Telegram path drops, do NOT claim
"on Telegram" in the cron closing note. Say: "PDF delivered to
`<email>`; Telegram delivery failed silently (gateway log silent for
the send window); the paper rendered and was attempted, delivery
failed, the PDF is at `<path>` for manual recovery." This is the
non-negotiable editorial honesty clause.
