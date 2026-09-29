# AGENTS.md

**zero-hermes** — a small, self-hosted Hermes-inspired agent that runs in one
Rust binary on low-power hardware (target: Raspberry Pi). It bundles a
tool-calling loop, SQLite memory, skills, cron, sub-agents, a Telegram
gateway and a local axum web UI. Guidance for agents working in this repo;
read it before touching anything beyond a one-line change.

## Tech stack

- Rust 2021, single cargo crate: binary `zero-hermes` (`src/main.rs`) +
  library `zero_hermes` (`src/lib.rs`).
- tokio (async runtime), reqwest + rustls (providers), axum + tokio-stream
  (web UI / SSE), rusqlite `bundled` (memory), clap (CLI), cron, serde.
- cargo is the only build/package manager — there is no Node or Python step.

## Layout (top level)

`src/` agent loop, providers, tools, channels, web; `tests/` integration
tests; `skills/` vendored Hermes skill pack (loaded recursively); `web/`
single-file UI embedded via `include_str!`; `assets/` logos; `bench/`
size/cold-start/RSS script; `.github/workflows/` CI. Full map under
[Architecture map](#architecture-map).

## CI

GitHub Actions — `.github/workflows/ci.yml`, on push to `main`/`master` and
every pull request:

```sh
cargo build --release
cargo test --release
cargo clippy --release -- -D warnings   # warnings are fatal
cargo fmt --all -- --check
```

## Build / verify

Run in the same order locally:

```sh
cargo build --release          # produces target/release/zero-hermes
cargo test --release           # all tests (unit + tests/*.rs integration)
cargo clippy --release -- -D warnings   # warnings are fatal in CI
cargo fmt --all -- --check     # formatting must be clean
```

Integration tests live in `tests/` and exercise the library crate directly
(`zero_hermes::agent`, `zero_hermes::tools::builtin`, `zero_hermes::web`,
`zero_hermes::channels::telegram`, `zero_hermes::cron`, `zero_hermes::memory`,
`zero_hermes::skills`). Four of them cover ground the library tests cannot
reach:

- `cli_test.rs` spawns the real binary in a throwaway `HOME` (via
  `env_clear()` + `XDG_*`), so config discovery, the safety gates and the web
  socket are exercised as a user would hit them. `src/main.rs` is otherwise
  invisible to every other test.
- `provider_http_test.rs` runs both providers against a local mock HTTP server
  to cover URLs, auth headers, status/parse failures, read timeouts and the
  streaming path end to end.
- `agent_stream_test.rs` drives `run_stream` with a scripted event stream
  (tool dispatch, think-block filtering, retry-before-output only).
- `builtin_tools_test.rs` covers the tools' failure branches, the output caps
  and the skills-root boundary (a model-supplied skill name must never write
  outside the root).

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

**Headers and `base_url` normalisation.** `[provider.headers]` is a
`HashMap<String, String>` applied by `apply_headers` to every request, and
expanded from the environment in `Config::expand_env`. It is parsed in
`build_headers` at provider construction, not at request time: release builds
set `panic = "abort"`, so a header name that dies inside reqwest's builder
would take the daemon with it. `apply_headers` also sends
`user-agent: zero-hermes/<CARGO_PKG_VERSION>` unless the config sets one,
because gateways reject a bare SDK/HTTP-library user agent (OpenCode asks
clients to name themselves and to send `x-opencode-session`, a stable
per-conversation id; without it it answers `400 MissingSessionID`).

`base_url` goes through `normalize_base_url`, which strips a trailing `/` *and*
a trailing `/v1`. Both clients append `/v1/...` themselves, and the README's own
Ollama example is `http://127.0.0.1:11434/v1`, so without the strip that config
would request `/v1/v1/chat/completions`. A bare `base_url = "/v1"` is left
alone so the mistake stays visible.

**Wire-format gotcha.** `Message::tool_results` stores results in a
*user*-role message (the Anthropic shape). `OpenAiCompat::build_body` has
to translate those into separate `role:"tool"` messages — handling only a
literal `"tool"` role silently dropped every tool result. Any test
covering that path must build history with `Message::tool_results`, not by
hand-writing `role: "tool"`.

**SSE parser gotchas.** Three things in `src/agent/stream.rs` are load-bearing,
and each was a real bug once:

- `buffer` is `Vec<u8>`, never `String`. A network read can end mid-codepoint,
  so `String::from_utf8_lossy` per chunk turns every split multi-byte
  character into U+FFFD. Splitting on the raw `\n` byte (which cannot occur
  inside a multi-byte sequence) and decoding whole *lines* is lossless.
- Once `finished` is set — `message_stop` for Anthropic, `[DONE]` for
  OpenAI — the stream must return `None` even if the peer leaves the body
  open. Otherwise a keep-alive connection strands the consumer forever.
- `run_stream` must honour text delivered in a `Done` event with no preceding
  deltas: that is exactly what the trait's default `one_shot_stream` produces
  for a provider without real SSE support, and dropping it made every such
  provider look like it returned an empty answer.

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
  mcp.rs               MCP stdio client (JSON-RPC 2.0 initialize / tools/list /
                       tools/call). McpTool wraps a remote tool as
                       mcp_<server>_<tool>; discovery is async and NOT yet wired
                       into the sync registry — see the TODO in bootstrap.rs
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
    builtin.rs         ~20 Tool impls: bash, read/write/edit/search_files,
                       fetch, web_search, web_extract, memory, cron, todo, ask,
                       message, execute_command, execute_code, read_file,
                       write_file, skill, skill_view, subagent. Hermes-compat
                       aliases (execute_command/execute_code/read_file/
                       write_file/search_files/skill_view/web_search/
                       web_extract) are registered so vendored skills resolve
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
  web.rs               axum router: /, /events (SSE), /send, /health and one
                       static route per entry in ASSETS; serves web/index.html
                       (vanilla JS, EventSource).
                       /send requires the per-process CSRF token injected
                       into the page — a form POST needs no preflight, so
                       without it any site could drive the bash tool
skills/                the vendored Hermes-compatible skill pack
                       (~130 SKILL.md files); loaded recursively
web/index.html         single-file UI; embedded into the binary via include_str!
assets/                zero_hermes_logo.png is the full-colour master (README
                       hero only); hermes-mark.png (192px) and
                       hermes-favicon.png (32px) are the greyscaled,
                       head-cropped derivatives the UI serves via
                       include_bytes! — brand mark, welcome mark, assistant
                       avatar, working row, favicon, mobile topbar
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
  declares (`src/main.rs`, `src/lib.rs`) plus every file pulled in by
  `include_str!`/`include_bytes!` — currently `web/index.html`,
  `assets/hermes-mark.png` and `assets/hermes-favicon.png` — and the build
  stage must `COPY web` and `COPY assets`. A missing stub fails the *dep*
  build, not the real one, so it looks like a cache problem.
- **The web UI is strictly achromatic** — black, white and greys only, in
  both themes. Every token in `web/index.html`'s `:root` block must have
  `r == g == b`; state (live / busy / error / in-flight) is carried by value
  and *treatment* instead of hue, e.g. a filled status dot vs a hollow ring, a
  2px solid border on an error vs a dashed one on a running badge. Re-deriving
  a coloured accent breaks the whole point, so check a change by grepping for
  `#hex`/`rgb(` outside the token block.
- The Hermes mark is greyscaled and head-cropped, not tinted: it is a
  greyscale PNG so one file serves both themes, and it is embedded
  (`include_bytes!`) rather than read from disk so the binary stays
  self-contained. Adding a UI image means adding it to `ASSETS` in
  `src/web.rs` — the path there is the route, so no catch-all and no
  traversal surface. Keep derivatives small: the release profile is
  size-optimized and the binary budget is <15 MB.
- **`bash`/`execute_code` timeouts must kill the process tree, not just the
  shell.** `run_with_timeout` starts the child with `process_group(0)` and
  `kill_process` signals the negative pid (the whole group). `/bin/sh -c "…"`
  *forks* rather than execs, so SIGKILLing only the shell orphans the real
  work; the orphan then inherits the stdout/stderr pipes, keeping the reader
  threads blocked, so a `timeout: 1` call still took the full 30 seconds.
- **Every tool that GETs a model-supplied URL needs the SSRF guard.**
  `is_fetch_blocked` rejects non-`http(s)` schemes, loopback/private/
  link-local hosts, `localhost`/`.local`/`.internal`, obscured numeric IPv4
  spellings (`is_obscured_ipv4`: `2130706433`, `0x7f000001`, `0177.0.0.1`),
  canonical addresses via `std::net` (`is_blocked_ip_addr`) and
  IPv4-mapped/translated IPv6 (`is_blocked_mapped_ipv4`: `[::ffff:127.0.0.1]`,
  `[::ffff:7f00:1]` — note `Ipv6Addr::is_loopback()` is *false* for these, so
  `std::net` alone does not catch them). `FetchTool` and `WebExtractTool`
  both call it and both expose `with_private_allowed()` for tests that need a
  local server — without that seam the HTML path is untestable, since every
  local server is blocked. Redirects are disabled outright
  (`redirect::Policy::none()`), so a 302 to `169.254.169.254` cannot slip
  past the initial-URL check. **What is still not covered:** no DNS
  resolution happens, so a *name* that resolves to a private address gets
  through — closing that needs resolving and re-checking the result, or
  pinning the connection to the checked address.
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
curl -fsS -o /dev/null -w '%{http_code} %{content_type}\n' \
  http://127.0.0.1:8088/assets/hermes-mark.png          # expect 200 image/png
curl -fsS -X POST http://127.0.0.1:8088/send -d 'message=hi'   # expect 403 (no CSRF token)
```

The bench script (`./bench/measure.sh 30`) gives you the binary size and
cold-start numbers called out in `README.md`.
