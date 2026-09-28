#!/usr/bin/env python3
"""Live MiniMax speed probe — first-party Token-Plan endpoint.

Measures wall-clock tok/s for one or more MiniMax models using the SAME prompt.
Key lessons baked in (from 2026-07-18 session):
  * `MiniMax-M2.7-Fast` does NOT exist -> use `MiniMax-M2.7-HighSpeed`.
  * Use flush + per-call timeout so a bad model name can't silently hang 120s.
  * Always include a known-good control (M3) first.

Run from /home/arctic/hermes (NOT a project repo):
  python3 bench_minimax_speed.py
Key is sourced from the do-code .env (Subscription Key, sk-cp-...).
"""
import os, json, time, sys, urllib.request
from dotenv import load_dotenv

ENV_PATH = "/home/arctic/Internship/DataObserve/do-code/.env"
KEY = os.getenv("MINIMAX_API_KEY")
if not KEY:
    load_dotenv(ENV_PATH)
    KEY = os.getenv("MINIMAX_API_KEY")
if not KEY:
    print("NO_KEY", flush=True); sys.exit(1)

# control first (known-good), then probe fast candidate names
CANDIDATES = [
    ("MiniMax-M3", "control"),
    ("MiniMax-M2.7-HighSpeed", "hs"),
    ("MiniMax-M2.7", "standard"),
]
PROMPT = ("Explain in detail (about 300 words) how sparse mixture-of-experts "
          "language models route tokens to experts, including the router network "
          "and load-balancing. Be thorough.")

for model, tag in CANDIDATES:
    body = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": PROMPT}],
        "max_tokens": 400,
        "temperature": 0.3,
    }).encode()
    req = urllib.request.Request(
        "https://api.minimax.io/v1/chat/completions", data=body,
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {KEY}"})
    print(f"==> trying {model} ({tag})", flush=True)
    try:
        t0 = time.time()
        with urllib.request.urlopen(req, timeout=40) as r:
            resp = json.loads(r.read().decode())
        t1 = time.time()
        u = resp["usage"]
        comp = u["completion_tokens"]; pt = u.get("prompt_tokens", 0)
        elapsed = t1 - t0; tps = comp / elapsed
        cached = u.get("prompt_tokens_details", {}).get("cached_tokens", 0)
        print(f"    OK  completion={comp} prompt={pt} cached={cached} "
              f"elapsed={elapsed:.2f}s  =>  {tps:.1f} tok/s", flush=True)
    except urllib.error.HTTPError as e:
        print(f"    HTTP {e.code}: {e.read().decode()[:200]}", flush=True)
    except Exception as e:
        print(f"    ERR {type(e).__name__}: {e}", flush=True)
