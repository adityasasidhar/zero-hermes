# Tiny-repo inventory and verification

Companion to `deep-enrichment-recipes.md` for the edge case where the source
fits in one screen (<30 LOC). The bulk generator's "depth from many files"
approach doesn't apply — there's only one or two source files — so depth
has to come from history, manifests, IDE metadata, and actually running
the program. This file is the playbook for that case.

## When to use this file

- `git ls-files '*.rs' '*.py' '*.ts' '*.js' '*.go' '*.rb' | wc -l` returns ≤ 5.
- The whole program fits on one printed page (≤ 30 logical lines).
- The README is ≤ 5 lines (often just the project title).
- The repo has 1–3 commits.

The signal is "the codebase is a learning exercise, not a product." These
repos are common in `adityasasidhar`'s `Learning-*` family and similar
exploration projects. They still warrant a full-length enriched note
because the **progression** is the interesting part, not the runtime
behavior.

## Tiny-repo inventory batch

Run these before opening any source file. They tell you whether the
repo is a 1-commit "hello world" or a 3-commit learning step.

```bash
# Commit count + shape
git log --oneline | wc -l
git log --date=short --pretty=format:'%h %ad %s' -20

# Full diff of every commit — reveals what was added, deleted, renamed.
# Use this for 1-3 commit repos; it scales poorly past ~10 commits.
for sha in $(git rev-list --max-count=10 HEAD); do
  echo "=== $sha ==="
  git --no-pager show --stat --summary --format=fuller "$sha"
  echo "--- diff ---"
  git --no-pager show --format= --no-ext-diff "$sha"
done

# Tracked file bytes (the current tree, not the .git/objects size)
git ls-files | xargs wc -c

# Find deleted files still in history (binaries left in pack files)
git log --all --diff-filter=D --name-only --pretty=format: | sort -u
git verify-pack -v .git/objects/pack/*.idx 2>/dev/null \
  | sort -k3 -nr | head -5

# Branches + remote refs
git branch -a
git remote -v
```

The `git verify-pack` output is what reconciles GitHub's reported `size`
(KB) against the apparent current tree size. A repo with a 3.88 MB
binary committed and later deleted will show as ~1 MB on GitHub even
though `git ls-files | xargs wc -c` reports only a few KB.

## Manifest deep-dive

The README and the top-level manifest line tell you the *declared*
dependency. The lockfile tells you the *resolved* graph. For a
Rust repo, both are worth reading:

```bash
# Declared
cat Cargo.toml

# Resolved (the transitive stack IS the tech stack)
cat Cargo.lock | grep -E '^name = '

# Or pretty-formatted:
cat Cargo.lock | python3 -c "
import sys, re
text = sys.stdin.read()
# very rough: print package name and version pairs
for m in re.finditer(r'name = \"([^\"]+)\".*?version = \"([^\"]+)\"', text, re.DOTALL):
    print(m.group(1), m.group(2))
" | sort -u
```

For Python repos, read `pyproject.toml` AND `requirements.txt` AND
`requirements-*.txt` (dev/lock/constraints files) — they often disagree.
For JS/TS, `package.json` is the manifest; `package-lock.json` or
`pnpm-lock.yaml` is the resolved graph; `yarn.lock` is its own thing.

## Build / run verification

Don't trust the README. Run it. For a tiny repo this is fast enough to
always do:

```bash
# Rust
cd /path/to/cloned/repo
cargo metadata --locked --no-deps --format-version 1 >/dev/null
cargo test --locked
printf '7\n' | cargo run --locked --quiet
```

```bash
# Python
cd /path/to/cloned/repo
pip install -e . 2>&1 | tail -5
python3 -c 'import <package>; print(<package>.__version__)' || true
python3 -m pytest -q 2>&1 | tail -5
```

```bash
# JS/TS
cd /path/to/cloned/repo
npm ci 2>&1 | tail -5
npm test 2>&1 | tail -10
```

The output belongs in `## Status` and `### How to run it`. For an
interactive program, pipe a known input and capture the actual stdout.
For a CLI, run `--help`. The recorded behavior is often more honest
than the README's claims.

## Tiny-repo note structure

When the program fits on one page, the note's `### Architecture / How it
works` section should be a numbered walkthrough of the actual function
flow (1 → 2 → 3 → N), not a paragraph. Concrete beats prose.

`### Notable details` is where tiny repos earn their keep. Surface:

- **Concept coverage.** Which language concepts does the source
  demonstrate? For Rust: `mut`, `&mut`, `String::new()`, `read_line`,
  `expect()`, `println!`, exclusive range, trait-provided method call,
  `use` import. For Python: `@dataclass`, list comprehensions, type
  hints, async/await, etc. Each is a concept to link in `Concepts:`.
- **Repository/binary name divergence.** `Cargo.toml`'s `name = "forfun"`
  vs the repo `Learning-Rust`. Worth one sentence in both `Notable
  details` and `How to run it`.
- **`.gitignore` vs tracked files.** When `.gitignore` lists `.idea/`
  but `.idea/*.xml` is still tracked, that's a real curiosity.
- **Deleted-but-still-in-history artifacts.** The first commit's 3.88 MB
  `src/main` binary that the second commit deleted explains why GitHub
  reports 1,087 KB.
- **Sparse project metadata.** GitHub has no description, no topics,
  no license — name them.
- **The thing the program *doesn't* do.** A number-guessing game that
  doesn't compare the guess. Be explicit; it's the natural next
  learning step and the user knows it.

## Don't pad tiny repos

Resist the urge to add filler sections to hit the 5–10k char target. The
right depth for a 15-line program is roughly 6–9k chars covering:

1. Frontmatter (existing).
2. `# Title` + blockquote tagline.
3. `## Overview` — 2–3 sentences (progression + what it does).
4. `## Status` — 1 short paragraph.
5. `## Inside the Codebase` — the bulk: architecture walkthrough,
   file structure (root + src + .idea + absent), key files, notable
   details (the longest section), tech stack with locked versions,
   how to run it.
6. `## What this project is about` — 1 paragraph.
7. `## Use cases` — 3–5 bullets.
8. `## Key Features` — 5–7 emoji bullets.
9. `## Getting Started` — 2 code blocks (clone+run, test).
10. `## Stats` — emoji bullets.
11. `## Links` — required `Part of:` footer.

If your first draft is 12k, trim the `### Notable details` prose (it's
always the longest), collapse the architecture walkthrough, and drop
adjectives. Hit 9k by being precise, not by being verbose. The parent
agent verifies with `wc -c` and rejects 12k+.
