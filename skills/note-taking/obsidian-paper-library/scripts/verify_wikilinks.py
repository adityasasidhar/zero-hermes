#!/usr/bin/env python3
"""
verify_wikilinks.py — Check that every Obsidian [[wikilink]] in the target
folder resolves to an actual file in the vault. Mimics Obsidian's resolution:
[[X]] tries same folder, then siblings, then walks up the folder chain.

Usage:
    python verify_wikilinks.py <vault-root> [<folder> ...]

    vault-root = absolute path to the Obsidian vault root
    folder(s)  = subfolders to scope the check to (default: whole vault)

Exit code 0 if all wikilinks resolve, 1 otherwise.

Notes on the matching:
- We look for ANY file in the vault whose basename (without .md) matches the
  link basename (case-insensitive). Obsidian does this kind of lookup.
- We do not simulate Obsidian's "path up the chain" precisely — we approximate
  it by walking the whole vault. False positives are rare; a real broken link
  is a real broken link.
"""

from __future__ import annotations
import os
import re
import sys
from pathlib import Path


WIKILINK_RE = re.compile(r"\[\[([^\]\n]+?)\]\]")


def find_all_md(vault_root: Path) -> dict[str, Path]:
    """Map basename (lowercased) to first file path with that name under vault."""
    out: dict[str, Path] = {}
    for r, _, fs in os.walk(vault_root):
        for f in fs:
            if f.endswith(".md"):
                key = f[:-3].lower()
                # prefer the first found; doesn't matter for "does it exist"
                out.setdefault(key, Path(r) / f)
    return out


def extract_links(md_path: Path) -> list[tuple[str, int]]:
    """Extract [[link]]s with line numbers."""
    out = []
    with md_path.open() as fh:
        for lineno, line in enumerate(fh, 1):
            for m in WIKILINK_RE.finditer(line):
                out.append((m.group(1).strip(), lineno))
    return out


def link_target(link: str) -> str:
    """Strip alias & heading:  [[Some Name|alias]]  [[Some Name#heading]]"""
    return link.split("|", 1)[0].split("#", 1)[0].strip()


def resolve(target: str, by_basename: dict[str, Path], from_file: Path) -> Path | None:
    """Resolve a wikilink target. Returns path or None."""
    base = os.path.basename(target)
    if not base:
        return None
    # exact case-insensitive match
    candidate = by_basename.get(base.lower())
    if candidate:
        return candidate
    # also accept "<base>.md" name only (in case someone wrote [[foo.md]])
    if base.lower().endswith(".md"):
        candidate = by_basename.get(base[:-3].lower())
        if candidate:
            return candidate
    return None


def main() -> int:
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <vault-root> [folder ...]", file=sys.stderr)
        return 2

    vault_root = Path(sys.argv[1]).expanduser().resolve()
    if not vault_root.is_dir():
        print(f"vault root not a directory: {vault_root}", file=sys.stderr)
        return 2

    scopes = [Path(arg).resolve() for arg in sys.argv[2:]] if len(sys.argv) > 2 else [vault_root]

    by_basename = find_all_md(vault_root)

    broken: list[tuple[Path, str, int]] = []
    total_links = 0
    seen_syntax = {"wikilinks"}  # known non-link occurrences

    for scope in scopes:
        for r, _, fs in os.walk(scope):
            for f in fs:
                if not f.endswith(".md"):
                    continue
                p = Path(r) / f
                for link, lineno in extract_links(p):
                    total_links += 1
                    target = link_target(link)
                    # skip common non-link prose
                    if target.lower() in seen_syntax or not target:
                        continue
                    # also skip URLs accidentally wrapped: [[http://...]]
                    if "://" in target:
                        continue
                    if resolve(target, by_basename, p) is None:
                        broken.append((p, link, lineno))

    # summary
    if not broken:
        print(f"OK: {total_links} wikilinks, all resolve.")
        return 0

    print(f"BROKEN: {len(broken)} wikilinks (of {total_links} total):")
    for p, link, lineno in broken:
        print(f"   {p}:{lineno}  [[{link}]]")
    return 1


if __name__ == "__main__":
    sys.exit(main())
