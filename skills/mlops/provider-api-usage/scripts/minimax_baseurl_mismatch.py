#!/usr/bin/env python3
"""Diagnose the MiniMax `minimax` provider 404 caused by a base-URL override mismatch.

Hermes's key-based `minimax` provider speaks the Anthropic Messages protocol
(api_mode="anthropic_messages", default base https://api.minimax.io/anthropic).
If `.hermes/.env` sets MINIMAX_BASE_URL=https://api.minimax.io/v1 (OpenAI base),
that env var overrides the provider base (runtime_provider.py:1608
`base_url = env_url or pconfig.inference_base_url`), so Hermes POSTs
Anthropic-shaped requests to the OpenAI endpoint -> `404 page not found`.

This script:
  1. reads the active MINIMAX_BASE_URL from `.hermes/.env`,
  2. derives the effective request path,
  3. reproduces both (A) the broken path and (B) the correct path live,
  4. reports whether the key works at all and whether the base is misconfigured.

It never prints the raw key. Pass --env-file to probe a different .env
(e.g. the do-code project .env, where /v1 is CORRECT for their OpenAI usage).
"""
import argparse
import json
import os
import sys
import urllib.request

DEFAULT_HERMES_ENV = os.path.expanduser("~/.hermes/.env")
HERMES_MINIMAX_PROFILE_BASE = "https://api.minimax.io/anthropic"  # provider default


def _mask(v: str) -> str:
    return f"{v[:7]}…(len={len(v)})" if v else "<unset>"


def load_env(env_file: str) -> dict[str, str]:
    out: dict[str, str] = {}
    if not os.path.exists(env_file):
        return out
    for line in open(env_file, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        out[k.strip()] = v.strip()
    return out


def anthropic_post(base_url: str, key: str, max_tokens: int = 8):
    """Reproduce exactly what Hermes's minimax provider sends (Anthropic Messages shape)."""
    url = base_url.rstrip("/") + "/v1/messages"
    body = json.dumps({
        "model": "MiniMax-M3",
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": "ping"}],
    }).encode()
    req = urllib.request.Request(
        url, data=body,
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
            "anthropic-version": "2023-06-01",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, r.read().decode()[:200]
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:200]
    except Exception as e:  # noqa: BLE001
        return -1, f"{type(e).__name__}: {e}"


def main() -> int:
    ap = argparse.ArgumentParser(description="MiniMax base-URL mismatch diagnostic")
    ap.add_argument("--env-file", default=DEFAULT_HERMES_ENV,
                    help=".env to read MINIMAX_API_KEY / MINIMAX_BASE_URL from")
    args = ap.parse_args()

    env = load_env(args.env_file)
    key = env.get("MINIMAX_API_KEY", "")
    override = env.get("MINIMAX_BASE_URL", "")

    print(f"env file : {args.env_file}")
    print(f"key      : {_mask(key)}")
    print(f"override : {override or '<unset — provider default applies>'}")
    if not key:
        print("ERROR: no MINIMAX_API_KEY found. Cannot probe.", file=sys.stderr)
        return 2

    # Effective base Hermes will use: env override wins over provider default.
    effective = override or HERMES_MINIMAX_PROFILE_BASE
    print(f"effective: {effective}  (Hermes key-based minimax wants {HERMES_MINIMAX_PROFILE_BASE})")
    print()

    # (A) what the effective base does with an Anthropic-shaped request
    print(f"[A] Anthropic-shaped POST -> {effective}/v1/messages")
    a_status, a_body = anthropic_post(effective, key)
    print(f"    HTTP {a_status}  {a_body}")
    print()

    # (B) the correct Anthropic endpoint, always
    print(f"[B] Anthropic-shaped POST -> {HERMES_MINIMAX_PROFILE_BASE}/v1/messages")
    b_status, b_body = anthropic_post(HERMES_MINIMAX_PROFILE_BASE, key)
    print(f"    HTTP {b_status}  {b_body}")
    print()

    # Verdict
    misconfigured = override and override.rstrip("/") != HERMES_MINIMAX_PROFILE_BASE.rstrip("/")
    if a_status == 200 and b_status == 200:
        print("VERDICT: key works on both endpoints. No mismatch here.")
        return 0
    if a_status != 200 and b_status == 200 and misconfigured:
        print("DIAGNOSIS: base-URL override mismatch.")
        print(f"  Fix: in {args.env_file} set MINIMAX_BASE_URL={HERMES_MINIMAX_PROFILE_BASE}")
        print("  then RESTART Hermes (.env is read at startup).")
        return 1
    if a_status != 200 and b_status != 200:
        print("DIAGNOSIS: key appears dead on BOTH endpoints — check expiry / account status.")
        return 1
    print("DIAGNOSIS: inconclusive; review the two HTTP codes above.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
