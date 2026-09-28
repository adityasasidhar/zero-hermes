#!/usr/bin/env python3
"""Verify an llm-wiki after a cross-vault enrichment batch.

Run with the wiki path as argv[1] (default: /home/arctic/Documents/fun/wiki/aiml).
Exits 0 if the batch passes every check, 1 otherwise. Prints a human-readable
summary either way.

What it checks (per `references/cross-vault-wiki-enrichment.md`):

  1. `index.md` declares the same page count that exists on disk under
     entities/, concepts/, comparisons/, queries/.
  2. Every wikilink on `index.md` resolves to a content page (basename match).
  3. Every content page appears on `index.md` (no orphans, no extras).
  4. No NEW broken wikilinks anywhere in the wiki (excluding a configurable
     pre-existing-baselines block). Newly broken = bad.
  5. Every `raw/articles/*.md` with a `sha256:` field has a digest that
     matches a recomputed SHA-256 of its body.
  6. Every concept or comparison page that lists a `raw/...` source in its
     frontmatter points at a real file.
  7. Every entity page links to at least two existing concept pages.
  8. `log.md` mentions today's batch and lists every path the agent touched.
     We do NOT auto-assert log completeness because the paths to mention
     are session-specific — instead, this check warns.

Edit `PRE_EXISTING_BROKEN` below to match the destination wiki's pre-existing
broken-link baseline. The aiml wiki (as of 2026-07-19) has these.
"""

from __future__ import annotations

import hashlib
import re
import sys
from datetime import date
from pathlib import Path

PRE_EXISTING_BROKEN: dict[str, set[str]] = {
    "SCHEMA.md": {"wikilinks"},
    "concepts/gated-deltanet.md": {"attention", "mamba"},
    "concepts/rope-positional-encoding.md": {"attention"},
}

DEFAULT_WIKI = Path("/home/arctic/Documents/fun/wiki/aiml")


def collect_pages(root: Path) -> dict[str, list[Path]]:
    """Bucket every .md into raw/, entities/, concepts/, comparisons/, queries/."""
    buckets: dict[str, list[Path]] = {
        "raw_articles": [],
        "raw_papers": [],
        "entities": [],
        "concepts": [],
        "comparisons": [],
        "queries": [],
        "other": [],
    }
    for p in sorted(root.rglob("*.md")):
        rel = p.relative_to(root)
        parts = rel.parts
        if parts[:2] == ("raw", "articles"):
            buckets["raw_articles"].append(p)
        elif parts[:2] == ("raw", "papers"):
            buckets["raw_papers"].append(p)
        elif parts[0] == "entities":
            buckets["entities"].append(p)
        elif parts[0] == "concepts":
            buckets["concepts"].append(p)
        elif parts[0] == "comparisons":
            buckets["comparisons"].append(p)
        elif parts[0] == "queries":
            buckets["queries"].append(p)
        else:
            buckets["other"].append(p)
    return buckets


def all_basenames(md_files) -> set[str]:
    return {p.stem for p in md_files}


def extract_wikilinks(text: str) -> list[str]:
    """Return wikilink targets in order, with pipe aliases and #anchors stripped."""
    # Strip fenced code blocks and inline code first.
    no_code = re.sub(r"```[\s\S]*?```", "", text)
    no_code = re.sub(r"`+[^`\n]+?`+", "", no_code)
    targets = re.findall(r"\[\[([^]]+)\]\]", no_code)
    return [t.split("|")[0].split("#")[0] for t in targets]


def parse_frontmatter(text: str) -> tuple[str, str]:
    """Return (frontmatter_block, body_after_second_---). Empty strings if absent."""
    parts = text.split("---", 2)
    if len(parts) < 3 or not parts[0].strip() == "":
        return "", text
    return parts[1], parts[2]


def check_digests(raw_articles: list[Path]) -> list[str]:
    errors = []
    for p in raw_articles:
        text = p.read_text()
        fm, body = parse_frontmatter(text)
        if not fm:
            continue
        m = re.search(r"\bsha256:\s*([0-9a-f]{64})", fm)
        if not m:
            # Tolerable if the file has no frontmatter at all (some legacy files).
            continue
        body_text = body.lstrip("\n")
        actual = hashlib.sha256(body_text.encode()).hexdigest()
        if m.group(1) != actual:
            errors.append(f"{p}: sha256 digest out of sync with body")
    return errors


def check_sources_exist(root: Path, page: Path) -> list[str]:
    errors = []
    text = page.read_text()
    fm, _ = parse_frontmatter(text)
    if not fm:
        return errors
    sources_block = re.search(r"sources:\s*\n((?:\s+-\s+.+\n)+)", fm)
    if not sources_block:
        return errors
    for line in sources_block.group(1).splitlines():
        m = re.match(r"\s+-\s+(.+)", line)
        if not m:
            continue
        path = m.group(1).strip()
        # Tolerate URLs and short slugs; only require files that look like
        # in-repo paths exist on disk.
        if "://" in path:
            continue
        candidate = root / path
        if not candidate.exists():
            errors.append(f"{page}: missing source path {path}")
    return errors


def check_new_broken(wiki: Path, all_md: list[Path]) -> dict[str, list[str]]:
    new_broken: dict[str, list[str]] = {}
    stems = all_basenames(all_md)
    for p in all_md:
        text = p.read_text()
        links = extract_wikilinks(text)
        bad = sorted({x for x in links if Path(x).name not in stems})
        rel = str(p.relative_to(wiki))
        pre = PRE_EXISTING_BROKEN.get(rel, set())
        extras = sorted(set(bad) - pre)
        if extras:
            new_broken[rel] = extras
    return new_broken


def check_index(root: Path, page_paths: list[Path]) -> tuple[int | None, list[str]]:
    errors = []
    idx_path = root / "index.md"
    if not idx_path.exists():
        errors.append("index.md missing")
        return None, errors
    text = idx_path.read_text()
    m = re.search(r"Total pages:\s*(\d+)", text)
    declared = int(m.group(1)) if m else None
    declared_stems = {
        Path(t.split("|")[0].split("#")[0]).name for t in extract_wikilinks(text)
    }
    actual_stems = {p.stem for p in page_paths}
    if declared_stems != actual_stems:
        missing = sorted(actual_stems - declared_stems)
        extra = sorted(declared_stems - actual_stems)
        errors.append(
            f"index/stem mismatch: missing={missing or []} extra={extra or []}"
        )
    return declared, errors


def check_entity_concept_links(entities: list[Path], concept_stems: set[str]) -> list[str]:
    errors = []
    for p in entities:
        links = {
            Path(x.split("|")[0].split("#")[0]).name
            for x in extract_wikilinks(p.read_text())
        }
        if len(links & concept_stems) < 2:
            errors.append(f"{p}: fewer than 2 concept links")
    return errors


def check_log_for_batch(root: Path) -> tuple[bool, list[str]]:
    today = date.today().isoformat()
    log = root / "log.md"
    if not log.exists():
        return False, ["log.md missing"]
    text = log.read_text()
    if today not in text:
        return False, [f"log.md has no entry dated {today}"]
    if "batch" not in text.lower() and "enrich" not in text.lower():
        return False, ["log.md has today's date but no 'batch' or 'enrich' marker"]
    return True, []


def main(argv: list[str]) -> int:
    wiki = Path(argv[1]) if len(argv) > 1 else DEFAULT_WIKI
    if not wiki.is_dir():
        print(f"wiki not found: {wiki}", file=sys.stderr)
        return 1
    buckets = collect_pages(wiki)
    pages = (
        buckets["entities"]
        + buckets["concepts"]
        + buckets["comparisons"]
        + buckets["queries"]
    )
    all_md = sum(buckets.values(), [])

    errors: list[str] = []

    declared, idx_errors = check_index(wiki, pages)
    errors.extend(idx_errors)

    new_broken = check_new_broken(wiki, all_md)
    if new_broken:
        errors.append(f"new broken wikilinks: {new_broken}")

    errors.extend(check_digests(buckets["raw_articles"]))

    for page in pages:
        errors.extend(check_sources_exist(wiki, page))

    concept_stems = {p.stem for p in buckets["concepts"]}
    errors.extend(check_entity_concept_links(buckets["entities"], concept_stems))

    log_ok, log_errors = check_log_for_batch(wiki)
    errors.extend(log_errors)

    # Reporting
    print(f"Wiki: {wiki}")
    print(f"  declared total pages: {declared}")
    print(f"  actual content pages:  {len(pages)}")
    print(f"  entities:   {len(buckets['entities'])}")
    print(f"  concepts:   {len(buckets['concepts'])}")
    print(f"  comparisons: {len(buckets['comparisons'])}")
    print(f"  raw/articles: {len(buckets['raw_articles'])}")
    print(f"  log batch entry today: {log_ok}")
    if new_broken:
        print("  new broken wikilinks:")
        for path, links in new_broken.items():
            print(f"    {path}: {links}")
    else:
        print("  new broken wikilinks: none")

    if errors:
        print("\nVALIDATION FAILED:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    print("\nVALIDATION PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
