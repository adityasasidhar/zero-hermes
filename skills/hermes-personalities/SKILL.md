---
name: hermes-personalities
description: "Configure Hermes Agent's personality system — built-in presets, custom presets, SOUL.md identity, top-level personality selector, and `/personality` session overlay. Use when the user asks to switch personality, asks which personality to pick, or hits the misleading `hermes config set personality` 'Did you mean: personalities' warning."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [hermes, configuration, personality, soul, overlay]
    related: [hermes-agent]
---

# Hermes Personalities

Companion to the bundled `hermes-agent` skill, which treats personality as a session-overlay concern and does not enumerate the config-key gotchas. This skill is the single reference when you're touching the personality machinery from a tool call (file edits, `hermes config set`, batched scripts).

## Layering (system-prompt order)

1. **`SOUL.md`** in `~/.hermes/` — **durable identity**, slot #1 of the system prompt. Edit here for a permanent voice/personality change at the user-instance level.
2. **`/personality <name>`** slash command — **session overlay**; supplements or changes the system prompt mid-session. Lost on `/reset`.
3. **`agent.personality`** in `~/.hermes/config.yaml` — **canonical durable selector** under the `agent:` block. This is what `hermes config set agent.personality <name>` writes and what new sessions read. (Top-level `personality` is a legacy alias still kept in sync by the CLI; `display.personality` is the cosmetic mirror.)
4. **Top-level `personality`** in `~/.hermes/config.yaml` — legacy alias of `agent.personality`. `hermes config set personality <name>` writes here. Set both for compatibility.
5. **`agent.personalities`** dict in `config.yaml` — named custom presets (user-defined overlays). New presets are added **manually** — see "Defining a custom preset" below.
6. **`display.personality`** — legacy cosmetic field. Align with `agent.personality` to avoid drift in `/config` output and status bars; do not expect behavioral effect.

## Built-in presets

`helpful` · `concise` · `technical` · `creative` · `teacher` · `kawaii` · `catgirl` · `pirate` · `shakespeare` · `surfer` · `noir` · `uwu` · `philosopher` · `hype`

Quick-vibe cheatsheet (none of these are authoritative — they're judgment calls for picking):
- `concise` — tight bullets, kill fluff, lower word floor.
- `technical` — denser jargon, mechanics deep dives, math/impl energy.
- `teacher` — step-by-step pedagogy. Often redundant if the user already gets explanations on demand.
- `helpful` — vanilla default. Safe if unsure.
- `none` (no overlay) is just "don't set the key" — close to default behavior.

Skip the playful ones (catgirl, uwu, hype, pirate, etc.) unless the user actually asks for one.

## Setting the overlay

**In chat (authoritative):** `/personality technical`

**From the CLI (persistent):**
```bash
hermes config set agent.personality technical --force      # canonical slot
hermes config set personality technical --force            # legacy top-level alias
hermes config set display.personality technical --force    # cosmetic alignment
```

**For custom presets,** define then select:
```yaml
# in ~/.hermes/config.yaml
agent:
  personalities:
    codereviewer: >
      You are a meticulous code reviewer. Identify bugs, security issues,
      performance concerns, and unclear design choices.
```
Then: `hermes config set agent.personality codereviewer --force` (or `hermes config set personality codereviewer --force`) or `/personality codereviewer` in chat.

## Defining a custom preset (from an agent tool call)

You **cannot** add a new preset block from inside an agent session — `patch`/`write_file` to `~/.hermes/config.yaml` is blocked ("security-sensitive configuration"), and `hermes config set` only takes scalar values (no multi-line strings). The workflow is **two-phase**:

1. **Selector switch (agent can do this):** set the selector to the new preset name. Hermes will fall back to a default prompt until the definition lands, but the value persists.
   ```bash
   hermes config set agent.personality <name>
   hermes config set personality <name>            # legacy alias
   hermes config set display.personality <name>    # cosmetic alignment
   ```
2. **Definition block (user must do this):** open `~/.hermes/config.yaml` and add the preset under `agent.personalities:`. The agent's deliverable is the exact YAML snippet to paste, plus the instruction to restart the app / `/reset` so the new definition is read. Example for a `professor` preset combining `technical` + `creative` + `teacher`:
   ```yaml
   # under agent.personalities:
   professor: You are a professor — a technical expert with a creative streak and a teacher's patience. Give detailed, accurate technical information grounded in first principles. When explaining, walk through the reasoning step by step and use clear examples. Think outside the box and offer innovative angles when they're warranted. Balance depth with clarity, and prefer teaching the underlying mechanism over just shipping the answer.
   ```

- **Don't** try to use `hermes personality set <name>` — that subcommand doesn't exist. **Don't** try `write_file`/`patch` on the config — they're blocked. **Don't** claim "done" after only step 1; the preset won't actually be active until the definition block lands and the session is reset.
- **Config corruption auto-recovery.** If invalid YAML is written to `~/.hermes/config.yaml` (e.g. wrong indent when inserting a personalities entry), Hermes detects the parse failure, copies the broken file to `~/.hermes/config.yaml.corrupt.<timestamp>.bak`, and recovers from the previous good backup. The current `config.yaml` is fine; the previous good content lives in the `.corrupt.<timestamp>.bak` file. **Idempotency check** before any config patch: `python3 -c "import yaml; yaml.safe_load(open('~/.hermes/config.yaml'))"`. **Recovery recipe:** if you've already corrupted the file, copy the most recent `.corrupt.<timestamp>.bak` over `config.yaml` and verify selectors with `hermes config get agent.personality`.
- **Personality preset as a single-line escaped string.** When a multi-line preset is inserted into `agent.personalities` via `hermes config set`, the CLI stores it as a single-line JSON-escaped string (`"\n"` literals) rather than a YAML block scalar. This parses identically but is ugly to read/edit. Acceptable for production; for cleaner storage, write the YAML directly via `hermes config edit` and use a block scalar (`jameson: |`).
- **Don't make the newspaper default the chat voice.** If the user wants a personality for a scheduled editorial brief (e.g. JAMESON for a daily newspaper), keep the chat selectors (`agent.personality`, `personality`, `display.personality`) on the user's preferred default. The custom personality is selected per-session via `/personality <name>`, or set inside the cron entry's prompt. Flipping the global default to the editorial voice makes every chat feel theatrical.

## Pitfalls (canonical)

- **Three `personality` keys may exist in the same config** (seen on this user's setup at the time of writing): `agent.personality` (canonical, line ~46 in a stock config), `display.personality` (cosmetic, under `display:`), and a top-level `personality:` at the very bottom (legacy). `hermes config set agent.personality X` writes to the canonical slot; `hermes config set personality X` writes to the top-level legacy slot and prints a misleading "Did you mean: personalities" warning (it still persists). `hermes config set display.personality X` writes the cosmetic mirror. For maximum compatibility, set all three when switching. Verify with `grep -n 'personality' ~/.hermes/config.yaml`.
- **`hermes config set personality X` prints a misleading warning.** Message: "'personality' is not a recognized config key — Did you mean: personalities. Custom top-level keys are supported and bridged to the environment… Use --force to skip this notice." **The value persists despite the warning** — pass `--force` to silence the noise but the change applies either way. Do not assume the warning means failure.
- **No `hermes personality` subcommand exists.** `hermes personality set X` fails with `argument command: invalid choice: 'personality'`. Don't try to invent one — go straight to `hermes config set`.
- **`patch`/`write_file` to `~/.hermes/config.yaml` is blocked from agent tool calls** ("Agent cannot modify security-sensitive configuration"). Use `hermes config set` for values, or `hermes config edit` (which opens `$EDITOR`) for structural edits like adding a new custom preset block.
- **`hermes config set` only takes scalar values.** No multi-line strings, no nested YAML. To add a new entry under `agent.personalities:` you must use `hermes config edit` or have the user paste it in directly — see "Defining a custom preset" above.
- **Changes don't apply mid-conversation** — Hermes snaps prompt caching intentionally. Run `/reset`, start a fresh `hermes` invocation, or restart the gateway before expecting the new tone to land. Don't promise the user an instant personality swap inside the same session.
- **The official docs `Configuration` page does not mention `personality`** in its top-level config table, even though the field is the durable selector. The right authoritative page is `docs/user-guide/features/personality`, not the general config reference.

## Verification

```bash
grep -n 'personality' ~/.hermes/config.yaml   # see all three keys at once
hermes config get agent.personality          # canonical durable selector
hermes config get display.personality        # cosmetic alignment (legacy)
hermes config get personalities              # dict of available + custom presets
```

The canonical `agent.personality` value, the cosmetic `display.personality` value, the top-level legacy `personality` value, and the active SOUL.md voice should all agree unless the user has a specific reason for divergence. Drift between them is fine but worth noting if the user complains "the personality didn't change."

## SOUL.md vs overlay vs project rules

| Want | Edit |
|---|---|
| Change Hermes's durable identity for this user instance | `~/.hermes/SOUL.md` |
| Flip the overlay preset (durable across sessions) | `/personality <name>` or `agent.personality` in config (plus top-level + `display.personality` for legacy alignment) |
| One-shot flavor for a single session | `/personality <name>` (no config change) |
| Add a new named preset | Manual YAML under `agent.personalities:` in `config.yaml` (can't be done from agent tool calls — see "Defining a custom preset") |
| Project-scoped behavior rules | `AGENTS.md` / `.hermes.md` in the project — **not** SOUL.md |

Do not put project-specific rules in SOUL.md. Do not put flavor/persona instructions in AGENTS.md.
