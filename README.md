# zero-hermes

A from-scratch Rust reimplementation of the Hermes Agent core — the agent
loop, not OpenClaw's shape. Same minimal-Rust-binary ethos as
[ZeroClaw](https://github.com/...) (single binary, &lt;15 MB, &lt;10 MB RAM,
&lt;50 ms cold start), aimed at the Hermes Agent feature surface.

## Goals

* **Small.** Single static binary, `cargo build --release` produces a
  &lt;15 MB executable. No system dependencies at runtime.
* **Fast.** Cold start in &lt;50 ms. The agent loop adds at most one HTTP
  round-trip per LLM turn.
| **Composable.** The agent loop is a 50-line function with a
  `LlmProvider` trait and a `ToolRegistry`. New providers and tools are
  one file each.
* **Multi-provider.** Ships with two `LlmProvider` implementations:
  the Anthropic Messages API (default, minimax-compatible) and any
  OpenAI-compatible `/v1/chat/completions` endpoint (OpenAI, Together,
  Groq, OpenRouter, llama.cpp, ollama, ...). Switch with
  `[provider] kind = "openai_compat"`.

## Status

v0.1.0 — initial implementation. Telegram gateway, agent loop, cron
scheduler, skills loader, SQLite memory are all in place. Skills are
**stubs only** — the loader works, the registry works, but no real skills
are bundled yet.

## Build

```sh
cargo build --release
```

The binary lands at `target/release/zero-hermes`.

## Run

### Single agent turn (CLI)

```sh
./target/release/zero-hermes run --mock "say hi"
```

With `--mock` no network calls are made; the agent loops with a fake
provider that returns a canned response. Without `--mock`, the binary
talks to the configured Anthropic Messages endpoint.

### Telegram gateway

```sh
./target/release/zero-hermes init-config --force
$EDITOR ~/.config/zero-hermes/zero_hermes.toml   # set api_key and telegram.token
./target/release/zero-hermes gateway
```

The gateway long-polls `getUpdates`, dispatches each inbound message to
the agent loop, and sends the response back via `sendMessage`.

### Cron management

```sh
./target/release/zero-hermes cron list      # list configured jobs + next firing
./target/release/zero-hermes cron check "*/5 * * * *"
```

### Web UI

```sh
./target/release/zero-hermes web --bind 127.0.0.1:8088
```

Then open `http://127.0.0.1:8088/` in a browser. The page is a single HTML
file with a tiny vanilla-JS `<script>` that uses `EventSource` to render
streaming tokens. No JS framework, no build step.

## Sample config (`~/.config/zero-hermes/zero_hermes.toml`)

`kind` selects the wire format:

```toml
# --- Anthropic Messages (default; minimax uses this) ---
[provider]
kind       = "anthropic"
base_url   = "https://api.minimax.io/anthropic"
api_key    = "sk-..."
model      = "MiniMax-M3"
max_tokens = 8192

# --- OR: any OpenAI-compatible endpoint ---
# [provider]
# kind       = "openai_compat"
# base_url   = "https://api.openai.com"            # or api.together.xyz, api.groq.com, ...
# api_key    = "sk-..."
# model      = "gpt-4o-mini"
# max_tokens = 4096
# temperature = 0.7                                # openai-compat-only

[telegram]
token          = "123:abc"
poll_timeout   = 30
allowed_chats  = []
allowed_commands = []

[memory]
path = "~/.local/share/zero-hermes/memory.sqlite"

[agent]
max_iterations = 10
context_window = 50
enabled_tools  = []

[[cron.jobs]]
name = "status"
schedule = "*/15 * * * *"
prompt = "summarise recent activity"
```

## Architecture

```
src/
├── main.rs          # CLI: gateway | run | cron | tools | init-config | show-config
├── config.rs        # zero_hermes.toml loading + defaults
├── error.rs         # anyhow Result alias
├── agent/
│   ├── mod.rs       # the loop: prompt -> call LLM -> dispatch tools -> repeat
│   ├── context.rs   # message list + compaction (drop oldest, keep last N)
│   ├── provider.rs  # LlmProvider trait + AnthropicMessages + OpenAiCompat impls + build_provider()
│   └── tool.rs      # Tool trait + Anthropic-flavoured ToolCall/Result
├── tools/
│   ├── mod.rs       # ToolRegistry: name -> Arc<dyn Tool>
│   └── builtin.rs   # BashTool, ReadTool, WriteTool, FetchTool, MemoryTool
├── memory.rs        # sqlite (rusqlite) sessions + episodic notes
├── skills.rs        # SKILL.md loader (frontmatter + body), registry
├── cron.rs          # cron expr parser + tokio scheduler
└── channels/
    ├── mod.rs       # Channel trait + command parsing
    └── telegram.rs  # long-poll getUpdates -> agent -> sendMessage
```

## The agent loop

```rust
pub async fn run<P: LlmProvider + ?Sized>(
    llm: &P,
    tools: &ToolRegistry,
    system: Option<&str>,
    history: &mut Vec<Message>,
    user: &str,
    max_iterations: usize,
    ctx: &ToolContext,
) -> Result<String>
```

1. Append `user` to `history`.
2. Loop up to `max_iterations` times:
   a. Call `llm.complete(messages, tool_schemas)`.
   b. If text returns, append to history and return.
   c. If tool calls return, dispatch each through the registry, append
      the assistant + tool_result messages, continue.
3. Errors from tools are reported as `tool_result` with `is_error: true`.

## Adding a tool

```rust
struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "mytool" }
    fn description(&self) -> &str { "does my thing" }
    fn schema(&self) -> Value { json!({"type": "object"}) }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        Ok(ToolOutput::ok("ok"))
    }
}

// then in main.rs:
reg.insert_always(Arc::new(MyTool));
```

## Tests

```sh
cargo test
```

Covers the agent loop dispatch, tool registry, memory round-trip,
SKILL.md loader, cron expression parsing, and Telegram message parsing.

## Benchmarks (vs Hermes Agent baseline)

| Metric                      | Hermes Agent baseline | `zero-hermes` target |
| --------------------------- | --------------------- | -------------------- |
| Binary size                 | ~250 MB               | &lt;15 MB            |
| Resident memory at idle     | ~200 MB               | &lt;10 MB            |
| Cold start                  | ~1.5 s                | &lt;50 ms            |
| Single-turn agent latency   | depends on model      | 1 HTTP round-trip    |
| Lines of code (this crate)  | n/a                   | &lt;3000 LOC         |

## License

MIT.
