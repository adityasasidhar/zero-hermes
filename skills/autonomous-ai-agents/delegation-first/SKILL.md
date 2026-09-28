---
name: delegation-first
description: "Use whenever a task contains independent reasoning-heavy subtasks or can be productively decomposed. Dispatch Hermes subagents early and in parallel, while keeping direct mechanical work and final verification in the parent agent."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Delegation, Subagents, Parallelism, Orchestration]
    related_skills: [claude-code, codex, opencode]
---

# Delegation First

## Overview

Default to dispatching Hermes subagents whenever they can make useful independent progress. Do not reserve delegation only for very large jobs: use it for research branches, competing hypotheses, code review, repo inspection, drafting alternatives, and separable implementation work.

The parent agent remains responsible for decomposition, integration, verification, and the final user-facing answer. Subagent summaries are evidence to inspect, not claims to repeat blindly.

## When to Use

Dispatch one or more subagents when any of these apply:

- Two or more subtasks can proceed independently.
- A reasoning-heavy investigation would flood the parent context.
- Multiple hypotheses, implementations, sources, or review perspectives would improve quality.
- A repo-level task can be split by subsystem, concern, or read-only inspection area.
- Bulk work can be partitioned by repository, note, paper, file group, issue, or dataset shard.
- The parent can continue useful work while a child investigates in the background.

Do not delegate when:

- The whole task is a single mechanical tool call.
- The work requires interactive clarification from the user.
- A tight sequential dependency means the child cannot start yet.
- The task is a simple edit whose delegation overhead exceeds the work.
- Durable execution must outlive the current session; use a cron job or tracked background process instead.

## Workflow

1. **Decompose immediately.** Identify independent reasoning units before beginning serial work. Completion criterion: every useful parallel branch is either assigned to a subagent or explicitly kept in the parent because it is mechanical, interactive, or sequential.

2. **Dispatch early.** Use `delegate_task` as soon as each child has enough context. For multiple independent branches, send one batch call rather than serial delegations. Completion criterion: subagents are running while the parent continues non-overlapping work.

3. **Write self-contained briefs.** Include the exact goal, paths, constraints, error messages, expected language/tone, and required output. State that the child has no conversation memory. For external side effects, require a verifiable handle such as an absolute path, URL, ID, HTTP status, or commit diff. Completion criterion: each child can execute without asking the user or guessing missing context.

4. **Avoid overlapping writes.** Prefer read-only research/review tasks. If children must edit, assign isolated files, directories, branches, or worktrees. Completion criterion: no two agents can race on the same mutable artifact.

5. **Keep working in the parent.** Do not poll or idle waiting for background delegation. Handle orchestration, direct tool calls, integration preparation, or another independent branch. Completion criterion: parent time is used productively until results return.

6. **Verify before reporting.** Treat child output as an untrusted self-report. Read changed files, inspect diffs, run tests, fetch URLs, or check IDs yourself. Completion criterion: every user-facing claim about an artifact or side effect is backed by direct parent verification.

7. **Synthesize, do not concatenate.** Reconcile disagreements, remove duplication, and present one coherent result. Completion criterion: the final answer reflects integrated judgment and clearly marks unresolved uncertainty.

## Decomposition Patterns

| Task | Useful dispatch |
|---|---|
| Debugging | One child traces the failing path; another inspects tests/history; parent reproduces and integrates. |
| Code change | One child inspects architecture; another designs tests; parent implements or verifies isolated implementation work. |
| Research | Split by source family, hypothesis, time period, or competing interpretation. |
| Code review | Split into correctness, security, performance, and test-coverage reviews. |
| Bulk notes/repos | Partition non-overlapping groups across several children, then validate schema and links centrally. |
| Planning | Ask children for architecture, risks, and test strategy independently; parent merges into one plan. |

## Common Pitfalls

1. **Delegating vague goals.** “Look into this” produces shallow output. Give paths, constraints, and a concrete deliverable.
2. **Serial delegation.** If branches are independent, batch them in one `delegate_task` call.
3. **Waiting for children.** Delegation is background work; continue in the parent rather than polling.
4. **Concurrent edits to one file.** Use read-only assignments or isolated worktrees/file ownership.
5. **Trusting success claims.** Verify paths, diffs, commands, tests, URLs, IDs, and remote state directly.
6. **Delegating user interaction.** Children cannot call `clarify`; resolve ambiguity in the parent first.
7. **Recursive or durable work.** Leaf children cannot delegate, and background delegation dies with the parent session. Choose the correct execution mechanism.
8. **Delegation for its own sake.** “Whenever possible” means whenever useful progress can be isolated—not spawning a child for a trivial call.

## Verification Checklist

- [ ] Independent reasoning branches were dispatched early.
- [ ] Parallel branches were batched rather than launched serially.
- [ ] Each brief was self-contained and included paths, constraints, and output requirements.
- [ ] Write scopes did not overlap.
- [ ] The parent continued useful work while children ran.
- [ ] All artifact and side-effect claims were independently verified.
- [ ] Final output was synthesized into one coherent answer.
