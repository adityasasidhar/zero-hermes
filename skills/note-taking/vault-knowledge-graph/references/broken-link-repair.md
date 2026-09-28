# Broken-link repair (Obsidian vault)

When `wiki/INDEX.json` (or the local manifest) reports N broken wikilinks
across M source files, the right job is **classify-and-fix**, not a
wholesale rewrite. This reference is the recipe that emerged from a
session that started with "connect things in our obsidian graph" and ended
up finding 310 broken links caused by a previous deep-enrichment pass.

## When to use this

- `wiki/INDEX.json` (or your vault's manifest) reports `broken_links > 0`
  and you want them gone
- A bulk enrichment pass added concept/repo links to many sources, but the
  targets don't exist yet
- Two parallel systems (hand-written + agent-maintained) coexist in the
  vault and you want deliberate cross-system bridges

## Step 1 — Diagnose correctly (the diagnostic asymmetry)

Two scripts can run on the same vault and disagree:

| Tool | What it reports | How it resolves |
|---|---|---|
| `python3 wiki/build_index.py` | `broken: 0` when there are none | Mirrors Obsidian's recursive-name resolution + handles bare-stem ambiguity intentionally |
| `python3 graph_stats.py <vault>` | `broken: 699` for the same vault | Walks the tree literally; resolves `[[X]]` against the source's own folder |

**Trust the manifest.** `wiki/INDEX.json` is the canonical source of truth
because it knows Obsidian's resolution rules. The skill's own
`graph_stats.py` is a structural diagnostic (avg out-links, in-degree
leaderboard, orphan count) — it is NOT a correct broken-link counter.

Rule: when the two disagree, **run `python3 wiki/build_index.py` and read
the canonical `counts.broken_links`** before deciding there's a problem.

Common false-positive causes `graph_stats.py` will flag:
- Cross-area bare-stem links like `[[RMSNorm]]` from `wiki/aiml/concepts/rmsnorm.md`
  → resolves in Obsidian to `Concepts/RMSNorm.md` (the hand-written hub)
- Cross-vault links like `[[wiki/aiml/SCHEMA]]` from `wiki/aiml/index.md` →
  resolves to `wiki/SCHEMA.md` (the umbrella schema)
- See `wiki/AMBIGUOUS-LINKS.md` for the 18 deliberately-duplicated stems

Common real-broken-link sources (worth trusting):
- Concept links added by a deep-enrichment subagent that didn't verify the
  target exists
- Single-source concept links with no inbound references
- Folder-path references like `[[Hobbies/Cooking/recipies]]` (target is a
  folder, not a note)

## Step 2 — Classify the broken targets

For each unique target, build a **policy** using this rubric:

```
def policy(target, occurrences, existing):
    norm = normalize(target)  # lowercase, strip non-alphanumeric
    # 1. AUTO_FIX: exact normalized match to existing note (case/sep-only)
    if norm in existing: return 'AUTO_FIX', existing[norm]
    # 2. FUZZY_AUTO: tight similarity (≥0.85 SequenceMatcher ratio OR
    #    near-substring with ≤3-char difference) to existing CONCEPT note
    # (paper/repo notes are NOT valid concept targets — promote to CREATE_STUB)
    if fuzzy_concept_match(norm, existing): return 'FUZZY_AUTO', target_path
    # 3. CREATE_STUB: 2+ occurrences, target is a real concept name
    if occurrences >= 2: return 'CREATE_STUB', None
    # 4. STRIP: single-source, low-signal, not worth a page
    return 'STRIP', None
```

Observations from one session (310 broken links):
- ~215 STRIP (single-source, low-signal)
- ~41 CREATE_STUB (≥2 sources, real concept)
- ~5 FUZZY_AUTO (close to existing concept)
- ~2 AUTO_FIX (trivial rewrite)

**Threshold tuning**: `occurrences >= 2` is the right default. Drop to
`>= 1` only if the target name is something the user clearly wants
(checked via the source repo notes — what does the link text say?). Keep
at `>= 2` for noise (target is a foreign library/tool name that shouldn't
have its own page).

## Step 3 — Slice the work for parallel subagents

Round robin by load (total broken-link count per source), not by file count.
If slice loads are unequal, redistribute by moving the heaviest files
out of the heaviest slice.

```python
# 3 subagents, ~100 broken links each
slices = [[], [], []]
loads = [0, 0, 0]
for src, n in sorted_by_load_descending:
    i = loads.index(min(loads))
    slices[i].append(src)
    loads[i] += n
```

Result in one session: 20/18/20 files, 104/103/103 broken links.

## Step 4 — Subagent brief (the safe-enrichment pattern)

Each subagent gets:
- A per-slice JSON manifest (`/tmp/bridge-slice-N.json`) listing source files
  and per-target policy
- A `BRIEF.md` with the operating procedure, the four policies defined,
  and the safety rules

Critical brief rules:
1. **NEVER write to the vault.** Write everything to `/tmp/bridge-slice-N/`.
2. **Each modified file** → `/tmp/bridge-slice-N/<src-basename>.md` (full content)
3. **Each CREATED stub** → `/tmp/bridge-slice-N/stubs/Concepts/<Target>.md`
4. **Final MANIFEST.json** at `/tmp/bridge-slice-N/MANIFEST.json` listing all
   modified files, stubs, rewrites, and skipped items
5. **DO NOT run `python3 wiki/build_index.py`** — the parent does that
6. **DO NOT touch** `wiki/INDEX.json`, `wiki/AMBIGUOUS-LINKS.md`, `wiki/build_index.py`,
   or files in other slices

Stub format (per the policy):
```markdown
---
title: <Target>
status: stub
tags: [<1-3 inferred tags>]
created: <YYYY-MM-DD>
---

# <Target>

> Concept hub · [[Concepts]]

[stub — to be expanded: brief description of what this concept appears to be
based on the links in repo notes that reference it. Cross-link back to the
originating repos under `## Repos using this` once you can name them.]

## Repos using this
```

**No fabrication.** The stub is a placeholder marker so the link resolves.
The user fills in the body later. Subagents that try to "help" by writing
3 paragraphs of definition are over-stepping — the brief explicitly forbids
that.

## Step 5 — Verifying and applying

The parent agent does these, not the subagents:
1. Read each subagent's `MANIFEST.json`
2. Spot-check 3-5 stub files (no invented facts, single-paragraph max)
3. Spot-check 3-5 modified source files (line count delta is reasonable,
   no accidental corruption)
4. Apply with `cp /tmp/bridge-slice-N/<src> /home/arctic/<src>` for source files
5. Apply stubs with `cp /tmp/bridge-slice-N/stubs/Concepts/<X>.md /home/arctic/Concepts/<X>.md`
6. Re-run `python3 wiki/build_index.py` and check `broken_links` dropped
   by ~N where N is the slice's broken-link count

Expected outcome: from 310 broken → 0 broken (or close to it; some
un-stub-able edge cases like `Work/Internship/Dataobserve` may remain).

## Pitfalls

- **False declaration of "0 broken links"**: an `INDEX.json` showing 0
  broken may be stale. Always re-run `python3 wiki/build_index.py` at the
  start of the session. The mtime of the JSON will tell you; if it's older
  than the most recent note edit, it's stale.
- **The "8 in-code blocks" false negative**: a quick check for wikilinks
  inside triple-backtick code blocks can mask real broken links if the
  parser is wrong. The `build_index.py` parser has a known bug where
  `line` numbers in `broken_links` entries are wrong (they're the link
  index, not the line number). Verify by re-finding wikilinks in the
  source file directly with `re.finditer(r'\[\[([^\]]+)\]\]', line)`.
- **Single-source CREATE_STUB stubs** create orphan pages. Don't make
  stubs for things that only one source links to. Either STRIP those
  links or push the threshold to 2+ occurrences.
- **Folder-path link targets** like `[[Hobbies/Cooking/recipies]]` are
  not stub-able. They refer to a folder, not a note. Either:
  - Rewrite to the **bare basename** of the MOC note (`[[recipies]]` or
    `[[recipies|recipies]]`) — relies on `build_index.py:228`'s `by_stem`
    lookup and resolves regardless of where the file lives in the vault.
    **Preferred when the MOC has a unique basename.**
  - OR rewrite to the **full path from vault root** including the filename
    without `.md` (`[[Hobbies/Cooking/recipies/recipies]]` or
    `[[Hobbies/Cooking/recipies/recipies|recipies]]`). **Use only when the
    bare basename would be ambiguous** (e.g. `internship.md` exists at two
    different paths and would resolve to the wrong one).
  - Strip the link.
  - **DO NOT** write `[[Hobbies/Cooking/recipies]]` even though the folder
    exists — the resolver checks `c.lower() == want` (exact basename
    match with `.md`) and `c.lower().endswith("/" + want)` (suffix match),
    and a folder-only target like `cooking/recipies.md` cannot equal the
    file path `cooking/recipies/recipies.md` (different segment count)
    nor end-with `/cooking/recipies.md`. This is the most common cause
    of "the folder and file both exist but the link is still broken".

  **Diagnostic to confirm the bug** (run before patching):
  ```python
  import os
  VAULT = "/home/arctic/Documents/fun"
  SKIP = {".git", ".obsidian", ".claudian", ".claude", "node_modules", "__pycache__"}
  all_files = []
  for root, dirs, files in os.walk(VAULT):
      dirs[:] = [d for d in dirs if d not in SKIP]
      for f in sorted(files):
          all_files.append(os.path.relpath(os.path.join(root, f), VAULT))
  target = "Hobbies/Cooking/recipies"
  want = (target if target.endswith(".md") else target + ".md").lower()
  matches = [c for c in all_files if c.lower() == want] or \
            [c for c in all_files if c.lower().endswith("/" + want)]
  # matches will be [] for folder-only targets — that confirms the bug.
  ```
  Apply the bare-basename fix and re-run `build_index.py --check` to
  verify `broken` dropped by exactly N (one per source × targets fixed).

- **NEVER write folder-only wikilinks in new content either.** The same
  trap catches brand-new notes: it's tempting to write
  `[[Hobbies/Cooking/recipies]]` because "the folder exists," but the
  resolver won't find the MOC. The correct form for *new* content is
  always either the bare basename (`[[recipies]]`) or the full path with
  filename (`[[Hobbies/Cooking/recipies/recipies]]`). When in doubt,
  default to the bare basename — it's the form Obsidian's Quick Switcher
  produces when you copy a link from the file menu.
- **Special-case overrides in the brief trump the slice JSON policy.** When a
  per-target policy in the slice says `CREATE_STUB` but the brief's
  "special cases" section names that target with a redirect rule
  (e.g. `Hobbies/Cooking/recipies` → AUTO_FIX redirect to the existing
  `Hobbies/Cooking/recipies/recipies.md`, or `Work/Internship/Dataobserve`
  → FUZZY_AUTO redirect to the existing `Work/Internship/Dataobserve/Data Observe.md`),
  apply the brief's rule. The implementation pattern: before classifying
  any target, check the brief for a special-cases block AND run
  `search_files pattern='<Target>' target=files path=<vault>` to verify
  whether the named file actually exists. If it does, that target is a
  rewrite (AUTO_FIX or FUZZY_AUTO), never a stub — even when the slice
  says otherwise. Slice JSON is the *default* policy, not the *final*
  policy.
- **FUZZY_AUTO / AUTO_FIX display text = last path component, not the full target.**
  The brief's convention is `[[path/to/existing|Existing]]` where the
  display portion is just the basename/last segment of the broken link.
  For `[[Work/Internship/Dataobserve]]` rewriting to
  `Work/Internship/Data Observe.md`, the rewrite is
  `[[Work/Internship/Data Observe|Dataobserve]]`, NOT
  `[[Work/Internship/Data Observe|Work/Internship/Dataobserve]]`. Naive
  `display = t` breaks this. Same rule for AUTO_FIX with paths like
  `Hobbies/Cooking/recipies/recipies` → display `recipies`, not the
  full path. Strip any `.md` extension from the policy's `arg` to match
  Obsidian's vault convention (e.g. `Concepts/FlashAttention.md` →
  `Concepts/FlashAttention`).
- **Stripping a `- Concepts: [[A]], [[B]], [[C]]` line where ALL targets are
  broken drops the line entirely — don't leave an orphan `- Concepts:` bullet.**
  This is the most common source of "wasted" stub count drift after a
  repair pass. The detection regex: after stripping every `[[X]]` token
  and tidying commas/whitespace, if the line matches `^-\s+\w[\w\s]*:\s*$`
  AND the original had any wikilink AND the new line has none, drop it.
  This keeps the bullet for cases where SOME links survived (e.g. a
  `- Concepts: [[Structured Output]] (e.g. JSON schema)` becomes
  `- Concepts: (e.g. JSON schema)` after a STRIP on Structured Output).
- **Regex strip patterns that survive single-line Lists.** When all 10
  links on a `- Concepts: [[X]], [[Y]], …` line are STRIPs, the line
  collapses cleanly with this four-step pattern: (1)
  `,\s*\[\[X\]\]` → `` (leading-comma form), (2) `\[\[X\]\](,\s*)` → ``
  (trailing-comma form), (3) `^\s*\[\[X\]\]\s*$` → `` (whole-line form),
  (4) `^\s*\[\[X\]\]\s*` and `\s*\[\[X\]\]\s*$` for left/right anchored.
  Then collapse `,\s*,` → `,`, trim leading/trailing commas, collapse
  whitespace, and apply the orphan-bullet detection above. The full
  implementation is in `scripts/strip_wikilinks.py` (CLI: takes slice
  JSON + out-dir, applies all four policies plus the special-case
  overrides, writes `MANIFEST.json`). Use it as a library too:
  `from strip_wikilinks import apply_policies, SPECIAL_CASES`.
- **FUZZY_AUTO promoting wrong targets**: `[[Tool Calling]]` is similar to
  `small-lms-efficient-agentic-tool-calling.md` in normalized form, but
  they describe different things. The classifier should ONLY fuzzy-match
  against CONCEPT notes (paths in `Concepts/` or `wiki/*/concepts/`).
  Paper/repo notes are not valid concept targets.
- **Stub-content fabrication**: the brief says "single paragraph placeholder"
  but a subagent may write 3 paragraphs of confident-sounding definition
  pulled from generic web knowledge. The verifier must catch this:
  compare the stub body to the link texts in the source repo notes —
  if the stub says something the source notes don't claim, it's fabricated.
- **Two wiki-style jumps in one slice**: don't let a subagent create
  `Concepts/X.md` AND `Concepts/X.md` (collision). Pre-check all
  CREATE_STUB targets with `search_files pattern=X target=files path=<vault>`
  before dispatching; collision targets need either AUTO_FIX behavior or
  a renamed stub.
- **The `Concepts.md` MOC doesn't update automatically**. When you create
  ~30 new stubs, `Concepts/Concepts.md` won't list them until you append
  to it. This is a known follow-up — not required to fix broken links,
  but worth doing afterwards so the new stubs are discoverable from the
  MOC.
