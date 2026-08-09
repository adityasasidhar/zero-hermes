# Brief: build zero-hermes

You are OpenCode, given full autonomy to build the project end-to-end. Do not stop after a stub or scaffold — keep going until `cargo build --release` succeeds and at least the loop unit tests pass.

## What this is

A from-scratch Rust reimplementation of the Hermes Agent core (the agent loop, not OpenClaw's shape). Think of it as "ZeroClaw but for Hermes Agent" — same minimal-Rust-binary ethos (single binary, <15 MB, <10 MB RAM, <50 ms cold start), aimed at the Hermes Agent feature surface.

Reference for "minimal agent loop in few lines of Python":
`/home/arctic/projects/claude-code-in-100-lines/src/` — 210 LOC total (main.py:32, llm.py:60, tools.py:56, memory.py:30, loader.py:32).

Reference for Hermes Agent target surface:
`~/.hermes/` — read SOUL.md, CLAUDE.md, config.yaml to understand the agent behavior to port.

## Scope (locked in advance by the user)

- **Mid-weight**: Telegram gateway + agent loop + cron scheduler + skills loader (stubbed, no bundled skills yet) + SQLite memory. NO web UI, NO multi-provider router in v1.
- **One LLM provider**: openai-compatible HTTP (works with minimax at `https://api.minimax.io/anthropic` — adapt the request shape; check the minimax config in `~/.openclaw/openclaw.json` for the exact request shape).
- **Skills loader stubbed** — just SKILL.md parser + registry + a few hard-coded fake skills for the loader test. No real skills bundled yet.
- **No git push** — local git init only. Do NOT push to GitHub.
- **Tests required**: `cargo test` must pass. At minimum: agent loop dispatch, tool registry, memory round-trip, cron expression parsing, SKILL.md loader, Telegram message parse.

## Target architecture

```
zero-hermes/
├── Cargo.toml          # one binary crate, name = zero-hermes
├── src/
│   ├── main.rs         # CLI: zero-hermes gateway | run "msg" | cron list
│   ├── config.rs       # zero_hermes.toml loading + defaults
│   ├── error.rs        # anyhow-style Result alias
│   ├── agent/
│   │   ├── mod.rs      # the loop: prompt -> call LLM -> dispatch tools -> repeat
│   │   ├── context.rs  # message list + compaction (drop oldest, keep system + last N)
│   │   ├── provider.rs # LlmProvider trait + AnthropicMessages impl (minimax-compatible)
│   │   └── tool.rs     # Tool trait + ToolCall parsing
│   ├── tools/
│   │   ├── mod.rs      # ToolRegistry: name -> Arc<dyn Tool>
│   │   └── builtin.rs  # BashTool, ReadTool, WriteTool, FetchTool, MemoryTool
│   ├── memory.rs       # sqlite (rusqlite) sessions + episodic notes
│   ├── skills.rs       # SKILL.md loader (frontmatter + body), registry
│   ├── cron.rs         # cron expr parser + tokio scheduler
│   └── channels/
│       ├── mod.rs      # Channel trait
│       └── telegram.rs # long-poll getUpdates -> agent -> sendMessage
├── skills/             # 2 stub SKILL.md files for the loader test
│   ├── stub-1/SKILL.md
│   └── stub-2/SKILL.md
├── tests/
│   ├── loop_test.rs
│   ├── tools_test.rs
│   ├── memory_test.rs
│   ├── skills_test.rs
│   └── cron_test.rs
├── bench/              # empty for now, just a README
├── README.md
├── .gitignore          # target/, *.swp, .DS_Store
└── BRIEF.md            # this file
```

## Dependencies (Cargo.toml)

Keep it lean. Use:
- `tokio` (rt-multi-thread, macros, net, time, sync) — async runtime
- `reqwest` (json, rustls-tls, stream) — LLM HTTP
- `serde` + `serde_json` — message/tool serialization
- `rusqlite` (bundled) — SQLite, no system dep
- `cron` — cron expression parser
- `chrono` — timestamps
- `clap` (derive) — CLI subcommands
- `anyhow` + `thiserror` — errors
- `tracing` + `tracing-subscriber` — logs
- `dirs` — find config dir

NO `async-tungstenite`, NO `actix`, NO `axum` — keep it stdlib+tokio. Use `std::process::Command` for bash tool (no async-shell crate).

## The agent loop (the heart of this)

Single function `agent::run(llm: &impl LlmProvider, tools: &ToolRegistry, system: &str, history: &mut Vec<Message>, user: &str) -> Result<String>`:

1. Append user message to history.
2. Loop:
   a. Call `llm.complete(messages, tool_schemas)`. Get back either an assistant text or a list of tool_use blocks.
   b. If text -> append to history, return text.
   c. If tool_use blocks -> for each: dispatch via ToolRegistry, collect results, append as tool_result messages, continue loop.
   d. Cap iterations at 10 (safety).
3. Errors from tools -> append as tool_result with `is_error: true`, continue.

The LlmProvider trait needs ONE impl: `AnthropicMessages` that POSTs to a configurable base_url (default `https://api.minimax.io/anthropic`) with an `x-api-key` header (matching the format in `~/.openclaw/openclaw.json`).

## Verification checklist (must all be true before reporting done)

- `cargo build --release` succeeds, emits `target/release/zero-hermes`
- `cargo test` passes (all tests)
- `cargo clippy -- -D warnings` clean (or document warnings)
- `./target/release/zero-hermes --help` shows gateway | run | cron subcommands
- `./target/release/zero-hermes run "say hi"` runs the agent loop against a mock provider in --mock mode (add a --mock flag for tests)
- `./target/release/zero-hermes cron list` parses a sample cron expr
- File counts: `find src -name '*.rs' | xargs wc -l` should be under 3000 LOC
- `du -h target/release/zero-hermes` < 15 MB
- Update README.md with: what it is, how to build, how to run gateway vs run, sample config, RAM/size benchmarks vs Hermes Agent baseline.

## Style rules

- Rust 2021 edition. `cargo fmt` clean.
- No `unwrap()` outside tests. Use `?` + `anyhow`.
- Public items get doc comments.
- Tool implementations: each tool is `Arc<dyn Tool>` with `fn name(&self) -> &str`, `fn schema(&self) -> Value`, `async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>`.
- Use `#[tokio::main]` for the gateway.
- Prefer `&str` over `String` in function args, owned `String` only where needed.

## What NOT to do

- Don't push to GitHub (user will handle).
- Don't add a web UI / control UI.
- Don't bundle real skills — stubs only.
- Don't add multi-provider routing — single AnthropicMessages impl is enough.
- Don't pull in heavy crates (bson, prost, sqlx, axum, etc.).
- Don't leave TODOs in the code — implement or remove.
- Don't write a single huge file — split per the layout above.

## Working directory

`/home/arctic/projects/zero-hermes/` — already initialized as a git repo.

After everything works: `git add -A && git commit -m "feat: initial zero-hermes implementation"`. Do not push.

Report back with:
- Final file tree
- LOC count
- cargo test output
- binary size
- any caveats
