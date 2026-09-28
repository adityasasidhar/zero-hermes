#!/usr/bin/env python3
"""mail — read & send Gmail via the GNOME Online Accounts OAuth2 token.

Subcommands:
  inbox [N]                     show N most recent inbox messages (default 20)
  read <id>                     print plain-text body of message id
  search <text> [N]             full-text search, show newest N (default 10)
  send <to> <subject> <body>    send a plain-text email
  folders                       list IMAP folders
  whoami                        show the resolved account + token expiry

Auth: reuses the GOA-stored XOAUTH2 token (no app password, no re-login).
Token is fetched fresh each invocation.
"""
from __future__ import annotations

import argparse
import email as emaillib
import email.policy
import imaplib
import os
import re
import smtplib
import subprocess
import sys
from email.message import EmailMessage
from email.utils import formataddr, getaddresses, parsedate_to_datetime
from html.parser import HTMLParser

IMAP_HOST = "imap.gmail.com"
IMAP_PORT = 993
SMTP_HOST = "smtp.gmail.com"
SMTP_PORT = 587


def get_token_and_email() -> tuple[str, str]:
    out = subprocess.check_output(
        [os.path.expanduser("~/.local/bin/goa-mail-token")],
    )
    # Format: user=<email>\x01auth=Bearer <token>\x01\x01
    decoded = out.decode("utf-8")
    parts = decoded.split("\x01")
    # parts: ['user=<email>', 'auth=Bearer <token>', '', '']
    if len(parts) < 2:
        print(f"Bad token payload from helper: {decoded[:80]!r}", file=sys.stderr)
        sys.exit(1)
    user = parts[0].removeprefix("user=")
    token = parts[1].removeprefix("auth=Bearer ")
    return user, token


def xoauth2_string(email: str, token: str) -> str:
    return f"user={email}\x01auth=Bearer {token}\x01\x01"


class GmailClient:
    def __init__(self):
        self.email, self.token = get_token_and_email()
        self.imap: imaplib.IMAP4_SSL | None = None

    def __enter__(self):
        self.imap = imaplib.IMAP4_SSL(IMAP_HOST, IMAP_PORT)
        # ⚠️ imaplib double-encodes — pass RAW bytes, let it b64
        self.imap.authenticate(
            "XOAUTH2",
            lambda _resp: xoauth2_string(self.email, self.token).encode("utf-8"),
        )
        self.imap.select("INBOX")
        return self

    def __exit__(self, *args):
        if self.imap is not None:
            try:
                self.imap.close()
            except Exception:
                pass
            try:
                self.imap.logout()
            except Exception:
                pass

    # ---- IMAP ops ----------------------------------------------------

    def list_folders(self) -> list[dict]:
        assert self.imap
        status, data = self.imap.list()
        out = []
        for line in data or []:
            if isinstance(line, bytes):
                line = line.decode("utf-8", errors="replace")
            # parse: (\\HasNoChildren) "/" "INBOX"
            m = line.rsplit(' "/" ', 1)
            flags = m[0].split(" ", 1)[0] if m else ""
            name = m[1].strip('"') if len(m) > 1 else line
            out.append({"name": name, "flags": flags})
        return out

    def search(self, query: str, limit: int = 10) -> list[dict]:
        """Full-text search. Splits query into multiple terms joined by AND."""
        assert self.imap
        terms = [t for t in query.split() if t]
        if not terms:
            return []
        criteria = " ".join(f'TEXT "{t}"' for t in terms)
        status, data = self.imap.uid("SEARCH", None, criteria)
        if status != "OK" or not data or not data[0]:
            return []
        ids = data[0].decode().split()
        # Get the latest N (right side of the list)
        ids = ids[-limit:][::-1]
        return self._fetch_envelopes(ids)

    def inbox(self, limit: int = 20) -> list[dict]:
        """Most recent N messages in INBOX, newest first."""
        assert self.imap
        status, data = self.imap.uid("SEARCH", None, "ALL")
        if status != "OK" or not data or not data[0]:
            return []
        ids = data[0].decode().split()
        ids = ids[-limit:][::-1]
        return self._fetch_envelopes(ids)

    def _fetch_envelopes(self, uids: list[str]) -> list[dict]:
        """Fetch RFC822.HEADER for each UID — much more reliable than parsing
        IMAP ENVELOPE parens manually."""
        assert self.imap
        out = []
        for uid in uids:
            status, data = self.imap.uid("FETCH", uid, "(RFC822.HEADER RFC822.SIZE)")
            if status != "OK" or not data:
                continue
            blob = b""
            size = 0
            for chunk in data:
                if isinstance(chunk, tuple):
                    header_blob = chunk[0]
                    body_blob = chunk[1] if len(chunk) > 1 else b""
                    if isinstance(header_blob, bytes):
                        # first line looks like: '... UID RFC822.SIZE ...'
                        if b"RFC822.SIZE" in header_blob and size == 0:
                            m = re.search(rb"RFC822\.SIZE (\d+)", header_blob)
                            if m:
                                size = int(m.group(1))
                    blob += body_blob
                elif isinstance(chunk, bytes):
                    blob += chunk
            msg = emaillib.message_from_bytes(blob, policy=email.policy.default)
            entry = {
                "uid": uid,
                "size": size,
                "from": _addr(msg.get("From", "")),
                "to": _addr(msg.get("To", "")),
                "subject": str(msg.get("Subject", "(no subject)")),
                "date": str(msg.get("Date", "")),
                "internaldate": _internaldate(msg.get("Date", "")),
            }
            out.append(entry)
        return out

    def read(self, uid: str) -> str:
        """Fetch and return text body. Prefers text/plain; falls back to
        naive HTML→text stripping for digest-style emails."""
        assert self.imap
        status, data = self.imap.uid("FETCH", uid, "(RFC822)")
        if status != "OK" or not data or not data[0]:
            return f"(no data for uid {uid})"
        raw = data[0][1] if isinstance(data[0], tuple) else data[0]
        msg = emaillib.message_from_bytes(raw, policy=email.policy.default)

        plain, html = None, None

        def walk(m):
            nonlocal plain, html
            if m.is_multipart():
                for child in m.iter_parts():
                    walk(child)
                return
            ct = m.get_content_type()
            payload = m.get_content()
            if isinstance(payload, bytes):
                try:
                    payload = payload.decode("utf-8", errors="replace")
                except Exception:
                    return
            if ct == "text/plain" and plain is None:
                plain = payload
            elif ct == "text/html" and html is None:
                html = payload

        walk(msg)

        if plain is not None:
            return plain
        if html is not None:
            return _html_to_text(html)
        return msg.as_string()

    # ---- SMTP ops ----------------------------------------------------

    def send(self, to: str, subject: str, body: str) -> str:
        msg = EmailMessage()
        msg["From"] = formataddr(("Aditya Sasidhar", self.email))
        msg["To"] = to
        msg["Subject"] = subject
        msg.set_content(body)
        # ⚠️ smtplib uses authobject() (no args) for initial response,
        # then encodes our return — return plain str.
        with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as s:
            s.starttls()
            payload = xoauth2_string(self.email, self.token)
            s.auth("XOAUTH2", lambda _challenge=None: payload)
            s.send_message(msg)
        return "sent"


def _parse_envelope(fetch_blob: str) -> dict:
    # Kept for any future debug; envelope parsing is now done in Python's
    # email module via RFC822.HEADER. Stub out instead of crashing.
    return {}


def _internaldate(date_str: str) -> str:
    """Best-effort compact timestamp for printing."""
    if not date_str:
        return ""
    try:
        dt = parsedate_to_datetime(date_str)
        return dt.strftime("%Y-%m-%d %H:%M")
    except Exception:
        return date_str[:25] if len(date_str) >= 25 else date_str


def _addr(header_value: str) -> str:
    """Pull the cleanest display form from an address header."""
    if not header_value:
        return ""
    adds = getaddresses([header_value])
    if not adds:
        return header_value
    real_name, addr = adds[0]
    if real_name:
        return f"{real_name} <{addr}>" if addr else real_name
    return addr or ""


class _HTMLStripper(HTMLParser):
    """Very small HTML→text converter.

    Not pretty; but for digest/notification emails without text/plain it's
    good enough to read in a terminal.
    """

    BLOCK = {"p", "div", "tr", "li", "h1", "h2", "h3", "h4", "h5", "h6", "br"}
    SKIP = ("style", "script", "head")

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self._buf: list[str] = []
        self._skip = 0
        self._in_skip = 0

    def handle_starttag(self, tag, attrs):
        if tag.lower() in self.SKIP:
            self._in_skip += 1
            return
        if tag.lower() in self.BLOCK:
            self._buf.append("\n")

    def handle_endtag(self, tag):
        if tag.lower() in self.SKIP:
            self._in_skip = max(0, self._in_skip - 1)
            return
        if tag.lower() in self.BLOCK:
            self._buf.append("\n")

    def handle_data(self, data):
        if self._in_skip:
            return
        self._buf.append(data)

    def get_text(self) -> str:
        text = "".join(self._buf)
        text = re.sub(r"[ \t]+", " ", text)
        text = re.sub(r"\n{3,}", "\n\n", text)
        return text.strip()


def _html_to_text(html: str) -> str:
    s = _HTMLStripper()
    s.feed(html)
    return s.get_text()


def _short(s: str, n: int = 60) -> str:
    s = s.replace("\n", " ").replace("\r", " ")
    if len(s) > n:
        return s[: n - 1] + "…"
    return s


def cmd_inbox(args):
    with GmailClient() as g:
        items = g.inbox(args.limit)
        if not items:
            print("(empty inbox)")
            return
        print(f"INBOX ({args.limit} most recent):\n")
        for it in items:
            d = it.get("internaldate", it.get("date", "?"))[:25]
            frm = _short(it.get("from", "?"), 40)
            sub = _short(it.get("subject", "(no subject)"), 60)
            print(f"  [{it['uid']:>5}]  {d}  {frm:<40}  {sub}")


def cmd_read(args):
    with GmailClient() as g:
        print(g.read(args.id))


def cmd_search(args):
    with GmailClient() as g:
        items = g.search(args.query, args.limit)
        if not items:
            print("(no matches)")
            return
        print(f"search: {args.query!r} ({len(items)} matches)\n")
        for it in items:
            d = it.get("internaldate", "")[:25]
            frm = _short(it.get("from", "?"), 40)
            sub = _short(it.get("subject", "(no subject)"), 60)
            print(f"  [{it['uid']:>5}]  {d}  {frm:<40}  {sub}")


def cmd_send(args):
    body = args.body
    with GmailClient() as g:
        result = g.send(args.to, args.subject, body)
        print(result, "->", args.to)


def cmd_folders(_args):
    with GmailClient() as g:
        for f in g.list_folders():
            print(f"  {f['name']}")


def cmd_whoami(_args):
    user, token = get_token_and_email()
    print(f"  email:  {user}")
    print(f"  token:  {token[:12]}…{token[-6:]}  (length {len(token)})")


def main():
    p = argparse.ArgumentParser(
        prog="mail", description="Read & send Gmail via GOA OAuth2 token"
    )
    sub = p.add_subparsers(dest="cmd", required=True)

    sp = sub.add_parser("inbox")
    sp.add_argument("limit", nargs="?", type=int, default=20)
    sp.set_defaults(func=cmd_inbox)

    sp = sub.add_parser("read")
    sp.add_argument("id")
    sp.set_defaults(func=cmd_read)

    sp = sub.add_parser("search")
    sp.add_argument("query")
    sp.add_argument("limit", nargs="?", type=int, default=10)
    sp.set_defaults(func=cmd_search)

    sp = sub.add_parser("send")
    sp.add_argument("to")
    sp.add_argument("subject")
    sp.add_argument("body")
    sp.set_defaults(func=cmd_send)

    sp = sub.add_parser("folders")
    sp.set_defaults(func=cmd_folders)

    sp = sub.add_parser("whoami")
    sp.set_defaults(func=cmd_whoami)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
