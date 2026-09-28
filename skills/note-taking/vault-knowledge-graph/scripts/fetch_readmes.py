#!/usr/bin/env python3
"""Robustly fetch GitHub READMEs for every repo note in a vault's Repos folder.

Why this exists: the naive `gh api .../readme --jq .content | base64 -d` pattern
SILENTLY CORRUPTS repos that have no README. On a 404, `gh api` still prints an
error JSON object to stdout; base64-decoding that JSON yields ~38 bytes of
identical binary garbage that looks like a valid (tiny) file. A downstream agent
then parses junk. This script guards every step:

  - checks gh's exit code (returncode != 0  -> no README)
  - checks stdout.strip() not in ("", "null")
  - decodes base64 then utf-8 with errors="strict"
  - writes an EMPTY (0-byte) marker file for repos with no README, so agents
    can skip cleanly instead of parsing garbage.

Usage:
  python3 fetch_readmes.py
Defaults: scans /home/arctic/Documents/fun/Github/Repos/*.md, writes
/tmp/readmes/<name>.md. Override REPOS_DIR / READMES_OUT via env if needed.
"""
import subprocess, os, base64

REPOS_DIR = os.environ.get("REPOS_DIR", "/home/arctic/Documents/fun/Github/Repos")
OUT = os.environ.get("READMES_OUT", "/tmp/readmes")
os.makedirs(OUT, exist_ok=True)
manifest = []

repos = sorted(os.path.splitext(f)[0] for f in os.listdir(REPOS_DIR) if f.endswith(".md"))
for name in repos:
    try:
        r = subprocess.run(
            ["gh", "api", f"repos/adityasasidhar/{name}/readme", "--jq", ".content"],
            capture_output=True, text=True, timeout=30,
        )
        content = r.stdout.strip()
        if r.returncode != 0 or not content or content == "null":
            open(os.path.join(OUT, name + ".md"), "w").close()  # empty marker
            manifest.append(f"NOREADME  {name}")
            continue
        text = base64.b64decode(content).decode("utf-8", errors="strict")
        with open(os.path.join(OUT, name + ".md"), "w", encoding="utf-8") as fh:
            fh.write(text)
        manifest.append(f"HASREADME {name}")
    except Exception as e:
        open(os.path.join(OUT, name + ".md"), "w").close()  # empty marker
        manifest.append(f"ERROR     {name}: {e}")

with open("/tmp/manifest.txt", "w") as fh:
    fh.write("\n".join(manifest) + "\n")

has = sum(1 for m in manifest if m.startswith("HASREADME"))
print(f"total {len(repos)} | with README {has} | without {len(repos) - has}")
