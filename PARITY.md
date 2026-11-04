# Parity with Hermes Agent

An honest feature matrix against the Hermes Agent README
(<https://github.com/NousResearch/hermes-agent>, the only first-party page
reachable when this was written; the official docs host was not). zero-hermes
is deliberately a *small* Hermes-inspired agent for a Raspberry Pi, so "parity"
means: every capability a single-user, self-hosted agent needs, minus the
cloud-scale and multi-platform surface.

**Verdict:** feature-complete for the single-box core; deliberately absent
for platform sprawl (see *Out of scope*). It is not a drop-in replacement.

## Matrix

| Hermes capability | zero-hermes | Notes |
| --- | --- | --- |
| Tool-calling loop, streaming | **yes** | Anthropic + OpenAI-compat wire formats, SSE streaming, retry on transient errors, provider fallback chain |
| 40+ tools / toolsets | partial | ~20 builtins (shell, files, fetch, search, memory, cron, todo, subagent…). `[agent].enabled_tools` is the toolset switch. More tools arrive via MCP |
| MCP servers | **yes** | stdio (persistent session, respawn) and streamable HTTP (JSON/SSE, session id, headers) |
| Agent-curated memory | **yes** | SQLite notes + `MEMORY.md`/`USER.md` sidecars, per-turn recall injected into the prompt |
| Cross-session search | partial | FTS5 recall; no LLM summarisation of hits |
| Context compression | **yes** | pair-safe compaction + token-budget summary (extractive); `/compress` asks the model for a summary and keeps the recent third verbatim, falling back to extractive on failure |
| Skills (agentskills-style `SKILL.md`) | **yes** | recursive loader, create/patch/archive, hub sync, `curate` command |
| Self-improving skills | partial | model can write/patch skills; `curate` reviews the library on demand or via cron |
| Command approval / security | partial | no interactive approval (headless daemon). Instead: command guard refuses catastrophic commands, SSRF guard on fetch (spelling checks + DNS resolution vetting with connection pinning), CSRF on web, Telegram allow-list required, tools allow-list |
| Cron with delivery | **yes** | 5-field cron, timezone, delivery to Telegram chats |
| Subagents | **yes** | up to 4 concurrent, isolated, cannot recurse |
| Messaging gateway | partial | Telegram only (Hermes: Telegram, Discord, Slack, WhatsApp, Signal, Email, Home Assistant) |
| Session commands | **yes** | `/new`, `/clear`, `/retry`, `/undo`, `/compress`, `/status`, `/tools`, `/help` in chat |
| `hermes doctor` | **yes** | `zero-hermes doctor` |
| Provider switching | **yes** (model) | config-driven (`[provider]`, fallbacks) plus `/model NAME` in chat; switching *provider kind* needs a config edit |
| Web UI | **yes** | local axum + SSE UI, multi-session |
| Terminal backends | local only | Hermes: Docker, SSH, Modal, Daytona, … |

## Out of scope (on purpose)

Voice/TTS, cloud browser, image generation, extra chat platforms, container /
cloud sandbox backends, plugin marketplace, profiles, batch trajectory
generation, Honcho user modelling, migration importers. Each is either heavy
(violates the <15 MB / <10 MB RSS budget), needs a vendor service, or only
matters at multi-user scale.

## Known gaps worth closing next

1. LLM-written *recall* summaries and automatic (not just `/compress`) model-written compaction.
2. A second gateway (Discord or Slack) behind the existing `Channel` trait.
