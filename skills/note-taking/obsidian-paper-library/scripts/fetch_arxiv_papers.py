#!/usr/bin/env python3
"""
fetch_arxiv_papers.py — Download a batch of arXiv PDFs to per-paper folders.

Usage:
    python fetch_arxiv_papers.py --base <out-dir> --pairs "Name|YYMM.NNNNN" ...

Pairs may be passed inline or via a JSON file (--from-json). Concurrency
capped via delay to be polite to arXiv (~1 RPS).

Example:
    python fetch_arxiv_papers.py \
        --base ~/Documents/fun/Research/Papers \
        --pairs "Mooncake|2407.00079" "Muon|2502.16982"

Verifies each PDF starts with `%PDF-` (real PDF, not an HTML error page).
Reports size and page count via pdfinfo if available.
"""

from __future__ import annotations
import argparse
import json
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path


PDF_URL = "https://arxiv.org/pdf/{id}"


def fetch_pdf(arxiv_id: str, out_path: Path) -> tuple[bool, str]:
    """Download one PDF; return (ok, status_message)."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    url = PDF_URL.format(id=arxiv_id)
    req = urllib.request.Request(url, headers={"User-Agent": "Hermes-paper-library/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = resp.read()
    except Exception as e:
        return False, f"download error: {e!r}"
    if not data.startswith(b"%PDF-"):
        return False, f"not a PDF (head={data[:32]!r})"
    out_path.write_bytes(data)
    return True, f"OK ({len(data):,} bytes)"


def page_count(pdf_path: Path) -> int | None:
    pdfinfo = shutil.which("pdfinfo")
    if not pdfinfo:
        return None
    try:
        r = subprocess.run([pdfinfo, str(pdf_path)], capture_output=True, text=True, timeout=10)
        for line in r.stdout.splitlines():
            if line.lower().startswith("pages:"):
                return int(line.split(":", 1)[1].strip())
    except Exception:
        return None
    return None


def parse_inline_pairs(pairs: list[str]) -> list[tuple[str, str]]:
    out = []
    for p_str in pairs:
        if "|" not in p_str:
            print(f"--pair must be Name|ID, got {p_str!r}", file=sys.stderr)
            sys.exit(2)
        name, arxiv_id = p_str.split("|", 1)
        out.append((name.strip(), arxiv_id.strip()))
    return out


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--base", type=Path, required=True, help="output base directory")
    p.add_argument("--pairs", nargs="+", default=[],
                   help='pairs as "Name|YYMM.NNNNN"')
    p.add_argument("--from-json", type=Path,
                   help="JSON file: [{\"name\": \"...\", \"id\": \"...\"}, ...]")
    p.add_argument("--delay", type=float, default=1.1,
                   help="seconds between downloads (default: 1.1; arXiv polite)")
    args = p.parse_args()

    pairs: list[tuple[str, str]] = []
    if args.from_json:
        data = json.loads(args.from_json.read_text())
        pairs = [(d["name"], d["id"]) for d in data]
    elif args.pairs:
        pairs = parse_inline_pairs(args.pairs)

    if not pairs:
        print("no pairs supplied", file=sys.stderr)
        return 2

    base = args.base.expanduser().resolve()
    base.mkdir(parents=True, exist_ok=True)

    print(f"downloading {len(pairs)} papers to {base}")
    failed = 0
    for name, arxiv_id in pairs:
        target = base / name / "paper.pdf"
        ok, msg = fetch_pdf(arxiv_id, target)
        pages = page_count(target) if ok else None
        page_info = f", {pages} pages" if pages is not None else ""
        marker = "OK" if ok else "FAIL"
        print(f"  {marker:>4}  {name:<32}  arXiv:{arxiv_id}  {msg}{page_info}")
        if ok:
            time.sleep(args.delay)
        else:
            failed += 1

    if failed:
        print(f"\n{failed} downloads failed.")
        return 1
    print(f"\nall {len(pairs)} PDFs downloaded.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
