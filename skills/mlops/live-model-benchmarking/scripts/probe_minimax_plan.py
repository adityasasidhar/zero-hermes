#!/usr/bin/env python3
"""Probe MiniMax token-plan usage/quota.

MiniMax docs mislabel fields (see references/minimax-quirks.md): the
`current_*_usage_count` fields actually mean REMAINING, and on the new
token plan they are 0. The real signal is `remains_time` + `*_remaining_percent`.
Usage estimate: used ~= remains_time / remaining_percent * (100 - remaining_percent)/100.
"""
import json
import os
import sys
import urllib.request

KEY_ENV = "MINIMAX_API_KEY"


def get_key(explicit=None):
    if explicit:
        return explicit
    k = os.environ.get(KEY_ENV)
    if k:
        return k
    candidate = "/home/arctic/Internship/DataObserve/do-code/.env"
    if os.path.exists(candidate):
        with open(candidate) as f:
            for line in f:
                if line.startswith(f"{KEY_ENV}="):
                    return line.strip().split("=", 1)[1]
    return None


def remains(key):
    url = "https://api.minimax.io/v1/token_plan/remains"
    req = urllib.request.Request(url, method="GET")
    req.add_header("Authorization", f"Bearer {key}")
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode())


def main():
    key = get_key()
    if not key:
        print("NO KEY", file=sys.stderr)
        sys.exit(1)
    obj = remains(key)
    for m in obj.get("model_remains", []):
        pct = m.get("current_weekly_remaining_percent")
        rt = m.get("weekly_remains_time")
        name = m.get("model_name")
        if pct and rt and pct > 0:
            used = rt / pct * (100 - pct) / 100
            print(f"[{name}] weekly remaining {pct}% | ~{used:,.0f} tokens used | "
                  f"{rt:,} remaining")


if __name__ == "__main__":
    main()
