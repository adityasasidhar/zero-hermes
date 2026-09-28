# AGENTS.md

Guidance for OpenCode sessions working in `zero-hermes`. The repo is a small
Rust crate (binary `zero-hermes` + library `zero_hermes`). Read this before
touching anything beyond a one-line change.

## Build / verify

CI order (`.github/workflows/ci.yml`) — run in this order locally too:

```sh
cargo build --release          # produces target/release/zero-hermes
cargo test --release           # all tests (unit + tests/*.rs integration)
cargo clippy --release -- -D warnings   # warnings are fatal in CI
cargo fmt --all -- --check     # formatting must be clean
```

Integration tests live in `tests/` and exercise the library crate directly
(`zero_hermes::agent`, `zero_hermes::tools::builtin`, `zero_hermes::web`,
`zero_hermes::channels::telegram`, `zero_hermes::cron`, `zero_hermes::memory`,
`zero_hermes::skills`).

Bench (binary size / cold start / idle RSS): `./bench/measure.sh 30`.

## CLI

Subcommands (see the `Command` enum in `src/main.rs`):

| subcommand       | what it does                                                                                                                 |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `gateway`        | Telegram long-poll + agent loop + cron scheduler                                                                             |
| `run "msg"`      | one agent turn against the configured provider, then exit                                                                    |
| `chat`           | multi-turn interactive chat on stdin, streaming (`repl` is a visible alias; `/help`, `/status`, `/tools`, `/clear`, `/exit`) |
| `cron list`      | print configured jobs and next firing time                                                                                   |
| `cron check E`   | parse a 5-field cron expr and print next firing                                                                              |
| `tools`          | list registered tool names (honours the loaded config)                                                                       |
| `init-config`    | write a default `zero_hermes.toml` to the user config dir                                                                    |
| `show-config`    | print the resolved config as TOML                                                                                            |
| `web --bind H:P` | axum + SSE web UI (default `127.0.0.1:8088`)                                                                                 |

Global flags: `--config <path>`, `--mock` (no network), `--stream` (token
streaming for `run`/`repl`). `--mock` is the only way to run the gateway
without a Telegram token — `gateway` bails if both are missing. It also
bails when `telegram.allowed_chats` is empty unless
`telegram.allow_all_chats = true`: the agent has a shell tool, so an open
bot is remote code execution and has to be opted into.

`gateway` and `web` both shut down cleanly on Ctrl-C.

## Config discovery

Precedence (`main` → `config::load`):

1. `--config <path>` flag
2. `ZERO_HERMES_CONFIG` env var
3. `~/.config/zero-hermes/zero_hermes.toml`
4. `./zero_hermes.toml`

A missing file is **not** an error — `Config::default()` is used silently.
`init-config --force` writes a sample. A leading `~` in `memory.path`,
`memory.markdown_path` or `skills_dir` is expanded via `util::expand_tilde`
at load time; TOML has no shell, so an unexpanded tilde would create a
literal `./~/` directory (a stray one was once committed — `.gitignore`
keeps `/~` out now).

Before the config is read, `main` loads `./.env` (via `util::load_dotenv`)
and `config::load` expands `$VAR` / `${VAR}` in `provider.api_key`,
`provider.base_url`, `provider.model` and `telegram.token`. Real
environment variables always win over `.env`, so credentials can stay out
of `zero_hermes.toml`.

Skills dir precedence (`bootstrap::skills_dir`):

1. `cfg.skills_dir`
2. `ZERO_HERMES_SKILLS_DIR` env var
3. `<config_dir>/skills`
4. `./skills`

Loader recursively walks the tree for `SKILL.md` / `skill.md`
(case-insensitive) (`SkillRegistry::load_dir`), so the nested Hermes
category layout loads as well as one-directory-per-skill. Duplicate
frontmatter `name:` values are kept under a directory-derived alias rather
than dropped, so registry size equals the number of `SKILL.md` files.
Frontmatter is a `key: value` block delimited by `---`.

## Provider

`[provider].kind` selects the wire format (`config::ProviderKind`,
`src/agent/provider.rs`):

- `anthropic` (default) — `/v1/messages`, `x-api-key` header. Default
  `base_url = https://api.minimax.io/anthropic`, default `model = MiniMax-M3`.
- `openai_compat` — `/v1/chat/completions`, `Authorization: Bearer`.
  Supports `temperature`.

Both clients set `connect_timeout_secs` / `read_timeout_secs` from config.
The read timeout is per-read, not a request deadline, so streaming still
works while a wedged provider cannot stall the gateway forever.

**Wire-format gotcha.** `Message::tool_results` stores results in a
*user*-role message (the Anthropic shape). `OpenAiCompat::build_body` has
to translate those into separate `role:"tool"` messages — handling only a
literal `"tool"` role silently dropped every tool result. Any test
covering that path must build history with `Message::tool_results`, not by
hand-writing `role: "tool"`.

`build_provider(&cfg.provider)` (`src/agent/provider.rs`) returns the
right impl from the kind. `MockProvider` (`src/agent/mock.rs`) is only
used for `--mock` and tests.

## Architecture map

```
src/
  main.rs              CLI dispatch only; the shared wiring lives in bootstrap.rs
  lib.rs               crate root: re-exports modules
  config.rs            Config + ProviderKind + default paths
  error.rs             anyhow alias only
  bootstrap.rs         shared wiring (skills, system prompt, tool registry);
                       lives in the lib so integration tests can reach it
  util.rs              truncate_bytes, expand_tilde, chunk_text, random_hex,
                       load_dotenv, expand_env_vars
  agent/
    mod.rs             the loop: agent::run + agent::run_stream + dispatch_tool
                       + RunLimits (max_iterations + context_window)
    provider.rs        LlmProvider trait, AnthropicMessages, OpenAiCompat, parse_response
    stream.rs          Anthropic + OpenAI SSE parsers, StreamEvent enum
    tool.rs            Tool / ToolCall / ToolResult / ToolOutput / Message / ContentBlock
    context.rs         compact_history: pair-safe oldest-drop compaction
    mock.rs            MockProvider (text_only / echo) for --mock and tests
  tools/
    mod.rs             ToolRegistry (insert respects [agent].enabled_tools allowlist;
                       insert_always is unconditional)
    builtin.rs         BashTool, ReadTool, WriteTool, FetchTool, MemoryTool,
                       SkillTool (read/write SKILL.md), SubAgentTool
  memory.rs            rusqlite (bundled): durable session transcripts, notes,
                       and an FTS5 `message_search` index for cross-session
                       recall; `Memory::in_memory()` for tests
  skills.rs            SKILL.md frontmatter parser + SkillRegistry
  cron.rs              cron crate (6-field) with normalize_cron() for 5-field input;
                       Scheduler emits CronEvent on a tokio mpsc. Fires *every*
                       job due at an instant, and honours [cron].timezone
  channels/
    mod.rs             Channel trait, InboundMessage, parse_command, strip_bot_mention
    telegram.rs        getUpdates long-poll + sendMessage
  web.rs               axum router: /, /events (SSE), /send, /health;
                       serves web/index.html (vanilla JS, EventSource).
                       /send requires the per-process CSRF token injected
                       into the page — a form POST needs no preflight, so
                       without it any site could drive the bash tool
skills/                the vendored Hermes-compatible skill pack
                       (~130 SKILL.md files); loaded recursively
web/index.html         single-file UI; embedded into the binary via include_str!
bench/measure.sh       binary size + cold-start + idle-RSS benchmark
Dockerfile             multi-stage; sets ZERO_HERMES_CONFIG=/config/zero_hermes.toml,
                       ZERO_HERMES_SKILLS_DIR=/opt/zero-hermes/skills; volumes /config, /data
```

The agent loop is `agent::run` and its streaming sibling
`agent::run_stream`. Both take a `RunLimits` and, each iteration, compact
`history` before calling the provider. Tool errors become `tool_result`
blocks with `is_error: true` so the model can recover — note that builtins
report failure as `Ok(ToolOutput::err(..))`, not `Err`, so `dispatch_tool`
must carry the flag across.

Compaction (`agent::compact_history`) only ever cuts forward to a plain
user turn. Cutting between an assistant `tool_use` and its `tool_result`
is a hard 400 from the Messages API, so when no safe boundary exists it
declines rather than corrupting the transcript. That means a single long
tool chain is not compacted — `max_iterations` already bounds it.

## Adding things

**New tool** — implement `Tool` (`src/agent/tool.rs`) and register in
`bootstrap::build_tool_registry`. Use `insert_always` for tools that
should always be exposed; `insert` honors `[agent].enabled_tools`
(empty list = all tools allowed).

**New LLM provider** — implement `LlmProvider` (`src/agent/provider.rs`)
and wire it into `build_provider`. Both `complete` and `stream` are required;
`LlmProvider::stream` has a default one-shot fallback you can use.

**Sub-agent gotcha** — `SubAgentTool` (`src/tools/builtin.rs`) holds an
`Arc<ToolRegistry>` of the parent's tools and clones them minus `subagent`
itself at call time, so it cannot recurse. If you add new tools to the
parent registry, they automatically become available to sub-agents.

## Conventions

- Rust 2021. No `unwrap()` / `expect()` outside tests — use `?` + `anyhow`.
  This matters more than usual because the release profile sets
  `panic = "abort"`: one panic anywhere kills the whole daemon rather than
  one task. Recover instead (`Mutex` poisoning, client construction).
- Errors: `Result<T>` alias over `anyhow::Error` lives in `src/error.rs`.
- Public items get doc comments (`//!` / `///`).
- Tools take `&str` in args, owned `String` only where needed; return
  `ToolOutput::ok(...)` / `ToolOutput::err(...)`.
- Release profile (`[profile.release]` in `Cargo.toml`) is size-optimized: `opt-level = "z"`,
  `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
  Don't relax these without a reason — the <15 MB / <50 ms / <10 MB RSS
  targets depend on them.
- All public async fns use `#[tokio::main]` for binaries,
  `#[tokio::test]` for tests. Anything that waits on a real schedule uses
  `#[tokio::test(start_paused = true)]` so tokio auto-advances its clock —
  the cron tests used to burn 60s of wall time each.
- Tracing filter env: `RUST_LOG=zero_hermes=info` (or override). Subscriber
  is initialised in `init_tracing` (`src/main.rs`).

## Constraints (worth preserving)

- **No git push.** Local repo only; the user pushes manually.
- `skills/` vendors the nested Hermes skill pack and is loaded recursively
  (`tests/skills_test.rs` asserts every `SKILL.md` registers). Keep the
  layout flat-or-nested as Hermes ships it, and prefer adding a skill
  directory over changing the loader. The stub directories from v0.1
  (`stub-1`, `stub-2`) were removed once the real pack landed.
- The Dockerfile's dep-cache stage must stub **every** target Cargo.toml
  declares (`src/main.rs`, `src/lib.rs`) plus `web/index.html`, and the
  build stage must `COPY web` — `src/web.rs` pulls the page in with
  `include_str!`.
- Don't add heavy deps (no `axum`-style web stack beyond what's there,
  no `sqlx`, `bson`, `prost`). The brief is explicit about this.
- `.hermes/environment.json` is for the Lemma recipe system; leave it
  alone (or update it if you change the build/test recipe).
- `BRIEF.md` is a one-shot build brief for an earlier session — historical
  context, not a live spec. The current source of truth is the code +
  `README.md`.

## Smoke checks before finishing

```sh
cargo build --release
./target/release/zero-hermes --help
./target/release/zero-hermes run --mock "say hi"     # uses MockProvider, no network
./target/release/zero-hermes chat --mock --no-banner  # then /tools, /exit
./target/release/zero-hermes cron list               # needs a config; uses defaults otherwise
./target/release/zero-hermes tools --mock            # honours [agent].enabled_tools
./target/release/zero-hermes web --bind 127.0.0.1:8088 &
curl -fsS http://127.0.0.1:8088/health                 # should print "ok"
curl -fsS -X POST http://127.0.0.1:8088/send -d 'message=hi'   # expect 403 (no CSRF token)
```

The bench script (`./bench/measure.sh 30`) gives you the binary size and
cold-start numbers called out in `README.md`.
