---
name: hermes-memory-ops
description: "Curate Hermes memory and providers. Use when consolidating."
version: 1.2.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [hermes, memory, mem0, honcho, holographic, openviking, supermemory, byterover, retaindb, hindsight, memori, configuration]
    related: [hermes-agent]
---

# Hermes Memory Operations

This skill is the operational companion to the bundled `hermes-agent` hub. The hub points at the docs; this skill covers **what to actually do** with the docs' outputs — the workflow of curating the built-in stores, the trade-offs of the eight external providers, and the `memory` tool's silent-failure modes.

**Authoritative source:** https://hermes-agent.nousresearch.com/docs/user-guide/features/memory
**Provider catalog:** https://hermes-agent.nousresearch.com/docs/user-guide/features/memory-providers

The hub's `hermes-agent` skill is bundled/protected — extend this one, never that one.

## When to Load

- User asks "consolidate my MEMORY.md", "free up memory chars", "what's eating my memory budget"
- User asks which memory provider to use, wants to set one up, or asks "is Honcho / Holographic / Mem0 worth it"
- A `memory` tool call returned an error or a "stale" state and the next step is unclear
- New session, and you need to know what's already saved vs. what needs to be discovered via `session_search`

## First Rule: Don't Web-Search What `hermes memory --help` Already Tells You

The instinct to web-search "best Hermes memory provider" is wrong 90% of the time. The local CLI is the source of truth and is faster, offline, and doesn't hallucinate:

```bash
hermes memory --help                # subcommands + available providers list
hermes memory status                # current provider + char limit
hermes memory setup                 # interactive picker (REQUIRES A TTY — see pitfall below)
hermes memory off                   # disable external provider
hermes memory reset                 # ERASE built-in memory — destructive, never run without explicit ask
```

**`hermes memory setup` is TTY-only.** It runs an interactive picker; in non-TTY contexts (cron, agent loop, sandboxed shell) it exits silently with `Cancelled. No changes saved.` which looks like a successful early-exit. The non-interactive escape hatch is:

```bash
hermes config set memory.provider <name>     # e.g. holographic, honcho, mem0
hermes memory status                          # confirm it took
```

Use the wizard when you have a real terminal; use the config command for everything else.

Plus:

```bash
grep -A5 '^memory:' ~/.hermes/config.yaml          # current limit + provider
wc -c ~/.hermes/memories/MEMORY.md ~/.hermes/memories/USER.md
```

If those don't answer it, **then** check the docs. The web is for "what does Holographic's HRR algebra do", not "how do I list providers".

## Built-in Memory: MEMORY.md and USER.md

| File | Default cap | Configurable via | Purpose |
|---|---|---|---|
| `~/.hermes/memories/MEMORY.md` | 2,200 chars | `memory.memory_char_limit` in `config.yaml` | Agent's personal notes — environment, conventions, lessons, tool quirks |
| `~/.hermes/memories/USER.md` | 1,375 chars | same key | User profile — identity, preferences, communication style |

Both are injected into the system prompt at session start as a **frozen snapshot** (changes during a session are persisted to disk but won't appear in context until next session — this preserves prefix caching).

### What belongs where

- **MEMORY.md** = "what's true about the environment" (paths, billing facts, tool quirks, workflow patterns)
- **USER.md** = "what's true about the human" (name, communication style, preferences, response format)
- If a fact could change per-profile → MEMORY.md. If it travels with the human → USER.md.

### Capacity rules

- Memory **does not auto-compact**. A write that would exceed the limit returns an error with the current entries and asks you to consolidate, not silently drops.
- Best practice: when the system prompt shows >80% usage, consolidate *before* adding new entries.
- Compact, information-dense entries > verbose ones. Three facts in one entry beats three one-fact entries when they're related.

### Cap is a per-turn token cost, not just a storage ceiling

Every char in MEMORY.md and USER.md is rendered into the system prompt of every turn. The default 2,200-char cap is ~800 tokens per file; doubling to 15,000 chars is ~6,700 tokens per file — both go in every prompt. Going from 7,500 → 15,000 doesn't double the user-visible memory, it roughly **doubles the per-turn system-prompt cost** of memory. Use `hermes config set memory.memory_char_limit <n>` (and `user_char_limit`) only when the user explicitly asks for more headroom, and flag the cost trade-off before any change beyond ~5,000.

## Consolidation Triage Framework

When `MEMORY.md` or `USER.md` is bloated, don't just trim — triage each entry against three buckets. Ask the user to pick an aggression level if it's a non-trivial pass; default to "balanced" if they don't.

**Step 1 — Audit `fact_store` overlap first.** Before touching anything, run `fact_store list` (or `search_facts` for the topic). If an entry is already there, **do not duplicate it in MEMORY.md** — point to it instead, or remove it from MEMORY.md entirely. The two stores solve different problems (always-on vs on-demand lookup); same content in both is wasted in-context budget. Real example: a user had 3 MEMORY.md entries that were verbatim copies of `fact_store` facts (hermes-times-v4 send.sh silent-drop, /tmp wipe incident, obsidian-galaxy-graph project state). Removing them cut MEMORY.md by 70% with no information loss.

**Step 2 — Per-entry decision (apply each entry to one bucket):**

| Bucket | What goes here | Action |
|---|---|---|
| **KEEP** | Behavioral rules the agent must apply every turn (paths, billing facts, communication style, recurring workflow conventions) | Keep in MEMORY.md / USER.md, trim verbose phrasing and incident-log framing |
| **MOVE to `fact_store`** | Project state, incident reports, debugging recipes, one-shot context — anything true but not needed in every prompt | Remove from MEMORY.md, add to `fact_store` if not already there |
| **DROP** | Pure tombstone content ("removed file X on date Y", "confirmed rule X exists in skill Y"), duplicates between entries, status statements | Delete entirely |

**Step 3 — Trimming tactics that preserve the rule:**
- Replace "Confirmed 2026-07-24: removed old path, 8.2M reclaimed" → "lives at /path/" (drop the history)
- Replace "Always run `date` first (caught 2026-07-29 challenging my 8.5h vs 7.5h arithmetic)" → "Always run `date` first; show arithmetic for duration math" (drop the war story)
- Replace "(verified 2026-07-29)" → keep only if the date is a load-bearing fact; otherwise drop
- Compress minute-by-minute schedules into a one-line summary + pointer to the source note
- Replace "Captured as Golden Rule 1a in `vault-knowledge-graph` skill" with just the rule (the skill reference is editorial)

**Step 4 — User check before aggressive cuts.** Show the per-entry table with before/after char counts and ask the user to pick aggressive / balanced / minimal. Default to "balanced" if they're delegating. Never auto-trim >25% of a file without confirmation — silent aggressive curation is a trust violation.

**Step 5 — Write strategy depends on the scope:**
- **Small edits** (typo, one-entry update): use the `memory` tool with `replace` + `add`+`remove` per the safe-merge pattern below.
- **Whole-file rewrites** (consolidation): back up first (`cp ~/.hermes/memories/MEMORY.md ~/.hermes/memories/MEMORY.md.bak-$(date +%Y%m%d-%H%M%S)`), then `write_file` the new content. The `memory` tool's add-then-remove safe-merge pattern doesn't apply when overwriting the whole file — `write_file` either succeeds or raises.
- **Verify after**: `wc -c` both files, structural check (no double `§`, no leading/trailing `§`), re-read to confirm content is what you intended.

## The `memory` Tool: Field Names and Silent Failure Modes

The `memory` tool accepts `action` ∈ {`add`, `replace`, `remove`} and `target` ∈ {`memory`, `user`}.

### Field name pitfall

**Both `add` and `replace` use `content` for the new text — not `new_string`.** A `replace` with `new_string=` returns `{"success": false, "error": "content is required for 'replace' action."}` and **does not modify the file**, but the rest of the turn continues as if the call worked. Always check the response's `success` field; never trust a "no error" reading of the `error` field alone. After any `replace` or batch of writes, verify with `wc -c` on the file or `cat` the file — the per-call response isn't enough.

### Safe merge pattern (add-then-remove)

When consolidating N entries into one merged entry:

1. **`add` the merged entry first.** If the add bounces (over limit), you still have the originals — nothing is lost.
2. **Verify the add succeeded** (`success: true` in response, new `usage` shows the merge added the expected chars).
3. **Then `remove` each original** by a unique `old_text` substring. Substring matching requires a substring that matches exactly one entry — pick a phrase that doesn't appear in the merged entry.
4. **Read the file back at the end** and confirm the duplicates are gone. The `usage` percentage reports current state, but it doesn't tell you whether two semantically-identical entries exist. After the consolidation I caught 2 duplicate-merge bugs that the `usage` counter alone wouldn't have surfaced — a duplicate "Astro blog path" entry and a duplicate "Gmail tooling" entry, both because I trusted an in-flight `replace` that silently no-op'd.

The reverse pattern (remove-then-add) is risky: if the add fails, you've lost the original information.

### When to use `replace` vs `add`+`remove`

- **`replace`** when the change is a small edit to one entry (typo, value update, scope refinement). The `old_text` substring needs to uniquely identify the entry.
- **`add`+`remove`** when you're merging multiple entries or restructuring. `replace` is bound by the same char limit as `add`, so swapping an entry for a longer one can still overflow. **And: a failed `replace` is silent, so prefer `add`+`remove` whenever the outcome matters.**

### Security scanning

Memory entries are scanned for prompt-injection / exfiltration patterns before acceptance. If a write bounces with no obvious char-limit error, check whether the content contains a flagged pattern (URLs, base64-looking strings, "ignore previous instructions", etc.).

## External Provider Picker

Eight providers, all run **alongside** the built-in stores (additive, never replacing). Only one external can be active at a time.

| Provider | Storage | Cost | Best for | Setup friction |
|---|---|---|---|---|
| **Honcho** | Cloud (or self-hosted) | Paid | Multi-agent, cross-session user modeling, dialectic reasoning | API key + `hermes memory setup` |
| **Holographic** | Local SQLite | Free | Local-only, advanced retrieval (HRR algebra + trust scoring) | None — ships with Hermes |
| **OpenViking** | Self-hosted | Free | Filesystem hierarchy + tiered loading | Run a separate `openviking` server |
| **Mem0** | Cloud or self-hosted OSS | Free / OSS | Server-side LLM fact extraction | API key (or self-host) |
| **Hindsight** | Cloud or local | Free / Paid | Knowledge graph + reflect synthesis | API key |
| **Supermemory** | Cloud or self-hosted | Free / Paid | Context fencing + multi-container | API key |
| **ByteRover** | Local or cloud | Free / Paid | Pre-compression extraction | `brv` CLI |
| **RetainDB** | Cloud | $20/mo | Delta compression | API key |
| **Memori** | Cloud | Free / Paid | Tool-aware memory + structured recall | `pip install hermes-memori` + `hermes-memori install` |

### Picking rules of thumb

- **User said "free" / "fast" / "low power" / "local" / "no cloud" (any of those)** → **Holographic**. It's the only provider in the catalog that matches all three together.
- **User wants zero new accounts / zero monthly bills / local-first** → Holographic
- **User wants the most powerful cross-session user modeling and doesn't mind paying** → Honcho
- **User already has self-hosted infra for an OpenViking-style server** → OpenViking
- **User is paying $20/mo for RetainDB and also paying for a MiniMax plan** → suggest dropping RetainDB, not adding to it. RetainDB is a poor second subscription.
- **User just wants "more memory" without changing the agent's behavior much** → Mem0 (cloud or OSS) is the safe generalist

For the operational details of Holographic specifically — the README-vs-code gotchas (NumPy is required, not optional), the actual Python API surface (`add_fact` / `search_facts` / `list_facts` / `remove_fact`, not `add` / `list_all` / `search`), the non-TTY setup escape hatch, and the end-to-end smoke-test recipe — see `references/holographic.md`.

## After Enabling a Provider: Always Smoke-Test

Don't trust `hermes memory status` alone — the plugin can show "available" with a broken runtime path. After flipping providers, exercise the real path with a write→read→delete round-trip:

```python
import sys, os
sys.path.insert(0, '<plugin_dir>')           # e.g. ~/.hermes/hermes-agent/plugins/memory/<name>
from store import MemoryStore                # class name varies; use dir()/inspect.signature()

store = MemoryStore('<db_path>')             # from plugin.yaml / config
fact_id = store.add_fact(content="smoke-test", category="test")
assert any(h['fact_id'] == fact_id for h in store.search_facts("smoke-test"))
assert store.remove_fact(fact_id)
```

This catches three classes of failure that the status command hides: missing dependencies (the README's "NumPy optional" lie that turned out to mean "NumPy required"), wrong API names, and broken FTS5 indexing. Per the AGENTS.md E2E-validation rule: "For anything touching resolution chains, config propagation, security boundaries, remote backends, or file/network I/O, exercise the real path with real imports against a temp HERMES_HOME. Mocks hide integration bugs."

## Session Search vs Memory: The Boundary

| Tool | Scope | Use when |
|---|---|---|
| `memory` (MEMORY.md / USER.md) | Curated, in-context, always loaded | "What does the agent know about me / my environment" |
| `session_search` | All past sessions, FTS5 over `state.db` | "Did we discuss X last week? Find the session where Y happened" |

Don't try to remember everything in `memory`. Use `session_search` to look up specifics from past sessions and let `memory` carry only the high-signal, always-relevant facts.

## Verification Steps

After any memory write sequence:

```bash
wc -c ~/.hermes/memories/MEMORY.md ~/.hermes/memories/USER.md
# spot-check the file content directly
cat ~/.hermes/memories/MEMORY.md
```

After a provider setup:

```bash
hermes memory status                # should show the new provider as active
# then run the smoke-test above — status alone is not enough
```

## Pitfalls

- **The system-prompt header shows `usage` as a snapshot at session start.** It is NOT live during the session. If you write 500 chars and the header still says "67% — 1,474/2,200", that's correct — the new chars are on disk but the in-context copy is frozen.
- **`hermes memory reset` deletes built-in memory with no confirmation.** Never call it without an explicit user request. (The docs warn; the CLI does not.)
- **Multiple `memory` tool calls in one turn each fire an independent API round-trip.** The `operations` batch param is for memory only; round-trips are cheap enough that you can do `add`+`remove` serially without thinking.
- **The 7,500-char ceiling in `config.yaml` overrides the doc defaults.** If the user has set `memory_char_limit: 7500`, both MEMORY.md and USER.md share that cap — not separate caps. Plan consolidation accordingly.
- **Substring matching for `remove` is not anchored.** A too-short `old_text` (e.g. "test") can match multiple entries. Always include a phrase that's unique to the entry you intend to remove.
- **Plugin READMEs lie about dependencies and API surface.** For Holographic, the README says "NumPy optional for HRR algebra" but the code calls `_require_numpy()` in every HRR operation. Verify with `python3 -c "import numpy"` before assuming a feature will work, and use `inspect.signature()` to confirm the actual Python class API — the README documents the agent-facing tool, not the class methods, and method names differ (`add_fact` not `add`, `list_facts` not `list_all`, `search_facts` not `search`).
- **A failed `replace` is silent.** A `replace` with the wrong field name (e.g. `new_string=` instead of `content=`) returns `{"success": false, ...}` but the rest of the turn may continue as if it worked. Always verify with `wc -c` on the file or by re-reading its contents. The safe merge pattern (`add`-then-`remove`) is more robust than chaining `replace`s because each add is independently verifiable.
- **`hermes memory status` shows the plugin as "available" even when the runtime path is broken.** Treat "available" as "registered and importable", not "end-to-end works". Run the smoke-test above before trusting the provider.
- **Holographic's FTS5 search doesn't tokenize hyphens as you expect.** A fact containing `obsidian-galaxy-graph` may return 0 hits from `search_facts` even though `list_facts` shows it. Search for a hyphen-free fragment, or verify by ID. Don't conclude a fact is missing when search returns 0. See `references/holographic.md` for details.
- **The `memory` tool's safe-merge pattern (`add`-then-`remove`) only applies when modifying existing entries.** When overwriting the whole file with `write_file` (the right strategy for ≥30% rewrites), the safer pattern is: back up first (`cp ~/.hermes/memories/MEMORY.md ~/.hermes/memories/MEMORY.md.bak-$(date +%Y%m%d-%H%M%S)`), then `write_file`, then `wc -c` and re-read to verify. The `memory` tool's silent-failure rules don't apply to `write_file`.
- **The `fact_store` tool and `MEMORY.md`/`USER.md` are not alternatives; they are layers.** If the same fact is in both, it's duplicated cost. Always run `fact_store list` (or `search_facts`) before consolidating — three entries in one user's MEMORY.md were verbatim copies of `fact_store` facts, costing ~1,700 chars of always-on prompt budget for zero information gain.
