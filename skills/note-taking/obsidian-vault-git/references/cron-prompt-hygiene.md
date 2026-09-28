# Cron-prompt hygiene

When infrastructure state changes (vault becomes git-tracked, new cron
registered, tool wired up, env var added), audit **every** active cron
prompt for hard rules that contradict the new reality. Stale rules
silently corrupt workflows because the LLM will follow them obediently.

## When to run the audit

Trigger conditions for re-reading `~/.hermes/cron/jobs.json` end to end:

- Vault state changed: git-tracked, remote added, branch renamed.
- File system layout changed: vault moved, new top-level folder added.
- New cron registered that takes over a piece of work another cron used to do
  (e.g. you added a daily-vault-push cron — writer crons must be told not
  to git push).
- Tool / CLI availability changed: a script the cron's prompt referenced
  was renamed or moved.
- Env vars the prompt depended on were unset / renamed.
- Upstream contract changed: a remote API changed auth, an output format
  changed, etc.

## What to look for

A cron prompt is *infected* by staleness when it contains a hard rule
that's now false. Examples from this session, all observed in the wild:

| Stale rule (in prompt)                          | What changed                                | Patched to |
|-------------------------------------------------|---------------------------------------------|------------|
| "The vault isn't git-tracked at root"           | Vault was git-tracked in the previous session | "Vault is git-tracked (main, origin = adityasasidhar/obsidian). Do NOT run git — the daily-vault-push cron at 01:15 IST owns commits/pushes." |
| "Don't run `git` from this prompt" (wrong reason) | The vault was not yet git-tracked           | Same patch: reason flipped, prohibition stays |
| "Push to GitHub via `gh repo sync`"             | User switched to `git push` credential-helper auth | Update to use the new auth method |
| "Run `wiki/build_index.py --check`"             | That script was renamed to `wiki/index.py --verify` | Update script name |

## How to do the audit

```bash
# 1. List every active cron and its prompt
python3 -c "
import json
with open('/home/arctic/.hermes/cron/jobs.json') as f:
    for j in json.load(f).get('jobs', []):
        print(f\"=== {j['name']} ({j['id']}) schedule={j['schedule']} ===\")
        print(j['prompt'])
        print()
"

# 2. For each prompt, scan for hard-rule sentences
#    Look for: 'do not', 'never', 'always', 'must not', 'isn't', 'cannot'
#    Anything containing one of these and a concrete noun (a tool, a path,
#    a state, an auth method) is a candidate for staleness.

# 3. Cross-check each hard rule against the actual filesystem / env / config
#    that the prompt claims to describe. If the claim is now false, fix it.
```

A purely textual scan isn't enough — you have to verify each hard claim
against reality. The same sentence can be stale in one sense and correct
in another: e.g. "do not run git" was right when the vault was untracked
AND right after the vault became tracked (for a *different* reason). The
rule's text didn't change; the *justification* did. Capture both.

## The patch shape

Don't just append "Note: vault is now git-tracked" to a bullet list.
Restructure the hard-rule sentences to carry the new state AND the new
reasoning. A reader (human or LLM) following the prompt should understand
*both* what to do and *why*, so a future infrastructure flip reads as
sensible context rather than contradicting noise.

Anti-pattern: leave the old rule's *text* in place, add a fix-up note
above it. This guarantees the LLM sees both and gets confused about which
takes precedence. Always rewrite the rule in place.

## Why this lives at the skill level, not just memory

Memory captures *that the user prefers something*; the skill captures the
*operational procedure* for the class of work. "Re-audit cron prompts when
infrastructure changes" is a procedure, not a preference. The next session
that initializes a new vault will hit this exact trap; the skill it loads
must carry the prevention, not just the user-preference assertion.

## Cross-references

- `obsidian-vault-git` SKILL.md, "After init: schedule the daily push"
  — the canonical trigger: adding a git cron requires editing the
  upstream writer prompt.
- `references/daily-vault-push-cron.md`, "Edits to the upstream
  vault-writing cron's prompt" — the concrete edit recipe.
