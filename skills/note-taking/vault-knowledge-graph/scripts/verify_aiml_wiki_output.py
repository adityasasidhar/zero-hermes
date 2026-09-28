#!/usr/bin/env python3
"""Verify an aiml-wiki deep-enrichment output file against BRIEF-TEMPLATE requirements.

Usage:
    python3 verify_aiml_wiki_output.py <path-to-md-file>
    python3 verify_aiml_wiki_output.py <path-to-md-file> --min 6000 --max 10000
    python3 verify_aiml_wiki_output.py <path-to-md-file> --wiki-root /home/arctic/Documents/fun/wiki

Checks (prints [PASS]/[FAIL] per line, exits non-zero on any FAIL):
  - File size in target byte range (default 6000-10000)
  - YAML frontmatter present (between --- lines)
  - All 11 required sections present (Overview, History / Motivation,
    Mechanism, Math, Implementation, Variants, Use cases, Trade-offs,
    Connections, Open questions, References)
  - ≥5 unique outbound wikilinks
  - PASS line format matches `^PASS: <N> bytes, <K> outbound links$`
  - PASS line byte count matches actual file size
  - PASS line wikilink count matches actual unique outbound wikilinks
  - build_index.py reports broken=0 for the wiki

Why this script exists: in the 2026-07-29 RoPE leaf run, byte-budget discipline
failed on the first write (15,454 bytes vs 10,000 ceiling). Six rounds of
patch-trimming followed, and after EACH round the agent had to manually
re-run `wc -c`, re-grep wikilinks, re-check section presence, and re-verify
the PASS line. That's ~6 tool calls per round × 6+ rounds = 30+ tool calls
of pure verification work. This script collapses each round's verification
to one call and exits non-zero on any FAIL so it can be used as a gate.

Session-evidenced failure modes this catches:
  - File under/over budget (the trim loop's main signal)
  - Section accidentally deleted by an over-broad patch (RoPE run: lost
    ## History / Motivation because the patch's old_string included its
    body but new_string didn't — only visible by re-running schema check)
  - PASS line byte drift from the file size after the final patch
  - PASS line format drift (placeholder "TBD" or angle-bracket literals
    that grep -c 'PASS:' would match but the parent cannot parse)
  - Broken wikilinks introduced by the rewrite (regression vs the
    pre-write build_index baseline)

Exit code: 0 if all checks PASS, 1 if any FAIL.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys

REQUIRED_SECTIONS = [
    "Overview",
    "History / Motivation",
    "Mechanism",
    "Math",
    "Implementation",
    "Variants",
    "Use cases",
    "Trade-offs",
    "Connections",
    "Open questions",
    "References",
]

# Match `[[target]]` or `[[target|display]]`. Excludes `![[embed]]` (images).
WIKILINK_RE = re.compile(r"(?<!!)\[\[([^\]|]+)(?:\|[^\]]*)?\]\]")
PASS_LINE_RE = re.compile(r"^PASS: (\d+) bytes, (\d+) outbound links$")
FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?)\n---\s*\n", re.DOTALL)
SECTION_HEADER_RE = re.compile(r"(?m)^## (.+?)\s*$")


def check(name: str, ok: bool, detail: str = "") -> bool:
    """Print one [PASS]/[FAIL] line. Returns ok for chaining/counting."""
    status = "PASS" if ok else "FAIL"
    line = f"[{status}] {name}"
    if detail:
        line += f" -- {detail}"
    print(line)
    return ok


def verify(
    path: str,
    min_bytes: int = 6000,
    max_bytes: int = 10000,
    wiki_root: str = "/home/arctic/Documents/fun/wiki",
    baseline_broken: int | None = None,
) -> int:
    if not os.path.exists(path):
        print(f"[FAIL] file not found: {path}")
        return 1

    try:
        text = open(path).read()
    except OSError as e:
        print(f"[FAIL] cannot read {path}: {e}")
        return 1

    size = os.path.getsize(path)
    failed = 0

    # 1. Byte range
    if not check(
        "byte range",
        min_bytes <= size <= max_bytes,
        f"{size} bytes (target {min_bytes}-{max_bytes})",
    ):
        failed += 1

    # 2. Frontmatter
    fm_match = FRONTMATTER_RE.match(text)
    if not check("frontmatter present", fm_match is not None, f"head: {text[:60]!r}"):
        failed += 1

    # 3. All 11 sections
    found_sections = [m.group(1).strip() for m in SECTION_HEADER_RE.finditer(text)]
    missing = [s for s in REQUIRED_SECTIONS if s not in found_sections]
    if not check(
        "all 11 required sections present",
        not missing,
        f"missing: {missing}" if missing else f"found {len(found_sections)} H2s",
    ):
        failed += 1

    # 4. Wikilink count
    wikilinks = set(WIKILINK_RE.findall(text))
    if not check(
        ">=5 unique outbound wikilinks",
        len(wikilinks) >= 5,
        f"{len(wikilinks)} unique",
    ):
        failed += 1

    # 5. PASS line format + accuracy
    last_line = text.strip().split("\n")[-1]
    pm = PASS_LINE_RE.match(last_line)
    if not check("PASS line format", pm is not None, f"last line: {last_line!r}"):
        failed += 1
    if pm:
        pass_bytes, pass_links = int(pm.group(1)), int(pm.group(2))
        if not check(
            "PASS line byte count matches file",
            pass_bytes == size,
            f"PASS claims {pass_bytes}, file is {size}",
        ):
            failed += 1
        if not check(
            "PASS line wikilink count matches",
            pass_links == len(wikilinks),
            f"PASS claims {pass_links}, file has {len(wikilinks)}",
        ):
            failed += 1

    # 6. build_index broken-link check
    # The leaf agent's responsibility is "broken stays at baseline" (i.e. zero
    # NEW broken links), not "broken == 0 total" -- the wiki may already have
    # pre-existing broken links. Pass --baseline-broken to enforce the
    # no-regression rule; otherwise we just report the current count.
    idx_script = os.path.join(wiki_root, "build_index.py")
    if os.path.exists(idx_script):
        try:
            result = subprocess.run(
                ["python3", idx_script, "--check"],
                capture_output=True,
                text=True,
                timeout=30,
            )
            first_line = (result.stdout or "").split("\n")[0]
            broken_match = re.search(r"broken=(\d+)", first_line)
            if broken_match:
                broken = int(broken_match.group(1))
                if baseline_broken is not None:
                    if not check(
                        "no new broken wikilinks",
                        broken <= baseline_broken,
                        f"baseline={baseline_broken}, current={broken}",
                    ):
                        failed += 1
                else:
                    check(
                        "build_index broken (informational)",
                        True,
                        f"current broken={broken} (no baseline supplied; pass --baseline-broken N to enforce no-regression)",
                    )
            else:
                check("build_index parse", False, f"no broken= in: {first_line!r}")
                failed += 1
        except subprocess.TimeoutExpired:
            check("build_index check ran", False, "timeout after 30s")
            failed += 1
        except Exception as e:
            check("build_index check ran", False, str(e))
            failed += 1
    else:
        check(
            "build_index.py exists",
            False,
            f"not found at {idx_script}; pass --wiki-root to override",
        )
        failed += 1

    # Summary
    print()
    if failed:
        print(f"FAILED: {failed} check(s) failed for {path}")
        return 1
    print(f"OK: {path} passes all checks")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Verify an aiml-wiki deep-enrichment output file.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("path", help="Path to aiml-wiki output .md")
    parser.add_argument(
        "--min", type=int, default=6000, help="Minimum byte size (default 6000)"
    )
    parser.add_argument(
        "--max", type=int, default=10000, help="Maximum byte size (default 10000)"
    )
    parser.add_argument(
        "--wiki-root",
        default="/home/arctic/Documents/fun/wiki",
        help="Wiki root for build_index.py (default: /home/arctic/Documents/fun/wiki)",
    )
    parser.add_argument(
        "--baseline-broken",
        type=int,
        default=None,
        help="Pre-write broken-link count; if supplied, current count must not exceed it",
    )
    args = parser.parse_args()
    sys.exit(
        verify(
            args.path,
            min_bytes=args.min,
            max_bytes=args.max,
            wiki_root=args.wiki_root,
            baseline_broken=args.baseline_broken,
        )
    )


if __name__ == "__main__":
    main()