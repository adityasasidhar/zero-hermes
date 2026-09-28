#!/usr/bin/env python3
"""send_pdf_mail.py — Gmail SMTP + GOA XOAUTH2 fallback for PDF delivery.

When `hermes send` silently drops a Telegram message (gateway log shows
zero outbound activity but the CLI returns 0), this script delivers the
PDF as a Gmail attachment instead. Reuses the GNOME Online Accounts
OAuth2 token, no app password, no re-login.

Usage:
    send_pdf_mail.py <to> <subject> <body_file_or_-> <pdf_path>

Body is read from a file path; pass '-' to read body from stdin.

Prereqs:
    - `~/.local/bin/goa-mail-token` exists and returns a valid Bearer token.
    - The `From:` address (from the token) must match the GOA account
      identity, or Gmail returns 5.7.0.
    - For PDFs > 4 MB, the SMTP timeout MUST be >= 120s; 30s/60s
      causes `SMTPServerDisconnected` mid-DATA.

Gotchas baked in:
    - Do NOT try to `import` `~/.local/bin/mail` — it's `__main__`-guarded
      and `importlib.util.spec_from_file_location` returns None for it.
      Inline the two helper functions instead.
    - Use `s.auth("XOAUTH2", lambda _c=None: payload)`, NOT
      `s.docmd("AUTH", "XOAUTH2", payload)` — `SMTP.docmd` takes 2-3
      args and the 4-arg form raises TypeError.
    - `smtplib.SMTP` (587 + STARTTLS), not `SMTP_SSL`. The XOAUTH2 string
      is `f"user={email}\\x01auth=Bearer {token}\\x01\\x01"` (matches the
      `~/.local/bin/mail` CLI's helper exactly).
"""
from __future__ import annotations

import os
import smtplib
import subprocess
import sys
from email.message import EmailMessage
from pathlib import Path


def get_token_and_email() -> tuple[str, str]:
    out = subprocess.check_output(
        [os.path.expanduser("~/.local/bin/goa-mail-token")],
    )
    decoded = out.decode("utf-8")
    parts = decoded.split("\x01")
    if len(parts) < 2:
        print(f"Bad token payload from helper: {decoded[:80]!r}", file=sys.stderr)
        sys.exit(1)
    user = parts[0].removeprefix("user=")
    token = parts[1].removeprefix("auth=Bearer ")
    return user, token


def xoauth2_string(email: str, token: str) -> str:
    return f"user={email}\x01auth=Bearer {token}\x01\x01"


def main() -> int:
    if len(sys.argv) != 5:
        print(__doc__, file=sys.stderr)
        return 1

    to, subject, body_arg, pdf_path = sys.argv[1:5]
    pdf = Path(pdf_path)
    if not pdf.exists():
        print(f"send_pdf_mail: missing PDF: {pdf}", file=sys.stderr)
        return 1

    if body_arg == "-":
        body = sys.stdin.read()
    else:
        body_path = Path(body_arg)
        if not body_path.exists():
            print(f"send_pdf_mail: missing body file: {body_path}", file=sys.stderr)
            return 1
        body = body_path.read_text()

    user, token = get_token_and_email()

    msg = EmailMessage()
    msg["From"] = user
    msg["To"] = to
    msg["Subject"] = subject
    msg.set_content(body)

    pdf_bytes = pdf.read_bytes()
    msg.add_attachment(
        pdf_bytes,
        maintype="application",
        subtype="pdf",
        filename=pdf.name,
    )

    # 120s timeout: 30s/60s cause SMTPServerDisconnected on >4MB PDFs
    # during the DATA phase.
    with smtplib.SMTP("smtp.gmail.com", 587, timeout=120) as s:
        s.starttls()
        payload = xoauth2_string(user, token)
        # `auth` takes (mechanism, authobject). The lambda returns the
        # XOAUTH2 string; smtplib base64-encodes it for the initial
        # response and any subsequent challenge.
        s.auth("XOAUTH2", lambda _challenge=None: payload)
        s.send_message(msg)

    print(f"send_pdf_mail: delivered to {to} \u00b7 {pdf.name} ({len(pdf_bytes)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())