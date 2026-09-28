#!/usr/bin/env python3
"""Verify MiniMax prompt-caching really engages on YOUR account/plan.

MiniMax-M3 supports PASSIVE (automatic) prompt caching: repeating an identical long
prefix across calls bills the repeated tokens at a discount and shows up as
`usage.prompt_tokens_details.cached_tokens`. Some June-2026 reports claim M3 caching is
"broken" on the direct API (tied to the `thinking` toggle). Run this to confirm on the
actual key before trusting or distrusting those reports.

KEY SOURCING (never print the secret)
  Priority: env MINIMAX_API_KEY  ->  else parse MINIMAX_API_KEY from --env-file.

USAGE
  export MINIMAX_API_KEY=sk-...
  python3 minimax_cache_probe.py --repeats 3 --prefix-mult 800

  # or point at a .env that holds MINIMAX_API_KEY (no key leaves the machine):
  python3 minimax_cache_probe.py --env-file /path/to/.env --repeats 3

Prints per-call: prompt_tokens, cached_tokens, and cache hit %.
Expect: call 1 caches only a small warm-up (~128), calls 2..N cache ~100% of a LONG prefix.
Short prefixes (<~200 tokens) cache 0 -- MiniMax has a minimum-prefix threshold.
"""
import json, urllib.request, argparse, os, sys

URL = "https://api.minimax.io/v1/chat/completions"
BASE_SYS = "You are a helpful assistant that explains machine learning concepts clearly and concisely. "

def load_key(args):
    if os.environ.get("MINIMAX_API_KEY"):
        return os.environ["MINIMAX_API_KEY"]
    if args.env_file:
        for line in open(args.env_file):
            line = line.strip()
            if line.startswith("MINIMAX_API_KEY="):
                return line.split("=", 1)[1].strip().strip('"').strip("'")
    print("NO MINIMAX_API_KEY found (set env or pass --env-file)", file=sys.stderr)
    sys.exit(1)

def call(key, model, sys_text, user_text):
    body = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": sys_text},
            {"role": "user", "content": user_text},
        ],
    }).encode()
    req = urllib.request.Request(URL, data=body, method="POST")
    req.add_header("Authorization", f"Bearer {key}")
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=120) as r:
        u = json.loads(r.read().decode())["usage"]
    pt = u.get("prompt_tokens", 0)
    det = u.get("prompt_tokens_details", {}) or {}
    ct = det.get("cached_tokens", 0) or 0
    return pt, ct

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--env-file", default=None)
    ap.add_argument("--model", default="MiniMax-M3")
    ap.add_argument("--prefix-mult", type=int, default=800)
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--user", default="Say hi in exactly one sentence.")
    a = ap.parse_args()
    key = load_key(a)
    sys_text = BASE_SYS * a.prefix_mult
    for n in range(1, a.repeats + 1):
        pt, ct = call(key, a.model, sys_text, a.user)
        pct = (100 * ct / pt) if pt else 0
        tag = "warm-up" if n == 1 else ("HIT" if ct > pt * 0.5 else "low")
        print(f"call {n}: prompt={pt} cached={ct} ({pct:.1f}%) [{tag}]")

if __name__ == "__main__":
    main()
