<p align="center">
  <img src="assets/zero_hermes_logo.png" width="220" alt="zero-hermes logo — a winged terminal mark inside a zero" />
</p>

<h1 align="center">zero-hermes</h1>

<p align="center">
  A small, self-hosted Hermes-inspired agent for the Raspberry Pi.
</p>

<p align="center">
  <a href="#why-zero-hermes">Why</a> ·
  <a href="#quick-start-on-a-raspberry-pi">Quick start</a> ·
  <a href="#what-it-can-do">Capabilities</a> ·
  <a href="#configuration">Config</a> ·
  <a href="#security">Security</a>
</p>

## Why zero-hermes

Hermes Agent is powerful. A Raspberry Pi does not need a desktop application,
a large Python environment, or a broad integration platform to be useful.

**zero-hermes** keeps the agent essentials in one Rust binary: a tool-calling
loop, durable memory, skills, scheduled work, delegation, Telegram, and a
small local web UI. Point it at a hosted model—or an OpenAI-compatible model
server such as Ollama—and leave it running on a Pi you control.

It is inspired by Hermes Agent, not a drop-in replacement. The point is a
small, inspectable agent that can live quietly on low-power hardware.

```
Telegram / terminal / browser
             │
             ▼
       zero-hermes on your Pi
       ├── tools: shell, files, HTTP
       ├── SQLite: sessions, recall, notes
       ├── skills: reusable local procedures
       ├── cron: unattended prompts
       └── provider: hosted API or local endpoint
```

## What it can do

- Chat from the terminal, a local browser, or Telegram.
- Stream model output and tool activity.
- Run shell commands; read and write files; fetch HTTP URLs.
- Persist conversations in SQLite and recall matching prior discussions with
  FTS5 search.
- Store durable notes and create or improve local `SKILL.md` procedures.
- Spawn up to four isolated subagents concurrently for independent work.
- Run configured prompts on cron schedules.
- Speak Anthropic Messages or OpenAI-compatible Chat Completions APIs.
- Keep long conversations bounded with pair-safe trimming and token-budget
  summaries.

The built-in tools are `bash`, `read`, `write`, `fetch`, `memory`, `skill`,
and `subagent`. An empty `enabled_tools` list enables them all.

## Quick start on a Raspberry Pi

zero-hermes is designed to run on Raspberry Pi OS 64-bit / other 64-bit ARM
Linux systems. The Pi runs the agent runtime; model inference may be remote or
local, depending on the endpoint you configure.

### 1. Install Rust and build

```sh
git clone <your-repository-url> zero-hermes
cd zero-hermes

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"

cargo build --release
```

The executable is `target/release/zero-hermes`. For a Pi that only runs the
agent, copy that binary and your `skills/` directory to its permanent home.

### 2. Confirm the binary works without network access

```sh
./target/release/zero-hermes run --mock "say hello"
./target/release/zero-hermes tools --mock
```

### 3. Create configuration

```sh
./target/release/zero-hermes init-config --force
nano ~/.config/zero-hermes/zero_hermes.toml
```

Set the provider credentials and model, then start a chat:

```sh
./target/release/zero-hermes chat
```

### Keep it running

For an always-on Pi, run it under a service manager such as systemd. Start
with the local web UI or Telegram gateway only after configuring the security
settings below.

```sh
./target/release/zero-hermes web --bind 127.0.0.1:8088
# or
./target/release/zero-hermes gateway
```

## Providers

The default provider uses the Anthropic Messages wire format and is compatible
with MiniMax's Anthropic endpoint. `openai_compat` works with OpenAI and
servers that implement `/v1/chat/completions`, including a local Ollama setup.

```toml
# Hosted Anthropic-compatible endpoint
[provider]
kind = "anthropic"
base_url = "https://api.minimax.io/anthropic"
api_key = "${MINIMAX_API_KEY}"
model = "MiniMax-M3"
max_tokens = 8192

# Or a local OpenAI-compatible endpoint, for example Ollama
# [provider]
# kind = "openai_compat"
# base_url = "http://127.0.0.1:11434/v1"
# api_key = "not-needed"
# model = "your-local-model"
# max_tokens = 4096
```

The model determines the quality of tool use. A Pi can host a small local
model, but a hosted model is usually the more capable option on constrained
hardware.

## Configuration

Configuration resolution, from highest priority to lowest:

1. `--config <path>`
2. `ZERO_HERMES_CONFIG`
3. `~/.config/zero-hermes/zero_hermes.toml`
4. `./zero_hermes.toml`

Here is a practical Pi configuration:

```toml
[provider]
kind = "openai_compat"
base_url = "http://127.0.0.1:11434/v1"
api_key = "not-needed"
model = "your-local-model"
max_tokens = 4096
connect_timeout_secs = 10
read_timeout_secs = 120

[memory]
path = "~/.local/share/zero-hermes/memory.sqlite"
# Optional hand-maintained facts injected into the base system prompt.
markdown_path = "memory/MEMORY.md"

[agent]
max_iterations = 50
context_window = 50
context_tokens = 24000
# Empty means every built-in tool. Restrict this for a narrower agent.
enabled_tools = []

[telegram]
token = "${TELEGRAM_BOT_TOKEN}"
allowed_chats = [123456789]
poll_timeout = 30

[cron]
timezone = "local"

[[cron.jobs]]
name = "morning-brief"
schedule = "0 8 * * *"
prompt = "Review relevant recent activity and write a concise morning brief."
```

Use `./target/release/zero-hermes show-config` to inspect the resolved config.
`$VAR` and `${VAR}` references are expanded in provider credentials and
endpoint settings; a local `.env` is loaded before configuration.

### Request headers

`[provider.headers]` adds headers to every request from either client. Values
may reference environment variables, so a session id can stay in `.env`:

```toml
[provider.headers]
"x-opencode-session" = "${OPENCODE_SESSION_ID}"
```

This exists because gateways that route, cache and bill per conversation
reject a request that arrives without a session header — OpenCode Go and Zen
answer `400 MissingSessionID` — and increasingly refuse a client that
identifies as a generic SDK or HTTP library. zero-hermes therefore always
sends `user-agent: zero-hermes/<version>`; a `user-agent` here replaces it.

`base_url` may include the `/v1` suffix or not: each client appends
`/v1/messages` or `/v1/chat/completions` itself, so a trailing `/v1` (as
Ollama, NVIDIA NIM and OpenCode publish it) or a trailing slash is absorbed
rather than doubled.

## Memory, learning, and skills

zero-hermes has three complementary forms of memory:

- **Conversation memory:** CLI and Telegram histories are stored as durable,
  replayable transcripts in SQLite.
- **Recall:** the transcript text is indexed using SQLite FTS5. Relevant
  matches from previous sessions are labelled and supplied as reference
  context for a new turn.
- **Notes:** the `memory` tool stores durable key/value facts. The optional
  Markdown file is for facts you want to curate by hand.

Skills are ordinary local directories containing `SKILL.md`. The agent sees
an index of available skills and can use its `skill` tool to read one or save
a new reusable procedure. This makes learned behavior inspectable and easy to
edit or remove—there is no hidden training state.

## Subagents and scheduled work

The `subagent` tool runs a focused child agent with its own fresh history and
with `subagent` removed from its tool registry, so children cannot recursively
explode. A model can submit one task or up to four independent tasks; batches
run concurrently with a per-child timeout.

Cron jobs are configured in TOML and run in the gateway process:

```sh
./target/release/zero-hermes cron list
./target/release/zero-hermes cron check "*/15 * * * *"
```

## Interfaces

| Interface | Command | What it is for |
| --- | --- | --- |
| One turn | `zero-hermes run "…"` | Scripting and smoke checks |
| Terminal chat | `zero-hermes chat` | Interactive, streamed conversation |
| Telegram | `zero-hermes gateway` | Reach the Pi remotely through your bot |
| Local web UI | `zero-hermes web --bind 127.0.0.1:8088` | Browser chat with SSE streaming |
| Cron tools | `zero-hermes cron list` | Inspect configured automation |

Terminal chat supports `/help`, `/status`, `/tools`, `/clear`, and `/exit`.
`repl` remains an alias for `chat`.

## Security

Treat zero-hermes as an agent with shell access. A message that reaches its
`bash` tool can execute commands as the account running the binary.

- The Telegram gateway refuses to start with an empty `allowed_chats` list
  unless `allow_all_chats = true` is explicitly set.
- The web UI uses a per-process CSRF token. Keep it bound to loopback unless
  you put it behind your own authenticated reverse proxy.
- Restrict `[agent].enabled_tools` when you do not need shell or write access.
- Run it as a dedicated unprivileged Linux user. Do not give that user sudo,
  SSH keys, or access to files you would not want an agent to read.
- Review learned skills and persistent notes; both are local files/data and
  should be treated as untrusted input until you approve them.

## Project shape

```text
src/
  agent/       tool loop, providers, streaming, context compaction
  tools/       built-in shell, file, HTTP, memory, skill, and subagent tools
  channels/    Telegram adapter
  memory.rs    SQLite notes, transcripts, and FTS5 recall
  skills.rs    SKILL.md discovery and parsing
  cron.rs      scheduler
  web.rs       small Axum + SSE local UI
  main.rs      CLI and runtime wiring
skills/        local Hermes-compatible skill pack
assets/        project branding
```

## Development

The project follows the same order as CI:

```sh
cargo build --release
cargo test --release
cargo clippy --release -- -D warnings
cargo fmt --all -- --check
```

Useful smoke checks:

```sh
./target/release/zero-hermes --help
./target/release/zero-hermes run --mock "say hi"
./target/release/zero-hermes cron list
./target/release/zero-hermes tools --mock
```

## Scope

zero-hermes deliberately does not attempt to be the full Hermes Agent product:
there is no desktop application, MCP client, multi-platform messaging layer,
voice stack, remote execution backend, approval UI, or semantic memory model.
Those are worthwhile systems; this project optimizes for a compact agent you
can understand, operate, and afford to leave on a Raspberry Pi.

## License

MIT — see [LICENSE](LICENSE).
