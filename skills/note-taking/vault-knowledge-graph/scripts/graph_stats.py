#!/usr/bin/env python3
"""Diagnostic for an Obsidian-style vault knowledge graph.

Walks a vault, reports structural health (note count, orphans, stubs,
avg out-degree, hub in-degree leaderboard) and a broken-wikilink check.

Usage:
    python graph_stats.py /path/to/vault [--known dangling.txt]

The broken-link check uses EXACT basename match, so filenames containing
dots (e.g. `adityasasidhar.github.io.md`) resolve correctly. Wikilinks that
appear inside CLAUDE.md / AGENTS.md doc-example text (e.g. `[[wikilinks]]`,
`[[Note Name]]`) are excluded from the broken count automatically. Wikilinks
inside inline backtick code (e.g. `` `[[foo]]` ``) are also stripped before
counting, since they're syntax examples rather than real links.
"""
import os
import re
import argparse

SKIP_DIRS = {".obsidian", ".git", ".claudian", "node_modules"}
DOC_EXAMPLE_LINKS = {"wikilinks", "Note Name", "Note"}  # CLAUDE.md/AGENTS.md samples


def collect(vault):
    names = set()
    md = {}
    for root, dirs, files in os.walk(vault):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            if not f.endswith(".md"):
                continue
            p = os.path.join(root, f)
            names.add(os.path.splitext(f)[0])
            with open(p, encoding="utf-8", errors="ignore") as fh:
                txt = fh.read()
            # Strip backtick-wrapped inline code so [[...]] examples inside
            # backticks aren't counted as wikilinks. A line like
            # `...the `[[...]]` syntax...` is a literal example, not a link.
            txt = re.sub(r"`+[^`\n]*`+", "", txt)
            links = [l.split("|")[0].split("#")[0].strip()
                     for l in re.findall(r"\[\[([^\]]+?)\]\]", txt)]
            md[p] = links
    return names, md


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vault")
    ap.add_argument("--known", help="file: one 'relpath|Target' dangling link per line to ignore")
    args = ap.parse_args()

    known = set()
    if args.known and os.path.exists(args.known):
        with open(args.known) as fh:
            for line in fh:
                line = line.strip()
                if line:
                    known.add(tuple(line.split("|", 1)))

    names, md = collect(args.vault)
    total = len(md)
    inbound = {n: 0 for n in names}
    out_total = 0
    orphans = empty = stubs = 0
    broken = []
    for p, links in md.items():
        rel = os.path.relpath(p, args.vault)
        size = os.path.getsize(p)
        if size == 0:
            empty += 1
        elif size < 50:
            stubs += 1
        out_total += len(links)
        for t in links:
            if t in DOC_EXAMPLE_LINKS:
                continue
            if t in inbound:
                inbound[t] += 1
        real_out = [t for t in links if t not in DOC_EXAMPLE_LINKS]
        if not real_out and inbound.get(os.path.splitext(os.path.basename(p))[0], 0) == 0:
            orphans += 1
        for t in real_out:
            if t not in names:
                broken.append((rel, t))

    new_broken = [b for b in broken if b not in known]
    print(f"Total notes:           {total}")
    print(f"Empty (0 bytes):       {empty}")
    print(f"Tiny stubs (<50b):     {stubs}")
    print(f"Orphans:               {orphans}")
    print(f"Avg out-links/note:    {out_total/total:.2f}")
    print(f"Pre-existing broken:   {len([b for b in broken if b in known])}")
    print(f"NEW broken:            {len(new_broken)}")
    for b in new_broken:
        print(f"   BROKEN {b[0]} -> [[{b[1]}]]")
    print("\nTop in-degree:")
    for n, c in sorted(inbound.items(), key=lambda x: -x[1])[:10]:
        print(f"   {n}: {c}")


if __name__ == "__main__":
    main()
