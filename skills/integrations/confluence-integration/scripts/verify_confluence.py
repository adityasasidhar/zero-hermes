#!/usr/bin/env python3
"""Verify Confluence Cloud credentials from a project .env — no secrets printed.

Reads CONFLUENCE_API_TOKEN (+ optional CONFLUENCE_USERNAME / CONFLUENCE_URL)
from the repo .env, attempts Basic auth, and reports whether everything works.
Also expands a /wiki/x/<short> page link to its real numeric page id.

Usage:
    python3 verify_confluence.py [SHORT_LINK_URL]
Exit code 0 = verified.
"""
import os
import re
import sys
import requests

ROOT = os.path.dirname(os.path.abspath(__file__))


def load_env(path):
    env = {}
    try:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                k, v = line.split("=", 1)
                env[k.strip()] = v.strip().strip('"').strip("'")
    except FileNotFoundError:
        pass
    return env


def main():
    env = load_env(os.path.join(ROOT, ".env"))

    token = env.get("CONFLUENCE_API_TOKEN")
    if not token:
        print("✗ CONFLUENCE_API_TOKEN not found in .env")
        sys.exit(1)

    email = env.get("CONFLUENCE_USERNAME") or env.get("CONFLUENCE_EMAIL")
    if not email:
        print("! CONFLUENCE_USERNAME not in .env — pass it or set the env var.")
        sys.exit(1)

    base = env.get("CONFLUENCE_URL") or "https://dataobserve.atlassian.net/wiki"
    base = base.rstrip("/")
    api = base + "/api/v2"

    print(f"Testing auth against: {base}")
    print(f"Account email       : {email}")
    print("-" * 50)

    try:
        r = requests.get(f"{api}/pages?limit=1", auth=(email, token), timeout=20)
    except requests.RequestException as e:
        print(f"✗ Network error: {e}")
        sys.exit(1)

    if r.status_code == 200:
        print(f"✓ AUTH OK (HTTP 200) — token + email valid for {base}")
    elif r.status_code == 401:
        print("✗ 401 Unauthorized — email or token wrong (or token revoked).")
        sys.exit(1)
    elif r.status_code == 403:
        print("✗ 403 Forbidden — creds valid but account lacks permission.")
        sys.exit(1)
    else:
        print(f"✗ Unexpected HTTP {r.status_code}: {r.text[:300]}")
        sys.exit(1)

    if len(sys.argv) > 1:
        short = sys.argv[1]
        print("-" * 50)
        print(f"Resolving short-link: {short}")
        try:
            resp = requests.get(short, auth=(email, token), timeout=20, allow_redirects=True)
            m = re.search(r"/pages/(\d+)", resp.url)
            if m:
                page_id = m.group(1)
                print(f"✓ Resolved to page id: {page_id}")
                pr = requests.get(f"{api}/pages/{page_id}", auth=(email, token), timeout=20)
                if pr.status_code == 200:
                    pj = pr.json()
                    print(f"  Title  : {pj.get('title')}")
                    print(f"  Space  : {pj.get('spaceId')}")
                    print(f"  Version: {pj.get('version', {}).get('number')}")
            else:
                print(f"  Could not extract page id from: {resp.url}")
        except requests.RequestException as e:
            print(f"  Resolution error: {e}")


if __name__ == "__main__":
    main()
