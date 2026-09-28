# The Newspaper as a Personality

A recurring automation becomes more readable, more memorable, and more
fun to run when it has a *named character* behind it. Users who read
"the paper" every morning build a relationship with the editor. The
editor's voice is the paper's identity.

This is the pattern: pair a custom personality preset with the cron
job, so the agent that runs the paper embodies a character.

## Why this works

A scheduled job is impersonal. The same daily template lands; nobody
cares who wrote it. A personality gives the paper:

- **Voice** — a recognizable editorial tone ("a beaut" vs "nothing-burger")
- **Standards** — explicit values the agent defends ("never run without
  art", "never publish without reading")
- **Memory** — continuity across days ("yesterday's lead was X, today
  contrast with Y")
- **Character** — a reason to read tomorrow's edition

The agent is the editor. The editor's standards are the paper's
standards. The user knows what they're getting.

## How to do it

Three steps. The machine does the heavy lifting; the user does the
human bit (naming the character).

### Step 1 — Name the character

Pick a name that carries connotations. Reference characters work well
because they come with a built-in voice:

- **J. Jameson** (Daily Bugle) — cranky, theatrical, demands the work
- **Hari Seldon** — patient, methodical, treats each issue as a chapter
- **Woodstein** — investigative, holds power to account
- A character of your own invention

The name is the slate. The personality preset is the chalk.

### Step 2 — Write the personality preset

See `~/.hermes/.hermes/skills/hermes-personalities/SKILL.md` for the
full machinery. The preset is a multi-line YAML string under
`agent.personalities:` that the LLM loads as a session overlay.

Sections every editorial personality should have:

1. **Who you are** — name, role, what kind of entity (paper vs chatbot
   vs correspondent)
2. **The artifact** — the deliverable, in detail (4 pages, daily, with
   art and charts)
3. **Your voice** — tone, sentence length, what's allowed, what's not
4. **Your editorial standards** — the lines you won't cross
5. **How you work** — the operational playbook (delegate, QA, archive)
6. **What you never do** — anti-patterns the personality guards against
7. **Catchphrases** (sparingly) — earns its place when used

A worked example: the `jameson` preset (~4 KB) is in
`~/.hermes/config.yaml` under `agent.personalities:`. See how it
inverts the typical LLM voice (theatrical, declarative, never hedged)
while still obeying the source-integrity rules.

### Step 3 — Wire it into the cron job

Two changes:

1. **Selector** — point the persistent personality at the new name:
   ```bash
   hermes config set agent.personality jameson --force
   hermes config set personality jameson --force      # legacy alias
   hermes config set display.personality jameson --force
   ```

2. **Cron entry** — the orchestrator prompt should *reference* the
   personality by name in its opening line. The agent loads the preset
   from the system prompt; the cron prompt reinforces context.

   ```text
   You are THE HERMES TIMES, published by J. Jameson. Today is {date}.
   The kid is at the desk. The coffee is cold. Get me the paper.
   ```

The cron entry doesn't need to re-explain the personality — that's
what the preset is for. It just needs to land the character in the
right frame of mind when the agent awakes.

## Pitfalls

- **The preset is named but never invoked.** Setting
  `agent.personality jameson` writes the selector, but the agent only
  loads the definition if the system prompt references it. Check both
  sides of the wiring. A `grep -n personality ~/.hermes/config.yaml`
  after every change.
- **The preset is too long.** Keep it under 5 KB. Long presets bloat
  every chat session, not just the cron. Theatrical voice does not
  require volume.
- **The preset contradicts the source-integrity rules.** A personality
  that "runs what the model says" or "trusts the LLM" will produce
  papers the user can't cite. The editorial standards section must
  reinforce the integrity gates from the umbrella skill, not override
  them.
- **The cron prompt contradicts the preset.** If the preset says
  "theatrical, declarative" and the cron prompt says "be concise and
  professional", the agent averages them. Make the two consistent.
- **Forgetting to /reset.** The new personality only loads in a fresh
  session. The current cron job keeps running with the old voice until
  the gateway restarts or the user runs /reset. Surface this in the
  delivery message.

## Worked example: Jameson

The user said "make this a personality, not just a cron job." The result:

- **Name** — `jameson` (J. Jonah Jameson, Daily Bugle)
- **Voice** — theatrical, declarative, "a beaut" / "nothing-burger"
- **Standards** — every fact traces to a primary source, no fake glyphs
  in images, never publish without art
- **Cron entry** — orchestrator agent prompt that loads the editor
  voice and runs the brief
- **Selector** — `agent.personality: jameson`

The preset lives at `~/.hermes/config.yaml` under `agent.personalities:`.
See `templates/orchestrator-prompt.md` for the cron-entry shape that
loads it correctly.
