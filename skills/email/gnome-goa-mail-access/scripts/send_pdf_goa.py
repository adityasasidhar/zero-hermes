#!/usr/bin/env python3
"""Send a PDF (or any file) via Gmail SMTP using the GOA XOAUTH2 token.

This is the canonical workaround for two real failures:

1. `smtplib.SMTP.auth("XOAUTH2", cb)` returns `501 Cannot Decode response`
   on Gmail. The library's `auth()` helper double-encodes the initial
   response and Gmail rejects it. Verified 2026-07-28.
2. The `~/.local/bin/mail` CLI from `gmail-goa-cli` is plain-text-only;
   it has no attachment support. This script is the minimal replacement.

Prerequisites:
- `goa-mail-token` on PATH (provided by `gmail-goa-cli` skill).
- An active GNOME session (GOA D-Bus reachable).

Usage:
    python3 send_pdf_goa.py <from-address> <to-address> <pdf-path> <subject>

Verified working file: 3,869,692 bytes PDF, 2026-07-28, code 235 from
Gmail (2.7.0 Accepted). Message landed in inbox at uid 33421.
"""
import base64
import smtplib
import subprocess
import sys
from email.message import EmailMessage
from pathlib import Path


def send(from_addr: str, to_addr: str, pdf_path: Path, subject: str) -> None:
    """Send `pdf_path` as an attachment from `from_addr` to `to_addr`."""
    if not pdf_path.exists():
        sys.exit(f"FATAL: {pdf_path} not found")

    # goa-mail-token prints the SASL initial-response payload, already in
    # the XOAUTH2 wire format:
    #   user=<email>\x01auth=Bearer <token>\x01\x01
    # This is the *exact* byte sequence Gmail's AUTH XOAUTH2 expects.
    payload = subprocess.check_output(["goa-mail-token"], text=True).strip()

    # The `from_addr` MUST match the GOA account identity, or Gmail
    # returns 5.7.0 (relay access denied). For self-loops, set both
    # to the same address.
    user_in_payload = payload.split("user=", 1)[1].split("\x01", 1)[0]
    if from_addr != user_in_payload:
        sys.exit(
            f"FATAL: from-addr {from_addr!r} != GOA account {user_in_payload!r}. "
            "Gmail will reject with 5.7.0."
        )

    msg = EmailMessage()
    msg["Subject"] = subject
    msg["From"] = from_addr
    msg["To"] = to_addr
    msg.set_content("Attached.")

    msg.add_attachment(
        pdf_path.read_bytes(),
        maintype="application",
        subtype="pdf",
        filename=pdf_path.name,
    )

    # Bypass smtplib.auth() — its encode_base64 pipeline is incompatible
    # with XOAUTH2's contract. Issue AUTH directly.
    with smtplib.SMTP("smtp.gmail.com", 587) as s:
        s.starttls()
        auth_b64 = base64.b64encode(payload.encode("utf-8")).decode("ascii")
        s.putcmd("AUTH", f"XOAUTH2 {auth_b64}")
        code, resp = s.getreply()
        if code != 235:
            sys.exit(f"FATAL: AUTH failed: {code} {resp}")
        print(f"send: auth ok ({code})")

        refused = s.send_message(msg)
        if refused:
            sys.exit(f"FATAL: refused recipients: {refused}")

    print(f"send: delivered {pdf_path} ({pdf_path.stat().st_size} bytes) to {to_addr}")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(f"usage: {sys.argv[0]} <from> <to> <pdf-path> <subject>")
    send(sys.argv[1], sys.argv[2], Path(sys.argv[3]), sys.argv[4])
