#!/usr/bin/env python3
"""Verify a vault's .gitignore matches its intended contract.

Tests two classes of paths:
  IGNORE  → git check-ignore must return 0 (path is ignored)
  TRACK   → git check-ignore must return 1 (path is NOT ignored)

Fails loudly with a diff-style report. Exits non-zero if any expectation
is violated. Single-use ad-hoc verifier — NOT a pytest fixture.

Usage:
    python3 scripts/verify_vault_gitignore.py [VAULT_PATH]

Default VAULT_PATH is the current working directory.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()

# (path, expected_ignored, human_label)
IGNORE_CASES = [
    (".obsidian/workspace.json",          True,  "per-machine workspace layout"),
    (".obsidian/workspace.json.bak",      True,  "workspace backup"),
    (".obsidian/graph.json.bak",          True,  "graph view backup"),
    (".claudian/sessions/",               True,  "claudian session metadata"),
    ("Untitled.md",                       True,  "scratch pad note"),
    ("fun.md",                            True,  "scratch dump note"),
    (".DS_Store",                         True,  "macOS noise"),
    ("Thumbs.db",                         True,  "Windows noise"),
    ("foo.swp",                           True,  "vim swap file"),
    ("bar.swo",                           True,  "vim swap file"),
    ("notes.txt~",                        True,  "tilde backup"),
]

TRACK_CASES = [
    (".gitignore",                        False, "gitignore itself"),
    ("Me.md",                             False, "root MOC (if present)"),
    (".obsidian/app.json",                False, "obsidian app config"),
    (".obsidian/community-plugins.json",  False, "community plugin list"),
    (".obsidian/core-plugins.json",       False, "core plugin list"),
    (".obsidian/hotkeys.json",            False, "keybind map"),
    (".obsidian/page-preview.json",       False, "preview config"),
    (".obsidian/webviewer.json",          False, "web viewer config"),
    (".obsidian/snippets/",               False, "css snippets dir"),
    (".obsidian/themes/",                 False, "custom themes dir"),
    (".obsidian/plugins/",                False, "community plugin payloads"),
    (".obsidian/graph.json",              False, "graph view settings"),
    (".obsidian/appearance.json",         False, "appearance settings"),
]


def check_ignore(path: str) -> tuple[bool, str]:
    """Return (is_ignored, stdout_from_git)."""
    result = subprocess.run(
        ["git", "check-ignore", "-v", path],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return (result.returncode == 0, (result.stdout or "").strip())


def main() -> int:
    print(f"Verifying .gitignore in {REPO}\n")

    passed = 0
    failed: list[str] = []

    print("== IGNORE expectations (path SHOULD be ignored) ==")
    for path, expect_ignored, label in IGNORE_CASES:
        is_ignored, info = check_ignore(path)
        ok = is_ignored == expect_ignored
        if ok:
            passed += 1
        else:
            verb = "ignored" if is_ignored else "NOT ignored"
            line = f"  [FAIL] {path:<38} {verb}  ({label})"
            failed.append(line)
            print(line)
            if info:
                print(f"          git said: {info}")

    print("\n== TRACK expectations (path should NOT be ignored) ==")
    for path, expect_ignored, label in TRACK_CASES:
        is_ignored, info = check_ignore(path)
        ok = is_ignored == expect_ignored
        if ok:
            passed += 1
        else:
            verb = "ignored" if is_ignored else "NOT ignored"
            line = f"  [FAIL] {path:<38} {verb}  ({label})"
            failed.append(line)
            print(line)
            if info:
                print(f"          git said: {info}")

    total = len(IGNORE_CASES) + len(TRACK_CASES)
    print(f"\n=== {passed}/{total} passed, {len(failed)} failed ===")

    if failed:
        print("\nFailures:")
        for f in failed:
            print(f)
        return 1

    # Cross-check: do HEAD and the gitignore actually agree?
    # Only runs if HEAD exists (has at least one commit).
    head_result = subprocess.run(
        ["git", "rev-parse", "--verify", "HEAD"],
        cwd=REPO, capture_output=True, text=True,
    )
    if head_result.returncode != 0:
        print("\n(no commits yet — skipping tree cross-check)")
        return 0

    print("\n== Cross-check against committed tree ==")
    out = subprocess.run(
        ["git", "ls-tree", "-r", "HEAD", "--name-only"],
        cwd=REPO, capture_output=True, text=True, check=True,
    )
    tracked = set(out.stdout.splitlines())
    print(f"  Files committed: {len(tracked)}")

    must_not_be_tracked = ["Untitled.md", "fun.md", ".obsidian/workspace.json"]
    leaked = [p for p in must_not_be_tracked if p in tracked]
    if leaked:
        print(f"  [FAIL] these leaked into the commit: {leaked}")
        return 1
    print(f"  [PASS] none of {must_not_be_tracked} are tracked")

    must_be_tracked = [".gitignore", ".obsidian/app.json"]
    missing = [p for p in must_be_tracked if p not in tracked]
    if missing:
        print(f"  [FAIL] these should be tracked but aren't: {missing}")
        return 1
    print(f"  [PASS] all of {must_be_tracked} are tracked")

    return 0


if __name__ == "__main__":
    sys.exit(main())
