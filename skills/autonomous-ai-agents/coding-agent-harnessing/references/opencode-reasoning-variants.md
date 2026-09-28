# OpenCode reasoning effort & variants

Quick reference for orchestrating opencode runs at specific thinking levels.
Three layers — pick the right one for the task.

## 1. CLI flag (per-run, ad-hoc)

```bash
# Show model thinking blocks
opencode run "Refactor auth module" --thinking

# Pick a specific reasoning variant for this run
opencode run "Tricky bug" --model anthropic/claude-sonnet-4-5 --variant max
opencode run "Trivial fix" --model openai/gpt-5 --variant low
```

`--variant` accepts values like `high`, `max`, `minimal`, `low`, `medium`,
`xhigh` — depends on provider. Built-in defaults ship per provider (Anthropic:
`high`/`max`; OpenAI: `none`/`minimal`/`low`/`medium`/`high`/`xhigh`; Google:
`low`/`high`).

## 2. Per-model defaults (persistent in `~/.config/opencode/opencode.jsonc`)

For a default that applies to every opencode session unless overridden:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "anthropic": {
      "models": {
        "claude-sonnet-4-5-20250929": {
          "options": {
            "thinking": { "type": "enabled", "budgetTokens": 16000 }
          }
        }
      }
    },
    "openai": {
      "models": {
        "gpt-5": {
          "options": {
            "reasoningEffort": "high",
            "textVerbosity": "low"
          }
        }
      }
    }
  }
}
```

Shape is provider-specific:
- Anthropic → `thinking: { type: "enabled", budgetTokens: N }`
- OpenAI → `reasoningEffort: "low"|"medium"|"high"`, plus `textVerbosity`/`reasoningSummary`

## 3. Named variants (presets you cycle through with `variant_cycle` keybind)

```jsonc
{
  "provider": {
    "openai": {
      "models": {
        "gpt-5": {
          "variants": {
            "high": { "reasoningEffort": "high", "textVerbosity": "low" },
            "low":  { "reasoningEffort": "low",  "textVerbosity": "low" }
          }
        }
      }
    }
  }
}
```

Inside the TUI, the `variant_cycle` keybind flips between named variants
mid-session. Useful when a task starts simple and you want to escalate thinking
mid-stream.

## Web UI quirk (opencode `web`)

When reasoning effort is configured at the **model level** (layer 2 above),
`opencode web` shows `Default` in the UI for new sessions even though requests
use the configured value. Workaround: pass `--variant <level>` explicitly when
launching the session, or set the variant at the agent level instead of the
model level. (opencode issue #17588.)

## Model selection priority at startup

1. `--model` / `-m` CLI flag
2. `model` key in `opencode.json` (global or project)
3. Last used model
4. First model using internal priority

Variant selection has no CLI fallback to a config default — if you want a
persistent variant, configure it in the model options or define named variants.
