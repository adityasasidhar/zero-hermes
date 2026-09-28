#!/usr/bin/env python3
"""Discover recurring cross-cutting concepts in a vault's repo notes.

Scans every note under <vault>/Github/Repos (and any extra --dirs) for
technique keywords and reports which concepts appear in >= MIN repos.
Output is the candidate list to turn into `Concepts/<Name>.md` notes
(see references/graph-growth-levers.md, Lever 2).

Usage:
    python discover_concepts.py /path/to/vault [--min 2] [--json]

This is a SCAFFOLD: the CONCEPTS dict below is the pattern from
adityasasidhar's vault. Edit the regex patterns to fit a new vault, or
pass nothing and just use it as a template.
"""
import os
import re
import argparse
import json

REPOS_SUBDIR = os.path.join("Github", "Repos")

# (display_name, [regex patterns, case-insensitive])
CONCEPTS = {
    "Transformers": [r"transformer", r"decoder-only", r"attention"],
    "RoPE": [r"rope", r"rotary"],
    "SwiGLU": [r"swiglu"],
    "RMSNorm": [r"rmsnorm", r"rms norm"],
    "BabyLM": [r"babylm"],
    "Small Language Models": [r"small language model", r"\bslm\b", r"tiny transformer"],
    "Mixture of Experts": [r"\bmoe\b", r"mixture of experts", r"expert"],
    "LLM Routing": [r"router", r"routing", r"\broute\b"],
    "Model Context Protocol": [r"\bmcp\b", r"model context protocol"],
    "Agent Harnesses": [r"agent harness", r"\bharness\b", r"agent framework"],
    "Self-Improving Agents": [r"self-improv", r"self improving", r"evolv.*system prompt", r"context engineering"],
    "Reinforcement Learning": [r"reinforcement", r"\brl\b", r"reward"],
    "Computer Vision": [r"\bcnn\b", r"resnet", r"convolutional", r"image classification", r"cifar", r"fashion mnist"],
    "Ollama": [r"ollama"],
    "From-Scratch ML": [r"from scratch", r"from-scratch"],
    "Depth Recursion": [r"recursi", r"weight-?tied", r"depth-recursive", r"weight tying"],
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vault")
    ap.add_argument("--min", type=int, default=2, help="min repos a concept must appear in")
    ap.add_argument("--dirs", nargs="*", default=[], help="extra dirs under vault to scan")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    scan_dirs = [os.path.join(args.vault, REPOS_SUBDIR)] + \
                [os.path.join(args.vault, d) for d in args.dirs]
    scan_dirs = [d for d in scan_dirs if os.path.isdir(d)]

    repo_txt = {}
    for d in scan_dirs:
        for f in sorted(os.listdir(d)):
            if f.endswith(".md"):
                p = os.path.join(d, f)
                with open(p, encoding="utf-8", errors="ignore") as fh:
                    repo_txt[os.path.splitext(f)[0]] = fh.read().lower()

    results = {}
    for c, pats in CONCEPTS.items():
        members = [r for r, t in repo_txt.items() if any(re.search(p, t) for p in pats)]
        if len(members) >= args.min:
            results[c] = sorted(members)

    if args.json:
        print(json.dumps(results, indent=2))
    else:
        for c, mem in sorted(results.items(), key=lambda x: -len(x[1])):
            print(f"\n## {c}  ({len(mem)})")
            print("   " + ", ".join(mem))


if __name__ == "__main__":
    main()
