# Deep enrichment of repo notes (beyond the first-pass 250-word section)

This is the **second-pass** play. The existing `project-notes-enrichment.md`
covers adding a `## Inside the Codebase` section (~250 words) to stub notes.
This file covers the **deep-enrichment** variant: taking a vault where notes
already have ~1.5k chars of detail and bringing them to **5–12k chars** with
a full 11-section schema, real tech stack, project status, use cases, and
cross-links.

Built from a session that took 50 GitHub-repo notes from 1.5k → 15.3k chars
average (10× denser) using 50 delegated subagents dispatched in 11 waves.

## When to use this playbook

Use the **deep-enrichment** pattern (this file) when:

- The user says "enrich the content **even more**", "add the tech stack I
  used", "what the project is about", "go in great detail about every
  aspect", "add Status", "add Use cases", or phrases implying more depth.
- The existing notes already have `## Inside the Codebase` with ~1.5k chars
  of accurate detail (don't lose it — DEEPEN it, don't replace it).
- The repo has a non-trivial code surface: real `src/` directory, real
  config, multiple files worth describing.

Use the **first-pass** pattern (`project-notes-enrichment.md`) when:

- Existing notes are stubs (<500 chars) or have no `## Inside the Codebase`.
- The user says "write a basic enrichment" or "add a quick description".

**Don't re-run the first-pass pattern on already-enriched notes** — that
would overwrite real detail with shorter content. The right call is: read
first, identify what's already good, then DEEPEN.

## The 11-section deep-enriched schema

Every deep-enriched note has this exact structure, in order:

```markdown
---
<repo metadata + tags + extended description from gh API>
---
# <Display Name>
> <one-line tagline blockquote>

## Overview
2–4 sentences: what the project is, who it's for, what makes it interesting.

## Status
1–3 lines: active/exp/prototype, last-pushed, deps pinned?, tests?, CI?,
stars/forks. Be honest — "single-push snapshot", "no test suite", "no CI".

## Inside the Codebase                <-- the deepest section
### Architecture / How it works
### File structure                    <-- every meaningful dir + 1-line purpose
### Key files                          <-- 1–2 line descriptions
### Notable details                   <-- gotchas, design choices, hidden gems
### Tech stack                         <-- concrete libs/versions
### How to run it                      <-- real commands

## What this project is about
1 paragraph: problem, motivation, why does this exist.

## Use cases / When you'd reach for this
2–5 realistic scenarios.

## Key Features
Bullets with emoji lead-ins (🔁 🐚 🧠 🔌 🎚️ ...).

## Getting Started
Real run commands (uv / pip / npm / cargo depending on the repo).

## Stats
Mirror GitHub metadata: stars, forks, visibility, dates, category, status.

## Links
- GitHub: <url>
- Part of: [[Public Repos]]    <-- MUST preserve exactly
- Concepts: [[C1]], [[C2]], ... <-- 4–6 concepts the repo actually uses
- Themes: [[T1]], [[T2]]        <-- 1–2 themes from Themes/ folder
- Related: [[other-repo-slug]]   <-- 1–3 sibling repos
```

## Target output size

- **5–10k chars** for a "normal" repo (typical learning project, prototype,
  or app).
- **7–12k chars** for repos that already have decent detail (deepen, don't
  rewrite). These are the higher-quality existing notes.
- **Never go below 5k chars** — that's the floor for a "deep" note.
- **Upper bound ~15k chars** — beyond that you're padding.

## The subagent brief template

This is the contract you ship to every leaf subagent. Copy it verbatim into
a `/tmp/enrich-wave<N>/BRIEF.md` so each subagent can read it before starting.

```markdown
# DEEP ENRICHMENT BRIEF — for subagents

You are one of N parallel subagents enriching an Obsidian vault note about a
single GitHub repo owned by <owner>. Your job: produce a MUCH DETAILEDER
version of the existing note and write it to a TEMP FILE ONLY. Never touch the
real vault note. The parent agent will verify and apply.

## Inputs you have
1. Existing note at <vault>/Github/Repos/<slug>.md — read it to learn what's
   already said and what schema is in use.
2. Cloned repo at /home/<user>/projects/<slug>/ — explore freely.
3. GitHub API: `gh api repos/<owner>/<slug>` (full JSON, parse with Python).
   Don't use `--jq` object constructors.
4. README: `gh api repos/<owner>/<slug>/readme --jq .content | base64 -d` —
   but ONLY if the JSON has a `.content` field. If `gh` prints 404 JSON,
   the repo has no README; fall back to the existing note's description.

## Target schema
[full 11-section schema above]

## Hard rules
- **No fabrication.** Every claim must trace to (a) README, (b) actual
  code you read, or (c) GitHub API. If you can't find the answer, write
  "Not specified" or "Unknown" — do NOT invent versions, features, or
  commands.
- **Preserve existing good prose.** If the existing note already says
  something accurate, keep it / rewrite it crisply. Don't lose real detail.
- **Don't downgrade.** If the existing note says X and your code-reading
  disagrees, trust the CODE. The user can correct later.
- **Wikilink exact names.** `[[LLMs and SLMs]]` (with spaces) for the
  theme file. For concepts, use the bare name (e.g. `[[RoPE]]`).
- **Strip markdown noise from README.** Badges, raw HTML, images, link-only
  references — drop them, keep the prose.
- **Comment-only code blocks** (≥half lines are `# comments`) are NOT
  Getting Started commands. Skip them.

## Output
Write the full enriched note to:
  /tmp/enrich-wave<n>/<slug>.md

After writing, print:
- The path
- The byte count of the new file
- A 3-line summary of what you enriched

## Things you CANNOT do
- Touch <vault>/Github/Repos/<slug>.md
- Spawn further subagents (you are a leaf)
- Edit anything outside /tmp/enrich-wave<n>/<slug>.md
- Use `git commit` or `git push`
```

## Wave dispatch + wait + apply loop

Use this Python pattern from `execute_code` to keep multiple waves moving
autonomously. Critical: `execute_code` has a **5-minute (300s) timeout**, so
the loop budget must be ≤ 280s of waiting — plan around that.

```python
import os, time

transcripts_dir = "/home/arctic/.hermes/cache/delegation/live/<deleg_id>"
start = time.time()
while time.time() - start < 280:  # leave 20s headroom
    files = [f for f in os.listdir(f"/tmp/enrich-wave{n}")
             if f != "BRIEF.md" and f.endswith(".md")]
    active = 0
    if os.path.exists(transcripts_dir):
        for tf in os.listdir(transcripts_dir):
            if not tf.startswith("task-"): continue
            p = os.path.join(transcripts_dir, tf)
            if time.time() - os.path.getmtime(p) < 90:
                active += 1
    print(f"[{time.strftime('%H:%M:%S')}] {len(files)}/N files, {active} active, {int(time.time()-start)}s")
    if len(files) >= N and active == 0:
        break
    time.sleep(20)
```

### Pitfall: file-mtime is the only reliable signal

Subagent "active" status is invisible to `process list`. The only way to
know if work is still happening is to watch the **transcript log file mtime**
in `/home/arctic/.hermes/cache/delegation/live/<deleg_id>/task-*.log`. Use
the 90-second threshold: if no transcript line in the last 90s, treat the
agent as idle.

The bad case to watch for: all 4 files exist and are correct, but the
transcript log keeps updating (the agent is doing extra polish work). Don't
wait for `active == 0` if all files are written — apply and move on.

### Pitfall: execute_code timeout vs terminal timeout

- `execute_code`: 5-min hard limit (300s). Don't try to wait longer.
- `terminal` (foreground): 10-min hard limit (600s).
- `terminal` (background=true): unlimited, but you must poll.

For waves that exceed 5 minutes (e.g. 4 agents on richer repos), use
`terminal` with the looping pattern, NOT `execute_code`. The terminal
pattern is uglier but doesn't kill the loop.

## Verification gate (parent-side, before applying)

After each wave, **before** applying the temp files to the vault, run this
check:

```bash
for f in /tmp/enrich-wave<n>/*.md; do
  [ "$(basename $f)" = "BRIEF.md" ] && continue
  has_fm=$(head -1 "$f" | grep -c "^---")
  has_part=$(grep -c "Part of:" "$f")
  has_inside=$(grep -c "^## Inside the Codebase" "$f")
  has_overview=$(grep -c "^## Overview" "$f")
  has_status=$(grep -c "^## Status" "$f")
  has_about=$(grep -c "^## What this project is about" "$f")
  has_features=$(grep -c "^## Key Features" "$f")
  has_stats=$(grep -c "^## Stats" "$f")
  has_links=$(grep -c "^## Links" "$f")
  has_concepts=$(grep -c "Concepts:" "$f")
  has_themes=$(grep -c "Themes:" "$f")
  printf "%-50s fm=%s part=%s inside=%s overview=%s status=%s about=%s feat=%s stats=%s links=%s concepts=%s themes=%s\n" "$(basename $f)" "$has_fm" "$has_part" "$has_inside" "$has_overview" "$has_status" "$has_about" "$has_features" "$has_stats" "$has_links" "$has_concepts" "$has_themes"
done
```

A skip-safe note must have ALL flags = 1. If any note is missing a flag,
**don't apply it** — investigate the transcript log first.

## Apply step

```bash
for f in /tmp/enrich-wave<n>/*.md; do
  [ "$(basename $f)" = "BRIEF.md" ] && continue
  slug=$(basename "$f" .md)
  target="/home/arctic/Documents/fun/Github/Repos/${slug}.md"
  [ -f "$target" ] && cp "$f" "$target" && echo "  $slug: $(wc -c < $target) bytes"
done
```

Always copy with `cp` (not `mv`) — the temp file stays around as a backup
in case you need to re-apply or diff.

## DEEPEN, don't rewrite — for already-good notes

If the existing note already has decent detail (≥1.5k chars inside `##
Inside the Codebase` with real architecture/file/notes content), the brief
must say so explicitly:

> DEEPEN the existing note. Preserve the good prose. Add more depth on: tech
> stack (real versions from pyproject.toml), use cases (3–5), getting started
> (real commands), status (active/exp? deps pinned? tests?). Target 7–12k chars.

Without this explicit signal, agents will rewrite from scratch and lose the
existing detail. When the recursive-babylm note's existing SparseMQA +
LightningIndexer prose was preserved perfectly, it was because the brief
said "deepen" — apply this pattern consistently.

## Pitfalls (session-specific, learned the hard way)

- **Brief as a path, not inline.** Subagents receiving a 7k-char brief
  inline in `context` work fine, but if the brief lives at
  `/tmp/enrich-wave<n>/BRIEF.md` they can re-read it after losing the
  context entry — much more robust. Always write the brief to disk first.
- **`path` AND `BRIEF.md` must BOTH exist before dispatch.** If the dir
  doesn't exist, the write fails. **Even worse**: if the dir exists but
  `BRIEF.md` is missing, the agent silently reconstructs the schema from
  the existing note + task instructions — which usually works but means
  the agent didn't follow the brief you wrote. Always do this upfront,
  one command, before dispatching wave 1:

  ```bash
  for n in 1 2 3 4 5 6 7 8 9 10 11; do
    mkdir -p /tmp/enrich-wave$n
    cp /tmp/enrich-wave1/BRIEF.md /tmp/enrich-wave$n/BRIEF.md
  done
  ```

  This single command prevents the recurring "wave N has empty dir" or
  "wave N missing BRIEF.md" mistake that surfaced twice in the 50-repo
  session. Don't trust the dispatch tool to create the dir.

- **Even when the brief is missing, agents self-recover.** The
  RealTimeColorDetector agent (in a wave where the brief file was missing)
  read the existing note structure, inferred the 11-section schema from
  task instructions, and produced a 25k-char output. This is robust but
  not what you want — the agent followed the task description, not your
  brief. Always do the upfront copy. The fallback exists; don't rely on it.
- **Set wave-N caps to 4, not 10.** A first wave of 10 agents works but
  makes the per-wave wait long. **4 agents per wave** is the right cap for
  this user — keeps the wait loop under 5 minutes and the per-agent token
  budget generous. The first wave can be larger if you want to front-load
  the work, but keep subsequent waves to 4.
- **Don't trust `final |` in transcript logs.** A `final` line in the
  task log means the agent finished its summary, but the parent's view of
  the file may lag. The **file-mtime** signal is the source of truth.
- **The verification regex `## Status` is exact.** A note that has
  `**Status:** active` inline will pass a loose grep but fail the strict
  `^## Status$` check. Always grep for the exact line, not the substring.
- **One file usually slips.** In sessions of 50+ subagents, expect 1–2
  notes to be missing a section (typically `Concepts:` or `Status`). Fix
  them with a 1-line `patch` after the wave applies — don't re-dispatch.

## Size targets vs reality

In the 50-repo session:
- Smallest: 9.6k chars (single-function project, minimal source)
- Median: 13.4k chars
- Largest: 31.9k chars (texed — large LaTeX tool with many features)
- Mean: 15.3k chars (10× the original 1.5k average)

Wave-per-agent cost: ~3-4 minutes per agent on a complex repo, ~2 min on
a simple one. Total wall-time for 50 repos across 11 waves: ~80 minutes.
Tokens: ~3.8k per agent (input brief + cloned code) + ~12k output = ~16k
tokens per agent × 50 = ~800k tokens, but agent overhead (skill loads,
tool calls, retries) brings actual usage to ~3-5M tokens for the full
session.

## When the user says "make it even deeper"

If after a deep-enrichment pass the user says "go even deeper on X" or
"add more detail on Y", the right move is a **targeted re-dispatch**,
not a full redo. Pick the 5–10 weakest notes by current size and
re-dispatch only those with a brief that says "deepen further on X". Re-
running the full 50 takes 80 minutes and burns ~3M tokens for marginal
gain when the bottlenecks are 5–10 specific notes.

## Diagnose before burning tokens

The user's first instinct is often "just dispatch 50 subagents and burn
10M tokens, what's the worst that could happen?". The right move is a
**cheap diagnostic first** to check whether the work is already done:

1. **Confirm the work isn't already done.** Before dispatching, scan the
   existing notes — count how many already have `## Inside the Codebase`,
   measure the inside-section sizes, list the notes that lack `Concepts:`
   or `Themes:` lines. If the skill's first-pass pattern has already run
   (50/50 notes have `## Inside the Codebase`, themes exist, 200+ wikilinks
   wired), re-running burns tokens to recreate work the vault already has.
2. **Surface the trade-off honestly.** "All 50 notes already have
   `## Inside the Codebase`. Re-running would regenerate near-identical
   output. The real value is going deeper (1.5k → 10k+ chars per note), not
   starting over." Then offer choices: stop, map a different area (papers?),
   refactor the graph, or proceed with deep-enrichment.
3. **The user almost always picks "make it deeper" once you frame it
   right.** That's the deep-enrichment playbook above. Run the diagnostic
   *before* dispatching — never after burning 10 minutes on it.

This pattern saved the 50-repo session from a near-miss disaster: the user
asked for 10M tokens of churn, I ran the diagnostic in 30 seconds, found
the work was mostly done, and pivoted to deep-enrichment which produced
real value (10× density gain + real bug discovery).

## Real bug discovery as a side effect

Deep-enrichment by code-reading isn't just about better descriptions — it
**surfaces real bugs the user didn't know about**. From the 50-repo
session, subagents independently reported:

- **nest** — `agent.py` broken: missing `if __name__ == "__main__"` guard,
  undeclared `groq`/`anthropic` deps, broken `model`/`ollama_client` globals.
- **gpt-based-miniature-python-code-completion-model** — stale
  `model_architecture.txt` says 1 layer / 2 heads but code is 12 / 16 / 512.
  `super.py` has non-divisible `dim=265` with 6 heads. Broken `<EOS>` handling.
- **Dynamic-Depth-Recurrence** — FA3-vs-SDPA divergence between
  `dynamic_dense/model.py` and `training/train_dynamic.py`. Per-token-KL
  router-collapse known issue.
- **recursive-babylm** — fla #640, nvidia-nvvm PTX-version pin, the
  mb=16/-0.0247 val-loss weighting bug, per-pass activation-RMS hook.
- **Trojanix** — hardcoded `"C:"` drive, no committed `requirements.txt` (only
  `venv/` site-packages), `LabelEncoder().fit_transform` per-column anti-pattern,
  duplicated `method`/`method_encoded` columns (0.99 correlation).
- **Caveman** — completely re-categorized: not a low-level interpreter, but a
  men's boxer-shorts e-commerce brand site.
- **insurance_chatbot** — API key persisted in `localStorage`, mocked webhook
  with commented-out fetch, three-way brand-name mismatch.
- **RealTimeColorDetector** — empty `requirements.txt`, missing LICENSE,
  wrong clone URL in README, dead `config.py` and `hsv_mask_red()`.

When a user says "enrich my repos", they're often really asking "tell me
what's wrong with my code". Frame the deep-enrichment deliverable as a
**code review surface**, not just a documentation upgrade. The bugs above
weren't found because the user asked for a code review — they surfaced
naturally because a subagent reading every file looking for "tech stack"
details inevitably trips over the things that don't add up.

When summarizing the wave results back to the user, **lead with the bug
discoveries**. The user got real value from "your nest agent.py is broken
because..." not from "we enriched 50 notes".

## Phantom-repo handling

When a subagent's brief references a cloned repo at
`/home/<user>/projects/<slug>/` but the dir doesn't exist, the agent
**must not fabricate** the repo. The right move:

1. **Check `git remote -v` in the parent.** The same GitHub repo can have
   multiple working trees (e.g. `adityasasidhar.github.io` and
   `the-deep-field` both push to the same GitHub repo via different
   working trees). The remote will tell you which working tree is the
   "real" one.
2. **Probe the parent's memory + `CLAUDE.md` / `AGENTS.md`.** The vault
   `CLAUDE.md` often notes "X is the canonical working tree, Y is stale".
   Memory likewise.
3. **Describe the real working tree, not the phantom path.** The
   adityasasidhar.github.io agent correctly read
   `/home/arctic/projects/website/the-deep-field/` (the active Astro blog)
   and noted that the path in the brief was incorrect. The vault note now
   accurately describes the real working tree.

Fabricating a fake repo from a README + gh API alone is the cardinal sin
of vault enrichment — every claim must trace to code you actually read. The
phantom-repo pattern is the test case: when the obvious code-source path
doesn't exist, fall back to the real working tree, not to fabrication.

## Leaf BRIEF delivery (concept-notes, not repos)

This playbook was originally written for the **50-repo GitHub deep-enrichment
session** (parallel leaf subagents, brief at `/tmp/enrich-wave<n>/BRIEF.md`,
target `/tmp/enrich-wave<n>/<slug>.md`, GitHub-API sources). The user has a
**second leaf BRIEF template** at
`/home/arctic/projects/aiml-burn-tmp/BRIEF-TEMPLATE.md` for deep-enriching
*concept* notes (one subagent, one note, temp at
`/home/arctic/projects/aiml-burn-tmp/wave<N>/<basename>.md`, sources are
vault `raw/articles/*.md` + `Research/Papers/*`). Both delivery modes share
the 11-section schema and the verify-before-apply philosophy, but differ in
five ways that bite agents who only know the repo pattern.

### 1. The 11-section schema is BRIEF-defined, not playbook-defined

The leaf BRIEF specifies the section list explicitly:

> Overview, History/Motivation, Mechanism, Math, Implementation, Variants,
> Use cases, Trade-offs, Connections, Open questions, References.

This is **concept-flavoured** (Math, Mechanism, Variants, Connections) — it
differs from this playbook's repo-flavoured schema (Status, Tech stack, How
to run, Stats). Always follow the section list in the BRIEF you were
given; the playbook's H2 schema above is the *default* for repo notes, not
a universal template. An agent reading only this playbook would default to
Status / Inside the Codebase / Stats / Getting Started sections and ship
the wrong shape for a concept BRIEF.

### 2. The PASS line is a contract — read the file back, don't estimate

The leaf BRIEF mandates the LAST line is exactly:

```
PASS: <bytes> bytes, <count> outbound links
```

Two failure modes that wasted tool calls in a 2026-07-29 leaf run:

- **Mental estimation.** Writing `PASS: 9612 bytes` when the file is
  actually 15771 bytes. The byte count is **the byte count of the file
  you just wrote** — read it back. The reliable pattern is:

  ```python
  import os
  size = os.path.getsize('/path/to/waveN/<basename>.md')
  ```

  then use `size` (not a guess) in the PASS line. Verify before final
  emit: `python3 -c "import os; print(os.path.getsize('...'))"`.

- **Counting the wrong thing for outbound links.** The BRIEF asks for
  "outbound wikilinks", which is the count of **unique outbound wikilink
  targets** in body content — not the total number of `[[...]]`
  occurrences, and not links in the frontmatter `sources:` / `tags:` /
  etc. lines. A reliable count:

  ```python
  import re
  text = open(path).read()
  targets = set(re.findall(r'\[\[([^\]|]+)\]\]', text))
  ```

  Note: per the leaf BRIEF, citation markers like `^[raw/articles/x.md]`
  are NOT wikilinks and do not count.

- **Fixpoint convergence for tightly-budgeted trims.** When the file
  is over-budget by a small amount (≤500 bytes) and a full
  "trim → write → `wc -c` → patch PASS" round-trip per iteration is
  too expensive, write the *entire* file in Python with a placeholder
  PASS line of fixed length, then iteratively converge:

  ```python
  PASS_TPL = "PASS: {} bytes, {} outbound links"
  total = len(content_bytes) + len(("\n" + PASS_TPL.format(0, 0) + "\n").encode("utf-8"))
  for _ in range(5):
      pl = PASS_TPL.format(total, n_links)
      total = len(content_bytes) + len(("\n" + pl + "\n").encode("utf-8"))
  final = content_bytes + ("\n" + pl + "\n").encode("utf-8")
  ```

  The fixpoint converges in 1–2 iterations because the byte delta from
  digit-count changes (`10069` → `9949`) is at most ±1. Combine with
  unique-link counting via `set(re.findall(r'\[\[([^\]|]+)\]\]', text))`
  so updating the link count doesn't reintroduce the same byte drift
  you're trying to converge. A 2026-07-29 leaf run that initially
  trimmed across 6 separate `write_file` calls (each guessing at the
  PASS value) landed in range on the *seventh* call after switching to
  this fixpoint pattern — a single write per remaining trim step.

### 3. Cross-vault bridge wikilinks are forbidden in body content

The leaf BRIEF explicitly forbids a class of wikilinks that *do* resolve
under `build_index.py --check` but cross out of the aiml wiki into the
personal vault's concept hubs. Common forbidden patterns:

- `[[Concepts/OCR|OCR]]`
- `[[Concepts/Native-Resolution Vision|Native-Resolution Vision]]`
- `[[Concepts/Vision-Language Model|Vision-Language Model]]`
- `[[Concepts/Multimodal|Multimodal]]`, `[[Concepts/KV Cache|KV Cache]]`,
  `[[Concepts/GRPO|GRPO]]` (other vault concept hubs the brief may list)

The agent's job is to **describe these concepts in prose**, not link them.
A 2026-07-29 leaf run caught this: the existing note had
`[[Concepts/OCR|OCR]]` in the "Related" section; rewriting that section as
prose preserved the information without violating the cross-vault rule.
The rule exists because a future agent maintaining the aiml wiki shouldn't
depend on resolving into a different vault's namespace.

### 4. `build_index.py --check`: `broken=0` is the gate, `ambiguous>0` is fine

The check reports five counts: `notes`, `stubs`, `assets`, `broken`,
`ambiguous`, `dup_stems`, `orphans`. For a leaf BRIEF delivery:

- **`broken` must be 0** for *new* outbound links (and stays at the
  pre-edit baseline for any pre-existing broken links). The leaf BRIEF
  says: "If a target doesn't resolve, add a `## Broken links in this
  note` footer flagging it."
- **`ambiguous` is informational, not a blocker.** A wikilink like
  `[[flashattention]]` that resolves to `raw/articles/flashattention.md`
  (with no `concepts/flashattention.md`) is ambiguous-but-resolving and
  is acceptable. The 154 ambiguous links in the aiml wiki are *intentional*
  duplicates per the `AMBIGUOUS-LINKS.md` policy — see
  `references/bridging-parallel-systems.md`.
- **`unknown_tags` and `deprecated_tags`** warnings are also non-blocking
  for delivery; the parent agent surfaces them but the leaf agent's
  deliverable is the enriched file.

### 5. Thin-source honesty: don't fabricate to fill the schema

When a leaf BRIEF targets a concept note whose only source is a thin
paper note (e.g. a `Research/Papers/X/x.md` with only 30 lines stating the
framing but no encoder dimensions, token counts, or training data), the
agent must NOT invent specifics to fill the `Implementation` / `Variants` /
`Math` sections. The right moves:

- In **Math**: state the framing in information-theoretic terms (e.g. an
  upper bound on token count from $H(X \mid Y)$) without inventing a
  specific loss or equation from the paper.
- In **Implementation**: list the *categories* of components (backbone,
  vision encoder, resolution handling, training) and explicitly flag
  "concrete numbers should be verified against the paper before reuse".
  Cite the brief source for the framing and stop there.
- In **Variants**: name variants that are evident from the *cross-link
  graph* (e.g. the alternative native-resolution design is named in a
  sibling note) and describe them in 1–2 sentences each.
- In **Open questions**: enumerate what's NOT in the source ("Where is
  the knee of the compression curve?" — the source doesn't record it) as
  a research question, not as a fabricated answer.

This is the inverse of the repo-enrichment rule "the code is the truth".
For paper-note enrichment, the *only* truth is what's in the source note;
beyond that, the agent must surface uncertainty rather than paper over it.

### 6. "Original (preserved)" footer is rare for leaf BRIEFs

The BRIEF says preserve original prose inside a `## Original (preserved)`
section "if the existing note has content you can't reconcile with the
11-section schema". For a clean concept-note leaf BRIEF where the existing
note is a 1–2 KB overview that maps cleanly onto the schema (Overview +
History/Motivation + Variants + References), the right move is **DEEPEN,
not preserve-and-append** — fold the original prose into the new sections
and drop the original verbatim section. The leaf BRIEF's "preserve" rule
triggers only when content doesn't fit the schema (rare for concept notes;
common for cross-vault bridges where the original points at a vault hub
the leaf BRIEF forbids).

### 7. Size-budget discipline: write to the ceiling, not past it

The leaf BRIEF specifies a tight **6,000–10,000 byte** target (not
characters — bytes; see §9). The trap on a leaf agent's first write is
to dump a complete, source-faithful draft at the natural depth of the
material — for a paper with a real algorithm and real math, that's
10k–13k bytes of substantive prose. If the first write lands at, say,
13.5k bytes, the agent then enters a **trimming loop**: many small `patch`
calls, each shaving 6–100 bytes of prose. In a 2026-07-29 leaf run on a
Muon optimizer note, the first `write_file` produced 13,484 bytes; six
trimming patches later it was at 10,425 — still over budget — and the
agent hit the tool-call iteration cap before converging in range.

The fix is to **size the initial write against the ceiling**. Two concrete
techniques:

- **Pre-budget each section.** A 10,000-byte ceiling with 11 sections
  averages ~900 bytes/section, but the actual distribution is uneven
  (Math + Mechanism + Connections carry the weight; Overview + Variants +
  References are short). Aim the longest section at ~1.8k bytes and the
  shortest at ~250 bytes, and pre-trim the draft before writing.
- **Write to the ceiling, not past it.** When the source material
  supports 13k bytes of good prose, *cut material to fit the ceiling*,
  not "write 13k and trim 3k". The difference between a 9.5k file
  written carefully and a 13k file that needs six trimming patches is
  six tool calls and 30k+ extra tokens burned in patch-tool overhead,
  plus a real risk of running out of tool-call budget.

### 8. PASS line is read-only after the final write — emit it last

Per §2, the PASS line must match `wc -c` of the file. A subtler trap:
the PASS line gets **stale on every subsequent edit**. In the same
2026-07-29 leaf run, the agent wrote a first draft with
`PASS: 9924 bytes, 8 outbound links` (a mental estimate from a draft
read-through), then trimmed the file across many patches — but never
updated the PASS line until the very end. The PASS line was wrong for
every intermediate file state. Fix: write the file once with the PASS
line as a sentinel literal (e.g. `PASS: TBD`), then after the final
trim, run `wc -c` and `grep -oE '\[\[...]]'` and patch the literal to
the real values in one targeted edit at the end. **Never estimate the
PASS line — it must come from the file's own byte/link count.**

**`PASS: 0 bytes, 0 outbound links` is a known bug pattern.** If a leaf
agent drafts the body first and intends to fill the PASS line at the
end, the temptation is to write a placeholder like
`PASS: 0 bytes, 0 outbound links` while drafting. In a 2026-07-29 run
on the `rl-verifiable-rewards` aiml-wiki enrichment, an agent did
exactly this, then ran a six-patch trimming loop to bring the file
down toward the 10k ceiling, hit the tool-call iteration cap mid-trim,
and shipped the file with the placeholder still in place — a
deliverable with no PASS contract at all. The parent had to
re-measure and patch the line itself. Two consequences:

- **Never write a numeric PASS placeholder.** Use a non-numeric
  sentinel (`PASS: TBD` or `PASS: WRITE_LAST`) so a stray grep for
  `0 bytes` flags it as un-replaced.
- **Treat any literal `PASS: 0 bytes` in a delivered file as a hard
  bug.** The parent should refuse to apply and either patch the line
  directly or dispatch a follow-up trim. A file without a real PASS
  line is no different from one that fails `wc -c` self-consistency.

### 9. "Bytes" means bytes (not chars) for the leaf BRIEF ceiling

The leaf BRIEF says "6,000–10,000 bytes" explicitly. For pure ASCII the
distinction is moot, but a draft heavy in LaTeX (`$\mathbb{R}^{m \times
n}$`), em-dashes (`—`), Greek letters (`$\lambda$`, `$\eta$`), or
non-ASCII quote marks (`'`) shifts the byte/char ratio. A 10,000-char
estimate can land at 10,500–11,000 bytes when the prose has dense LaTeX
(every `$` and `^` and `\top` is multi-byte UTF-8). The reliable check
is `wc -c` (bytes), not `len(text)` (chars). The size-budget discipline
in §7 should budget against `wc -c` from the first write, not against
char count.
