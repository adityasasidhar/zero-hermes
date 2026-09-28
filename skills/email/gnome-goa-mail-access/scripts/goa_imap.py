#!/usr/bin/env python3
"""Read Gmail via IMAP using the OAuth2 token from GNOME Online Accounts.

No app password, no re-login. Requires a running GNOME session (GOA D-Bus
daemon). The token is fetched fresh on every run via `gdbus`.

Usage:
    python3 goa_imap.py [FOLDER] [COUNT]
    python3 goa_imap.py INBOX 10

PITFALL: imaplib.authenticate() base64-encodes the callback's return value
itself, so the callback MUST return the RAW (un-encoded) XOAUTH2 string as
bytes. Returning an already-base64'd string gives
"AUTHENTICATE command error: BAD [b'Invalid SASL argument. ...']".
"""
import imaplib
import os
import re
import subprocess
import sys

GOA_ACCOUNTS_CONF = os.path.expanduser("~/.config/goa-1.0/accounts.conf")


def _parse_accounts_conf():
    """Read ~/.config/goa-1.0/accounts.conf and yield (account_path, email)."""
    try:
        with open(GOA_ACCOUNTS_CONF) as f:
            text = f.read()
    except FileNotFoundError:
        return
    # Section headers look like: [Account account_1782325375_0]
    sections = re.split(r"^\[Account\s+", text, flags=re.MULTILINE)
    for sec in sections[1:]:
        m_path = re.match(r"(account_\d+_\d+)\]", sec)
        m_email = re.search(r"^Identity=(.+)$", sec, re.MULTILINE)
        m_provider = re.search(r"^Provider=(.+)$", sec, re.MULTILINE)
        if not (m_path and m_email):
            continue
        path = "/org/gnome/OnlineAccounts/Accounts/" + m_path.group(1)
        email = m_email.group(1).strip()
        provider = m_provider.group(1).strip() if m_provider else ""
        yield path, email, provider


def discover_account(prefer_email=None):
    """Pick the best account — prefer matching email, else first google one."""
    accounts = list(_parse_accounts_conf())
    if not accounts:
        raise RuntimeError("No GOA accounts found in " + GOA_ACCOUNTS_CONF)
    for path, email, provider in accounts:
        if prefer_email and email.lower() == prefer_email.lower():
            return path, email
    for path, email, provider in accounts:
        if provider == "google":
            return path, email
    return accounts[0][0], accounts[0][1]


def get_token(acct_path):
    out = subprocess.check_output(
        [
            "gdbus", "call", "--session",
            "--dest", "org.gnome.OnlineAccounts",
            "--object-path", acct_path,
            "--method", "org.gnome.OnlineAccounts.OAuth2Based.GetAccessToken",
        ]
    ).decode()
    # format: ('ya29....', expires_in)
    m = re.search(r"'([^']+)'", out)
    if not m:
        raise RuntimeError(f"Could not parse token from: {out[:200]}")
    return m.group(1)


def xoauth2(user, token):
    # RAW string — imaplib will base64-encode it.
    return f"user={user}\x01auth=Bearer {token}\x01\x01"


def main():
    folder = sys.argv[1] if len(sys.argv) > 1 else "INBOX"
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    email_arg = os.environ.get("GOA_EMAIL")
    acct_path, email = discover_account(prefer_email=email_arg)
    print(f"Using account {email} at {acct_path}", file=sys.stderr)
    token = get_token(acct_path)
    imap = imaplib.IMAP4_SSL("imap.gmail.com", 993)
    imap.authenticate("XOAUTH2", lambda x: xoauth2(email, token).encode("utf-8"))
    status, data = imap.select(folder)
    print(f"SELECT {folder}: {status} ({data})", file=sys.stderr)
    status, data = imap.search(None, "ALL")
    ids = data[0].split()
    print(f"Total messages in {folder}: {len(ids)}", file=sys.stderr)
    for mid in ids[-count:]:
        status, msg = imap.fetch(mid, "(RFC822.HEADER)")
        raw = msg[0][1].decode(errors="replace")
        headers = {}
        for line in raw.split("\n"):
            if ":" in line and len(headers) < 6:
                k, v = line.split(":", 1)
                if k.strip() in ("From", "To", "Subject", "Date"):
                    headers[k.strip()] = v.strip()
        print(
            f"[{mid.decode()}] {headers.get('Date','')} | "
            f"{headers.get('From','(unknown)')} | {headers.get('Subject','(no subject)')}"
        )
    imap.logout()


if __name__ == "__main__":
    main()
