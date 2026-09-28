# Holographic Memory Provider — Operational Notes

**Source of truth:** `~/.hermes/hermes-agent/plugins/memory/holographic/`
README and code disagree in places — **the code wins**.

## What it actually is

- **Local SQLite** fact store at `$HERMES_HOME/memory_store.db` (default).
- **FTS5** full-text search built in (no extra dep).
- **HRR (Holographic Reduced Representations)** for compositional queries — **requires NumPy at runtime**, not "optional" as the README claims.
- **Trust scoring** per fact; `fact_feedback` trains the score.
- **`on_session_end` hook** is registered (see `plugin.yaml`) but **auto-extract is OFF by default**.

## README vs code — actual gotchas

| Claim in README | Actual behavior |
|---|---|
| "NumPy optional for HRR algebra" | `holographic.py` calls `_require_numpy()` in every HRR op (`encode_atom`, `bind`, `unbind`, `bundle`, `similarity`, `encode_text`). Without NumPy, `reason`/`related`/`probe` actions fail; FTS5 search still works. **NumPy is effectively required if you want the full toolset.** |
| "Tool: `fact_store` with 9 actions" | True; actions are `add, search, probe, related, reason, contradict, update, remove, list` per README. |
| "Tool: `fact_feedback` for trust scoring" | True. |

**The system is good. Don't trust the README on the NumPy part — verify with `python3 -c "import numpy"` before assuming HRR will work.**

## Public Python API (the names you'll actually use when smoke-testing)

```python
import sys
sys.path.insert(0, '/home/arctic/.hermes/hermes-agent/plugins/memory/holographic')
from store import MemoryStore

store = MemoryStore('/home/arctic/.hermes/memory_store.db')

# Real method signatures (verified via inspect.signature):
#   add_fact(content: str, category: str = 'general', tags: str = '') -> int
#   search_facts(query: str, category: str | None = None, min_trust: float = 0.3, limit: int = 10) -> list[dict]
#   list_facts(category: str | None = None, min_trust: float = 0.0, limit: int = 50) -> list[dict]
#   remove_fact(fact_id: int) -> bool
#   update_fact(fact_id: int, **kwargs) -> bool
#   record_feedback(fact_id: int, helpful: bool) -> None
#   rebuild_all_vectors() -> None
#   close() -> None
```

**Don't guess method names from the README** — it only documents the agent-facing `fact_store` tool, not the Python class. Use `inspect.signature(MemoryStore.<method>)` when in doubt.

## FTS5 hyphen quirk

`search_facts` does **not tokenize on hyphens the way you might expect**. A fact whose content contains `obsidian-galaxy-graph` is returned by `list_facts` (by ID) but a `search_facts("obsidian-galaxy-graph")` returns 0 hits — the FTS5 tokenizer treats the hyphen as part of the token, so the query pattern won't match unless the fact content has the same token boundary. Workarounds:
- Search for a fragment that doesn't include the hyphen: `search_facts("obsidian galaxy")` or `search_facts("obsidian-galaxy")` (the second works only because the underscore/normalized form sometimes matches).
- Use `search_facts("galaxy")` for a broad match.
- Verify a fact exists by `list_facts(limit=N)` and filter manually if search is unreliable.
- Same applies to project names with underscores, dots, or other punctuation: if `search_facts` returns 0, don't conclude the fact is missing — verify by ID.

This is the most common "I added a fact but `search_facts` can't find it" failure mode. When building indexing for a project, prefer plain-word tokens in fact content (e.g. "obsidian galaxy graph project" rather than "obsidian-galaxy-graph" as the only searchable term).

## End-to-end smoke-test recipe

After enabling Holographic with `hermes config set memory.provider holographic`, run this to prove the path works before trusting it in production:

```python
import sys, os
sys.path.insert(0, '/home/arctic/.hermes/hermes-agent/plugins/memory/holographic')
from store import MemoryStore

db_path = os.path.expanduser('~/.hermes/memory_store.db')
store = MemoryStore(db_path)

# Write
fact_id = store.add_fact(
    content="Holographic smoke-test: this should be retrievable via FTS5",
    category="test",
    tags="verification",
)
assert fact_id > 0

# Read
hits = store.search_facts("holographic smoke-test")
assert any(h['fact_id'] == fact_id for h in hits), f"write didn't round-trip: {hits}"

# Negative read (gibberish)
assert store.search_facts("xyzzyxyzzy") == [], "FTS5 isn't filtering noise"

# Cleanup
assert store.remove_fact(fact_id)
assert store.list_facts() == []

print("OK: Holographic end-to-end works.")
```

DB file size after a clean run: **65,536 bytes** (one SQLite page). The fact leaves a free-list entry but the page stays allocated — that's normal, not a bug.

## Setup command gotcha

`hermes memory setup` is an interactive picker that **exits silently in non-TTY** (`Cancelled. No changes saved.`) and the message looks like a successful early-exit. If you can't drive a TTY (cron, agent loop, sandboxed shell), use the direct config command instead:

```bash
hermes config set memory.provider holographic
hermes memory status          # verify it took
```

This is faster and more verifiable than the wizard for headless contexts.

## Configuration knobs (`plugins.hermes-memory-store` in `config.yaml`)

| Key | Default | Effect |
|---|---|---|
| `db_path` | `$HERMES_HOME/memory_store.db` | Override only if you want a non-default location (e.g. per-profile isolation) |
| `auto_extract` | `false` | When `true`, the `on_session_end` hook fires fact extraction at session close. Adds a background LLM call per session; turn on only if you want passive KB growth |
| `default_trust` | `0.5` | New facts default to this trust score. Bump to `0.7`+ if you want stricter quality gating |
| `hrr_dim` | `1024` | HRR vector width. Higher = more capacity per bundle, more CPU per similarity |

For the Aditya profile: defaults are correct. Don't enable `auto_extract` unless you want LLM cost on every session close.

## When to pick Holographic over the others

Use this rule of thumb (encoded in the parent skill's picker table too):

- User said **"free"** → Holographic
- User said **"fast"** or **"low power"** or **"local"** or **"no cloud"** → Holographic
- User said **"the most powerful cross-session modeling"** or **"I don't mind paying"** → Honcho
- User already has OpenViking infrastructure → OpenViking
- Everything else → Mem0 (safe generalist)

The "free + fast + low-power" combination is Holographic-specific. No other provider in the catalog matches all three.
