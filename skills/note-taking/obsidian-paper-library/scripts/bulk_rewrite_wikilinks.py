#!/usr/bin/env python3
"""
bulk_rewrite_wikilinks.py — Apply a list of (pattern, replacement) regex
pairs to every .md file under a target directory. Use after slug renames or
when fixing consistency across many notes (e.g. switching from [[../Papers/X]]
to [[X]]).

Usage:
    python bulk_rewrite_wikilinks.py <folder> --dry-run
    python bulk_rewrite_wikilinks.py <folder> [--rule PATTERN=REPL] ...

If no --rule is supplied, runs a sensible default cleanup:

    [[../Papers/Papers]]          ->  [[Papers]]
    [[../Papers]]                 ->  [[Papers]]
    [[../papers/<name>]]          ->  [[<name>]]
    [[../kimi-<name>]]            ->  [[kimi-<name>]]

Edit the DEFAULTS dict below to add custom rules.
"""

from __future__ import annotations
import argparse
import os
import re
import sys
from pathlib import Path


DEFAULTS = [
    (r"\[\[\.\./Papers/Papers\]\]",        "[[Papers]]"),
    (r"\[\[\.\./Papers\]\]",               "[[Papers]]"),
    (r"\[\[\.\./papers/([\w\-\.]+)\]\]",   r"[[\1]]"),
    (r"\[\[\.\./kimi-([\w\-\.]+)\]\]",     r"[[kimi-\1]]"),
]


def apply(content: str, rules: list[tuple[str, str]]) -> tuple[str, int]:
    """Apply each (pattern, replacement) in order; return new content + total subs."""
    total = 0
    for pat, rep in rules:
        new_content, n = re.subn(pat, rep, content)
        total += n
        content = new_content
    return content, total


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("folder", type=Path, help="folder to rewrite under (recursive)")
    p.add_argument("--dry-run", action="store_true", help="don't write, just report")
    p.add_argument("--rule", action="append", default=[],
                   help="extra PATTERN=REPL rule (regex); applied after defaults")
    args = p.parse_args()

    rules = list(DEFAULTS)
    for r in args.rule:
        if "=" not in r:
            print(f"--rule must be PATTERN=REPL, got: {r!r}", file=sys.stderr)
            return 2
        pat, rep = r.split("=", 1)
        rules.append((pat, rep))

    folder = args.folder.resolve()
    if not folder.is_dir():
        print(f"not a directory: {folder}", file=sys.stderr)
        return 2

    changed = 0
    total_subs = 0
    for r, _, fs in os.walk(folder):
        for f in fs:
            if not f.endswith(".md"):
                continue
            p = Path(r) / f
            with p.open() as fh:
                content = fh.read()
            new, n = apply(content, rules)
            if n and not args.dry_run:
                with p.open("w") as fh:
                    fh.write(new)
                print(f"  patched ({n:>3} sub) {p}")
            elif n:
                print(f"  would patch ({n:>3} sub) {p}")
            total_subs += n
            if n:
                changed += 1

    verb = "would change" if args.dry_run else "changed"
    print(f"\n{verb} {changed} files; {total_subs} total substitutions.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
