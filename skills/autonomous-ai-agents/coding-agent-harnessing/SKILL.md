---
name: coding-agent-harnessing
description: "Orchestrate Codex / Claude Code / OpenCode as sub-agents from Hermes — when to use which, prompt shape, sandbox flags, monitoring long runs, common pitfalls (deprecated flags, sandbox blocks, timeout clamps, output pagination)."
version: 1.0.0
author: Hermes Agent + Aditya Sasidhar
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Codex, Claude-Code, OpenCode, Coding-Agents, Subagent-Orchestration, PTY, Sandbox]
    related_skills: [codex, claude-code, opencode, delegation-first]
---

# Coding-Agent Harnessing

How Hermes dispatches and supervises external coding CLIs as sub-agents.
The parent (Hermes) does **not** write code in-context for this user — coding
goes to Codex / Claude Code / OpenCode. This skill captures the orchestration
layer that the per-CLI skills don't cover end-to-end.

## When to use which agent

| Task shape | Best fit | Why |
|---|---|---|
| Greenfield feature, scratch build, "build me X" | **Codex** (`exec --sandbox workspace-write`) | Strongest at clean implementations; sandbox is safe. |
| Multi-file refactor with invariants, repo-wide reasoning | **Claude Code** (`-p` print mode with `--max-turns`, or interactive tmux) | Best cross-file context. |
| Single-file fix, quick targeted change | **OpenCode** (`run '…'`) | Cheapest / lowest ceremony once configured. |
| PR review (your repo or upstream) | **Codex** (`codex review --base main`) or **OpenCode** (`opencode pr N`) | Both have built-in review commands. |
| Bulk parallel work (N independent fixes/docs sweeps) | **All three in parallel worktrees** | Each in its own workdir, different tasks. |
| Senior deep-dive review (read-only, severity-ranked) | **Codex** with `gpt-5.6-terra` model | Confirmed cost: ~170k tokens / ~6.5 min for a 12-file review. |

For the user's profile, default: **Codex for coding work**, escalate to Claude
Code when the task is cross-file reasoning over an unfamiliar repo.

## Canonical dispatch pattern (Codex — read-only review)

```bash
terminal(
  command='codex exec --sandbox read-only "Senior-engineer deep review of /path. Read every file, run git diff, deliver sections A–I…"',
  workdir='/path/to/project',
  background=true,
  pty=true,
  notify_on_complete=true,
)
```

Returns `session_id`. Then:

```python
# Long runs: poll in a loop (NOT a single big wait — see Pitfall #3)
while True:
    p = process(action="poll", session_id=..., timeout=600)
    if p["status"] != "running":
        break
    # do useful work here

# Get the full transcript in paginated chunks
process(action="log", session_id=..., limit=800, offset=2100)
```

## Canonical dispatch pattern (Codex — coding task)

```bash
terminal(
  command='codex exec --sandbox workspace-write "Fix #N: <title>. <acceptance criteria>. Add unit tests."',
  workdir='/path/to/project',
  background=true,
  pty=true,
  notify_on_complete=true,
)
```

Then verify the diff in the parent (`git diff`, run tests) before reporting done.

## Prompt shape that works

A well-shaped prompt for a coding-agent has:

1. **Goal statement** — what artifact to produce, in one sentence.
2. **Constraints** — what NOT to touch, what style/conventions to follow.
3. **Acceptance criteria** — how you will verify success (test command, file path,
   expected output, specific URL).
4. **For reviews:** explicit section list (A, B, C …) so the report is
   structured and easy to summarize.
5. **Explicit "do not" clauses** — "Do NOT make changes", "Do NOT commit",
   "Do NOT touch file X".
6. **Cost directive when appropriate** — "Cost is not a concern, be thorough"
   unlocks more careful work.

Real example that worked (drone-control-protocol deep review):

```
You are doing a senior-engineer deep review of a large in-progress diff
in this repo. Cost is NOT a concern — be thorough. Do NOT make changes.

Read EVERY file in the working tree and analyze the uncommitted work
end-to-end: …

Then deliver a structured report with:
(A) ARCHITECTURE OVERVIEW …
(B) SAFETY & CORRECTNESS REVIEW …
…
(I) TOP 10 ACTIONABLE NEXT STEPS

Be specific. Quote line numbers. Use bullet points. Be a senior reviewer,
not a cheerleader.
```

## Verification (always in the parent)

Treat agent output as a self-report. Verify:

- **File changes:** `git diff` / `git status` in the parent.
- **Tests:** run the project's test suite, don't trust "tests pass".
- **External side effects:** fetch URLs, check IDs, stat absolute paths.
- **Cost/safety:** if the agent claims a destructive op succeeded, check
  the artifact exists.

For diff-review tasks, the parent reads the **transcript** via
`process(action="log")`, distills into a structured summary, and **only then**
reports to the user.

## Common pitfalls

### Codex — deprecated `--full-auto`

Codex ≥ 0.144 prints:

```
warning: `--full-auto` is deprecated; use `--sandbox workspace-write` instead.
```

Always use the explicit sandbox flag in new prompts:

| Sandbox | Use for |
|---|---|
| `read-only` | review/audit/exploration tasks |
| `workspace-write` | build/fix/refactor (safe default) |
| `danger-full-access` | gateway contexts where bubblewrap is unavailable |

### OpenCode CLI positional argument quirk (2026-07-29)

OpenCode's `opencode run [message..]` treats `-f <file>` as taking the rest of
the command line as another file path. So this:

```
opencode run -f /tmp/brief.md "Implement it"
```

fails with `Error: File not found: Implement it` — the trailing string is
parsed as a SECOND `-f` filename, not as the message.

**Workaround:** put the message BEFORE `-f`:

```
opencode run "Implement /tmp/brief.md" -f /tmp/brief.md --thinking
```

The CLI parses positional args greedily until it hits a recognized flag, so
`-f` MUST come after the message.

### OpenCode — `opencode run "$(cat BRIEF.md)"` does NOT work (2026-08-09)

A tempting follow-up to the `-f` quirk is to pass the brief via shell
substitution: `opencode run "$(cat /tmp/zh-prompt.md)" "build it"`. This
fails silently with ~13s uptime and empty output. Two failure modes compound:

1. Bash expands `$(cat …)` *before* OpenCode sees the command, so the literal
   brief content ends up as the message.
2. OpenCode then parses that message as a single file path, not a prompt —
   produces `Error: File not found: <the entire brief>`.

**Workaround:** write the brief to a file *inside* the workdir (convention:
`.PROMPT.txt`) and reference it by relative path. Combine with the
`external_directory` pitfall below — the brief must live in OpenCode's CWD
or permission will be denied before it can read.

### OpenCode — `external_directory` permission auto-rejects everything outside CWD (2026-08-09)

OpenCode defaults to denying any tool call that reads/writes outside its
working directory — `/tmp/<brief>.md`, sibling project dirs,
`~/.config/opencode/`, all rejected. The error message looks like a
missing-file failure but is actually a permission denial:

```
! permission requested: external_directory (/tmp/zh-prompt.txt); auto-rejecting
✗ Read /tmp/zh-prompt.txt failed
Error: The user rejected permission to use this specific tool call.
```

This bites when you try to point OpenCode at reference material that lives
outside its workdir — config files in `~/.openclaw/`, code samples in
`~/projects/<other>/`, briefs in `/tmp/`.

**Fixes (pick one):**

1. **Add `--auto` to the `opencode run` invocation** — auto-approves
   permissions not explicitly denied. Reasonable default for non-interactive
   one-shot builds where the brief has already been sanity-checked.
2. **Self-contain the brief inside the workdir** — copy or compose the
   reference material into a file inside `cwd`, then point OpenCode at it
   with a relative path. Works without `--auto`, slightly safer.

For this user's pattern (Hermes orchestrates, OpenCode builds in its own
workdir), the self-contain pattern is the better default — `--auto` is
broad enough that an unexpected permission request could do something
undesirable. The canonical dispatch:

```bash
cd /home/arctic/projects/<project>
# inline the brief + any reference snippets you need
cat ../external.md > .PROMPT.txt   # or compose inline
opencode run "Read .PROMPT.txt in your cwd for the full spec, then build it."
# optional: add --auto if you accept the broader permission grant
```

### OpenCode — stale `notify_on_complete` pings look like new failures (2026-08-09)

When multiple `opencode run` dispatches are in flight with
`background=true, notify_on_complete=true`, completion pings arrive in
arbitrary order. A late ping for an *old failed run* (session ID from 10
minutes ago) is NOT a new failure of the current run — only process exit
is signalled, not current state. **Always cross-reference** the ping's
session ID against `process(action="list")` and the active build's
`session_id` before reporting a failure to the user.

### Claude Code print-mode permissions (2026-07-29)

`claude -p "<task>"` does NOT skip the permission layer for `Write` / `Bash`
tool calls — it still demands interactive approval and will exit blocked if
no one is there to approve. Confirmed: with `-p --max-turns 30`, Claude
spent its first turn reading the brief, drafted all three files, then
failed every `Write` call with "requested permissions… but you haven't
granted it yet" and exited with no files on disk.

**Workaround:** add `--permission-mode bypassPermissions` (or
`--dangerously-skip-permissions` for the full auto-approve path) so print-mode
runs actually finish non-interactively. With bypassPermissions the same brief
landed all three files in ~3 minutes.

### Codex — sandbox blocks `rm -rf` / `rm -f`

Inside `workspace-write`, Codex refuses any shell command matching `rm -rf …`
or `rm -f …`:

```
Rejected("`/bin/bash -lc 'rm -rf /tmp/x'` rejected: rm -f style commands are
not permitted. Use a safer approach")
```

**Workaround:** `find … -depth -delete` then `rmdir`. Pass "if the dir
exists…" precondition; don't loop on cleanup.

### Hermes `process wait/poll` timeout clamp

The `process` tool's `wait` and `poll` actions clamp to a **60-second** ceiling:

```
"timeout_note": "Requested wait of 600s was clamped to configured limit of 60s"
```

For long Codex runs (5–10 min is normal), use **`poll` with the full
requested timeout** in a parent loop, NOT a single big wait. The agent
stays in `running` state across calls.

### Output preview vs. full log

`process(action="poll")` returns a short `output_preview` (~100–200 lines).
For the full transcript use `process(action="log", session_id=..., limit=N, offset=M)`.

A 6-min Codex deep review produced **2,297 lines**. Always paginate; never
request everything in one call. Distill to a tight summary for the user.

### Token cost honesty

A "senior engineer deep review" with `gpt-5.6-terra` ran ~6.5 min and used
**170,873 tokens** on a 12-file diff. Plan prompts accordingly:

| Task | Suggested model |
|---|---|
| Narrow review / single-file fix | `gpt-5-mini` (or default) |
| Cross-file reasoning / multi-step plan | `gpt-5.6-terra` |
| "Without worrying about the cost" | `gpt-5.6-terra` + thorough prompt |

### PTY required

Codex and OpenCode are interactive TUI apps — **`pty=true` is required**.
Claude Code's `-p` print mode does NOT need PTY (no interactive dialogs).

### Git repo required

Codex refuses to run outside a git directory. For scratch work:

```bash
cd $(mktemp -d) && git init -q && codex exec "your task"
```

### Codex sandbox in gateway contexts

When the Codex CLI is invoked from a Hermes gateway/service context (e.g.,
Telegram-driven agent sessions), the `workspace-write` sandbox may fail with
bubblewrap / user-namespace errors. In that context, prefer:

```bash
codex exec --sandbox danger-full-access "your task"
```

Use **process boundaries as the safety layer**: explicit `workdir`, clean
git status before launch, narrow task prompts, `git diff` review, targeted
tests, and human/agent confirmation before committing broad changes.

### "Don't auto-fix things in deployed repos" — even small things

If the user says a project is already deployed and "works well", **do not
silently patch things you notice while inspecting it** (a corrupted CSS
block, a deprecated API, a TODO comment, a typo). That is the user's
*production* code.

Concrete signal phrases from this user:
- "its already deplyed and it works well, so dont touch it"
- "leave it, it works"
- "just review, don't fix"

What to do instead:
1. Surface the observation in a single sentence ("I noticed X in file Y").
2. List the **minimum** concrete fixes you'd make.
3. Ask explicitly before touching anything.

If the user pivots the task away (e.g. "just stash any changes"), confirm
`git status --short` is clean before stopping — proves no stray edits
landed and prevents follow-up cleanup.

### "Write this, don't commit it" — write-without-commit dispatches

For blog posts, drafts, and other creative artifacts the user wants to
review personally before committing:

1. Dispatch a write-only Codex task with explicit negative clauses in the
   brief: "Do NOT git add, commit, push, install deps, build, or modify
   any other file. Only create `<path>` and stop."
2. In the parent context, **always** verify after the child reports done:
   - `git status --short` — should show exactly one untracked file
   - `wc -w <path>` — sanity-check word count against the brief
   - `head -15 <path>` — confirm frontmatter and opening prose
   - **`stat -c '%y' <path>` (or equivalent)** — file mtime proves the
     child actually wrote it during this run, not just before launch.
     Critical when the parent session has already touched other files
     in the same worktree (`git status` cannot distinguish pre-existing
     modifications from child-introduced ones).
3. When summarizing for the user, include the verification output, not
   just the child's self-report. Flag 2–3 specific things worth eyeballing
   before commit (e.g. "Codex was conservative on benchmark numbers —
   spot-check the arXiv link").

**Real failure mode this prevents:** `git status --short` shows a file as
`M` — but that `M` was already there before the child launched. The parent
silently accepts the child's claim that it wrote the file, but the file
in question is actually unchanged. `stat -c '%y'` (or comparing mtime to
the child's `started_at`) catches this.

The brief itself should be rich on context the child cannot infer:
- Existing post to match tone of (absolute path)
- Content-collection schema (so frontmatter validates)
- Hero image assets that already exist vs. ones tied to other posts
- Voice/tone notes ("first-principles", "Indian-English phrasings",
  "explain to a smart friend, not academic-stiff")
- Explicit length target in words
- What *not* to fabricate (e.g. "no specific benchmark numbers, no full
  author lists you're unsure of")

Children cannot call `clarify`; if the brief leaves voice ambiguous the
child defaults to generic-academic and the parent has to rewrite.
Spend 5 extra sentences in the brief to save a rewrite.

## See also

- `codex` — Codex-CLI specifics (per-bundled skill).
- `claude-code` — Claude Code CLI specifics (per-bundled skill).
- `opencode` — OpenCode CLI specifics (per-bundled skill).
- `delegation-first` — General sub-agent orchestration (verify sub-claims,
  don't trust summaries blindly).

## References

- `references/opencode-reasoning-variants.md` — OpenCode `--thinking` / `--variant` flags, per-model `options` in `~/.config/opencode/opencode.jsonc`, named `variants` and `variant_cycle` keybind, plus the web-UI reasoning-effort display quirk (issue #17588).
