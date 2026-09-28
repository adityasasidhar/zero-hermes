//! `zero-hermes` — minimal Rust reimplementation of Hermes Agent core.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};

use zero_hermes::agent::mock::MockProvider;
use zero_hermes::agent::{build_provider, LlmProvider, Message, RunLimits};
use zero_hermes::bootstrap::{build_system_prompt_with_memory, build_tool_registry, load_skills};
use zero_hermes::channels::{telegram, Channel, InboundMessage};
use zero_hermes::config::Config;
use zero_hermes::error::Result;
use zero_hermes::memory::{recall_section, Memory};

/// Add a small, explicitly labelled recall section to a turn's system prompt.
/// Stored conversation is data, not instructions; the label helps the model
/// distinguish it from the active user request.
///
/// The current session is excluded so recall surfaces *other* conversations
/// instead of echoing back what was just said. Recall is injected every turn
/// so the agent sees past context without having to ask via the memory tool
/// first (partial auto-memory; a full background auto-summarizer that calls
/// the LLM after each turn is deferred — TODO Wave E — because it needs an
/// async provider call outside the turn loop).
fn system_with_recall(
    base: &str,
    memory: &Memory,
    query: &str,
    exclude_session: Option<&str>,
) -> String {
    let Some(section) = recall_section(memory, query, exclude_session, 4) else {
        return base.to_string();
    };
    format!("{base}\n\n{section}")
}

/// Whether a telegram slash command resets the chat session locally.
fn is_reset_command(cmd: &str) -> bool {
    matches!(cmd, "new" | "clear" | "reset" | "start")
}

/// Help text sent for `/help` without calling the LLM.
fn gateway_help_text() -> &'static str {
    "Commands:\n/new or /clear — reset this chat's conversation history\n/help — show this help"
}

/// Drain the `message`-tool outbox (`outbox:<session>`) after a successful turn.
///
/// Reads the note written by `MessageTool`, sends each queued entry via the
/// reply channel (per-entry `chat_id` override wins, else the current chat),
/// then deletes the note. Missing/blank notes are a no-op; a corrupt note is
/// logged and deleted so one bad write cannot poison every future turn.
async fn drain_outbox(
    memory: &Memory,
    reply: &Arc<dyn Channel>,
    session_id: &str,
    default_chat_id: &str,
) {
    let key = zero_hermes::tools::builtin::outbox_key(session_id);
    let raw = match memory.read_note(&key) {
        Ok(raw) => raw,
        Err(e) => {
            tracing::warn!(error = %e, "reading outbox note failed");
            return;
        }
    };
    let Some(raw) = raw else { return };
    if raw.trim().is_empty() {
        return;
    }
    let entries = match zero_hermes::tools::builtin::parse_outbox_entries(Some(&raw)) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(error = %e, "outbox note is corrupt; dropping it");
            if let Err(e) = memory.delete_note(&key) {
                tracing::warn!(error = %e, "deleting corrupt outbox note failed");
            }
            return;
        }
    };
    if entries.is_empty() {
        if let Err(e) = memory.delete_note(&key) {
            tracing::warn!(error = %e, "deleting empty outbox note failed");
        }
        return;
    }
    for entry in &entries {
        let target = entry.chat_id.as_deref().unwrap_or(default_chat_id);
        if let Err(e) = reply.send(target, &entry.text).await {
            tracing::warn!(error = %e, "outbox send failed");
        }
    }
    if let Err(e) = memory.delete_note(&key) {
        tracing::warn!(error = %e, "deleting drained outbox note failed");
    }
}

#[derive(Parser, Debug)]
#[command(name = "zero-hermes", version, about = "Minimal Hermes Agent in Rust")]
struct Cli {
    /// Path to the config file (default: $XDG_CONFIG_HOME/zero-hermes/zero_hermes.toml).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Use a mock provider (no network calls). Useful for `run` smoke testing.
    #[arg(long, global = true)]
    mock: bool,

    /// Stream LLM tokens to stdout as they arrive (used by `run`; chat always streams).
    #[arg(long, global = true)]
    stream: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the Telegram gateway (long-poll + agent loop).
    Gateway,
    /// Run a single agent turn against the configured provider and exit.
    Run {
        /// The user message to send.
        message: String,
    },
    /// Start an interactive multi-turn terminal chat.
    #[command(visible_alias = "repl")]
    Chat {
        /// Suppress the interactive welcome banner.
        #[arg(long)]
        no_banner: bool,
    },
    /// Manage cron jobs.
    Cron {
        #[command(subcommand)]
        action: CronAction,
    },
    /// List the tools registered in the default registry.
    Tools,
    /// Initialise a default config in the user config directory.
    InitConfig {
        /// Force overwrite if the config already exists.
        #[arg(long)]
        force: bool,
    },
    /// Print the resolved configuration to stdout.
    ShowConfig,
    /// Run the minimal web UI (axum + SSE) on 127.0.0.1:8088.
    Web {
        /// Bind address as host:port (default 127.0.0.1:8088).
        #[arg(long)]
        bind: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum CronAction {
    /// List jobs and their next firing time.
    List,
    /// Print a sample cron expression's next firing time.
    Check {
        /// The 5-field cron expression to evaluate.
        expr: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    // Load `./.env` before the config so `api_key = "${MINIMAX_API_KEY}"`
    // resolves. Real environment variables always win.
    let loaded = zero_hermes::util::load_dotenv(std::path::Path::new(".env"));
    if !loaded.is_empty() {
        tracing::debug!(vars = ?loaded, "loaded .env");
    }

    let cfg_path = cli
        .config
        .clone()
        .or_else(|| std::env::var("ZERO_HERMES_CONFIG").ok().map(PathBuf::from))
        // `config_file()` returns a path whether or not it exists, so it has
        // to be filtered on existence. Without the filter the documented
        // `./zero_hermes.toml` fallback was unreachable on any machine with a
        // home directory: the missing `~/.config/.../zero_hermes.toml` won,
        // and `config::load` quietly used defaults instead of stepping down.
        .or_else(|| zero_hermes::config::config_file().filter(|p| p.exists()))
        .unwrap_or_else(|| PathBuf::from("zero_hermes.toml"));
    let cfg = zero_hermes::config::load(&cfg_path).context("loading config")?;

    match cli.command {
        Command::Gateway => run_gateway(cfg, cli.mock).await,
        Command::Run { message } => run_once(cfg, &message, cli.mock, cli.stream).await,
        Command::Chat { no_banner } => run_chat(cfg, cli.mock, no_banner).await,
        Command::Cron { action } => match action {
            CronAction::List => cron_list(&cfg),
            CronAction::Check { expr } => cron_check(&expr),
        },
        Command::Tools => tools_list(&cfg),
        Command::InitConfig { force } => init_config(force),
        Command::ShowConfig => {
            let s = toml::to_string_pretty(&cfg).unwrap_or("<unparseable>".into());
            println!("{s}");
            Ok(())
        }
        Command::Web { bind } => run_web(cfg, cli.mock, bind).await,
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("zero_hermes=info,info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

async fn run_once(cfg: Config, message: &str, mock: bool, stream: bool) -> Result<()> {
    let skills = load_skills(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::text_only("hello from mock"))
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };
    let registry = build_tool_registry(&cfg, provider.clone());
    let system = build_system_prompt_with_memory(&cfg, &skills, Some(&memory));
    let ctx = zero_hermes::agent::make_context_with_memory(None, memory);

    let mut history: Vec<Message> = Vec::new();
    let out = if stream {
        zero_hermes::agent::run_stream(
            provider.as_ref(),
            registry.as_ref(),
            Some(&system),
            &mut history,
            message,
            RunLimits::from(&cfg.agent),
            &ctx,
            |ev| match ev {
                zero_hermes::agent::StreamTurn::TextDelta(s) => {
                    print!("{s}");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
                zero_hermes::agent::StreamTurn::ToolUse(tc) => {
                    println!("\n[tool: {}({})]", tc.name, tc.input);
                }
                zero_hermes::agent::StreamTurn::ToolResult {
                    name,
                    output,
                    is_error,
                } => {
                    let tag = if is_error { "error" } else { "ok" };
                    println!(
                        "[tool: {name} -> {tag}] {}",
                        zero_hermes::util::truncate_bytes(&output, 400)
                    );
                }
                zero_hermes::agent::StreamTurn::Done(_) => {
                    println!();
                }
            },
        )
        .await
    } else {
        zero_hermes::agent::run(
            provider.as_ref(),
            registry.as_ref(),
            Some(&system),
            &mut history,
            message,
            RunLimits::from(&cfg.agent),
            &ctx,
        )
        .await
    }?;
    if !stream {
        println!("{out}");
    }
    Ok(())
}

/// Interactive multi-turn terminal chat. Reads lines from stdin until EOF or `/exit`.
/// History compounds across turns; the provider streams tokens to stdout.
async fn run_chat(cfg: Config, mock: bool, no_banner: bool) -> Result<()> {
    let skills = load_skills(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::echo())
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };
    let registry = build_tool_registry(&cfg, provider.clone());
    let system = build_system_prompt_with_memory(&cfg, &skills, Some(&memory));
    let session_id = "cli-default";
    let mut ctx = zero_hermes::agent::make_context_with_memory(None, memory.clone());
    ctx.session_id = Some(session_id.into());
    if let Err(e) = memory.upsert_session(session_id, "cli", "default") {
        tracing::warn!(error = %e, "upsert_session failed");
    }
    let mut history = memory.load_history(session_id)?;
    let stdin = std::io::stdin();
    let mut buf = String::new();
    let interactive = {
        use std::io::IsTerminal;
        std::io::stdin().is_terminal()
    };
    if interactive && !no_banner {
        print_chat_banner(&cfg, mock, registry.names().len());
    }
    loop {
        print!("you › ");
        use std::io::Write;
        std::io::stdout().flush().ok();
        buf.clear();
        let n = stdin.read_line(&mut buf)?;
        if n == 0 {
            println!();
            break;
        }
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "/exit" | "/quit" | "/q" => {
                if interactive {
                    println!("\nbye");
                }
                break;
            }
            "/clear" | "/reset" => {
                history.clear();
                // Reset the durable transcript as well; without this the next
                // delta-append would resurrect the "cleared" prefix.
                if let Err(e) = memory.save_history(session_id, &history) {
                    tracing::warn!(error = %e, "resetting chat history failed");
                }
                println!("  chat history cleared");
                continue;
            }
            "/help" | "/h" => {
                print_chat_help();
                continue;
            }
            "/tools" => {
                print_chat_tools(&registry.names());
                continue;
            }
            "/status" => {
                println!(
                    "  provider: {}  model: {}  tools: {}  turns: {}",
                    if mock { "mock" } else { "configured" },
                    cfg.provider.model,
                    registry.names().len(),
                    history.iter().filter(|m| m.role == "user").count(),
                );
                continue;
            }
            other if other.starts_with('/') => {
                println!("(unknown command: {other})");
                continue;
            }
            _ => {}
        }
        let mut answer_started = false;
        let len_before = history.len();
        let result = zero_hermes::agent::run_stream(
            provider.as_ref(),
            registry.as_ref(),
            Some(&system_with_recall(
                &system,
                memory.as_ref(),
                line,
                Some(session_id),
            )),
            &mut history,
            line,
            RunLimits::from(&cfg.agent),
            &ctx,
            |ev| match ev {
                zero_hermes::agent::StreamTurn::TextDelta(s) => {
                    if !answer_started {
                        print!("zero-hermes › ");
                        answer_started = true;
                    }
                    print!("{s}");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
                zero_hermes::agent::StreamTurn::ToolUse(tc) => {
                    if answer_started {
                        println!();
                    }
                    println!("  ↳ using {} {}", tc.name, tc.input);
                }
                zero_hermes::agent::StreamTurn::ToolResult {
                    name,
                    output,
                    is_error,
                } => {
                    let tag = if is_error { "error" } else { "ok" };
                    println!(
                        "  ↳ {name}: {tag} — {}",
                        zero_hermes::util::truncate_bytes(&output, 400)
                    );
                }
                zero_hermes::agent::StreamTurn::Done(_) => {
                    if answer_started {
                        println!();
                    }
                }
            },
        )
        .await;
        match result {
            Ok(_) => {
                // Append only the delta: the agent loop compacts `history`
                // in place, so saving the whole vector would delete the
                // durable prefix that compaction trimmed.
                let start = len_before.min(history.len());
                if let Err(e) = memory.append_messages(session_id, &history[start..]) {
                    tracing::warn!(error = %e, "saving chat history failed");
                }
            }
            Err(e) => {
                eprintln!("(error: {e})");
                // Pop the orphan trailing user turn so the next turn doesn't
                // start with `user,user`, then persist the repaired history.
                zero_hermes::agent::repair_history_after_failure(&mut history);
                let start = len_before.min(history.len());
                if start < history.len() {
                    if let Err(e) = memory.append_messages(session_id, &history[start..]) {
                        tracing::warn!(error = %e, "saving chat history after failure failed");
                    }
                }
            }
        }
    }
    Ok(())
}

/// Print the interactive chat welcome banner.
fn print_chat_banner(cfg: &Config, mock: bool, tool_count: usize) {
    println!("zero-hermes chat");
    println!(
        "{} · {} · {tool_count} tools",
        if mock {
            "mock provider"
        } else {
            "configured provider"
        },
        if mock { "mock" } else { &cfg.provider.model },
    );
    println!("Type /help for commands. Ctrl-D or /exit to leave.\n");
}

/// Print the slash commands available during an interactive chat.
fn print_chat_help() {
    println!("  /help       show these commands");
    println!("  /status     show model, tool, and session details");
    println!("  /tools      list available tools");
    println!("  /clear      reset this chat's conversation history");
    println!("  /exit       leave chat (aliases: /quit, /q)");
}

/// Print registered tool names in a compact, terminal-friendly list.
fn print_chat_tools(names: &[String]) {
    if names.is_empty() {
        println!("  no tools are enabled");
        return;
    }
    println!("  tools: {}", names.join(", "));
}

async fn run_web(cfg: Config, mock: bool, bind: Option<String>) -> Result<()> {
    let skills = load_skills(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::echo())
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };
    let registry = build_tool_registry(&cfg, provider.clone());
    let system = build_system_prompt_with_memory(&cfg, &skills, Some(&memory));

    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    // Resume the durable web transcript so a restart does not lose it; new
    // turns append their delta (see `web::send_handler`).
    if let Err(e) = memory.upsert_session(zero_hermes::web::WEB_SESSION_ID, "web", "default") {
        tracing::warn!(error = %e, "upsert_session failed");
    }
    let histories: Arc<tokio::sync::Mutex<HashMap<String, Vec<Message>>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    // Preload the default web session so a restart resumes the transcript
    // instead of starting blank (persisted on every turn in `send_handler`).
    if let Ok(loaded) = memory.load_history(zero_hermes::web::WEB_SESSION_ID) {
        if !loaded.is_empty() {
            histories
                .lock()
                .await
                .insert(zero_hermes::web::WEB_SESSION_ID.to_string(), loaded);
        }
    }

    let state = zero_hermes::web::AppState {
        events: tx,
        histories,
        session_locks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        system,
        provider,
        tools: registry,
        memory,
        limits: RunLimits::from(&cfg.agent),
        csrf_token: zero_hermes::util::random_hex(16),
    };

    let bind_addr = bind
        .as_deref()
        .map(zero_hermes::web::parse_bind)
        .unwrap_or_default();

    println!(
        "zero-hermes web UI listening on http://{}:{}/",
        bind_addr.host, bind_addr.port
    );
    println!("(Ctrl-C to stop)");
    zero_hermes::web::serve(state, bind_addr).await
}

async fn run_gateway(cfg: Config, mock: bool) -> Result<()> {
    if cfg.telegram.token.is_empty() && !mock {
        anyhow::bail!("telegram.token is required to run the gateway (or pass --mock)");
    }
    // The agent can run arbitrary shell commands. An empty `allowed_chats`
    // means every stranger who finds the bot gets that capability, so it
    // has to be a deliberate choice rather than the default.
    if !cfg.telegram.token.is_empty()
        && cfg.telegram.allowed_chats.is_empty()
        && !cfg.telegram.allow_all_chats
    {
        anyhow::bail!(
            "refusing to start: `telegram.allowed_chats` is empty, so anyone who \
             messages this bot could run shell commands on this host. Either list \
             the chat ids you trust, or set `telegram.allow_all_chats = true` to \
             accept that risk explicitly."
        );
    }

    let skills = load_skills(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::echo())
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };
    let registry = build_tool_registry(&cfg, provider.clone());

    let (tx, mut rx) = tokio::sync::mpsc::channel::<InboundMessage>(16);
    let system = build_system_prompt_with_memory(&cfg, &skills, Some(&memory));

    // Cron scheduler
    let mut cron_handle = None;
    let (cron_tx, mut cron_rx) = tokio::sync::mpsc::channel(16);
    let mut parsed_jobs: Vec<_> = cfg
        .cron
        .jobs
        .iter()
        .map(|j| zero_hermes::cron::CronJob::new(&j.name, &j.schedule, &j.prompt))
        .collect::<Result<Vec<_>>>()?;
    // Duplicate job names make logs ambiguous and are always a mistake.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for j in &cfg.cron.jobs {
        if !seen.insert(j.name.clone()) {
            anyhow::bail!("duplicate cron job name: {:?}", j.name);
        }
    }
    // Merge agent-managed custom jobs from the `cron:custom` memory note
    // (written by the `cron` tool). Invalid entries are skipped with a
    // warning by the loader; name clashes with TOML jobs are skipped too.
    for custom in zero_hermes::tools::builtin::CronTool::load_validated_custom_jobs(memory.as_ref())
    {
        if !seen.insert(custom.name.clone()) {
            tracing::warn!(job = %custom.name, "skipping custom cron job: duplicate name");
            continue;
        }
        parsed_jobs.push(custom);
    }
    if !parsed_jobs.is_empty() {
        cron_handle =
            zero_hermes::cron::Scheduler::start_in(parsed_jobs, cfg.cron.timezone, cron_tx);
    }

    // Telegram channel
    let channel: Arc<dyn Channel> = if cfg.telegram.token.is_empty() {
        Arc::new(NoopChannel)
    } else {
        let tg_cfg = telegram::TelegramConfig {
            token: cfg.telegram.token.clone(),
            poll_timeout: cfg.telegram.poll_timeout,
            allowed_chats: cfg.telegram.allowed_chats.clone(),
            allowed_commands: cfg.telegram.allowed_commands.clone(),
        };
        Arc::new(telegram::TelegramChannel::new(tg_cfg))
    };
    let channel_name = channel.name().to_string();
    let tx_clone = tx.clone();
    let channel_arc = channel.clone();
    let mut channel_task = tokio::spawn(async move { channel_arc.run(tx_clone).await });
    let reply_channel = channel.clone();
    tracing::info!(channel = %channel_name, "gateway running (ctrl-c to stop)");

    // Per-chat conversation histories. Each inbound turn is spawned as its
    // own task so one slow LLM call never blocks other chats. Same-session
    // turns serialize on a per-session lock (different sessions stay
    // concurrent); the global histories lock is only held for the snapshot
    // and store-back. `Memory` appends already serialize on its own mutex,
    // so the DB positions cannot interleave — the session lock keeps the
    // in-memory history from losing a turn (LWW) instead.
    let histories: Arc<tokio::sync::Mutex<HashMap<String, Vec<Message>>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let session_locks: zero_hermes::web::SessionLocks =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let notify_chats: Vec<i64> = cfg.telegram.allowed_chats.clone();
    let agent_limits = RunLimits::from(&cfg.agent);

    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                let session_id = format!("{}-{}", msg.channel, msg.chat_id);
                if let Err(e) = memory.upsert_session(&session_id, &msg.channel, &msg.chat_id) {
                    tracing::warn!(error = %e, "upsert_session failed");
                }
                // Slash commands that reset state are handled locally so
                // `/new` and `/clear` actually reset instead of reaching
                // the model as plain text.
                if let Some(cmd) = msg.command.clone() {
                    let lc = cmd.to_ascii_lowercase();
                    if is_reset_command(&lc) {
                        let histories = histories.clone();
                        let session_locks = session_locks.clone();
                        let memory = memory.clone();
                        let reply = reply_channel.clone();
                        let chat_id = msg.chat_id.clone();
                        let session_id = session_id.clone();
                        tokio::spawn(async move {
                            let turn_lock =
                                zero_hermes::web::session_turn_lock(&session_locks, &session_id)
                                    .await;
                            let _turn_guard = turn_lock.lock().await;
                            {
                                let mut guard = histories.lock().await;
                                guard.remove(&session_id);
                            }
                            if let Err(e) = memory.save_history(&session_id, &[]) {
                                tracing::warn!(error = %e, "clearing session history failed");
                            }
                            if let Err(e) = reply.send(&chat_id, "Conversation cleared. Starting fresh.").await {
                                tracing::warn!(error = %e, "send reply failed");
                            }
                        });
                        continue;
                    }
                    if lc == "help" {
                        let reply = reply_channel.clone();
                        let chat_id = msg.chat_id.clone();
                        tokio::spawn(async move {
                            if let Err(e) = reply.send(&chat_id, gateway_help_text()).await {
                                tracing::warn!(error = %e, "send reply failed");
                            }
                        });
                        continue;
                    }
                }
                let user_text = match &msg.command {
                    Some(cmd) => match &msg.args {
                        Some(args) => format!("/{cmd} {args}"),
                        None => format!("/{cmd}"),
                    },
                    None => msg.text.clone(),
                };
                let provider = provider.clone();
                let registry = registry.clone();
                let system = system.clone();
                let memory_clone = memory.clone();
                let reply = reply_channel.clone();
                let histories_clone = histories.clone();
                let session_locks_clone = session_locks.clone();
                let chat_id = msg.chat_id.clone();
                tokio::spawn(async move {
                    // Serialize same-session turns; different sessions stay
                    // concurrent. The lock is held from snapshot through
                    // store-back so two quick messages cannot interleave.
                    let turn_lock =
                        zero_hermes::web::session_turn_lock(&session_locks_clone, &session_id)
                            .await;
                    let _turn_guard = turn_lock.lock().await;
                    // Snapshot history without holding the histories lock
                    // across the LLM call.
                    let snapshot = {
                        let guard = histories_clone.lock().await;
                        if let Some(h) = guard.get(&session_id) {
                            h.clone()
                        } else {
                            drop(guard);
                            let loaded =
                                memory_clone.load_history(&session_id).unwrap_or_else(|e| {
                                    tracing::warn!(error = %e, "loading durable session history failed");
                                    Vec::new()
                                });
                            let mut guard = histories_clone.lock().await;
                            guard.entry(session_id.clone()).or_insert_with(Vec::new);
                            loaded
                        }
                    };
                    let mut history = snapshot;
                    let history_len_before = history.len();
                    let mut ctx = zero_hermes::agent::make_context_with_memory(None, memory_clone.clone());
                    ctx.session_id = Some(session_id.clone());
                    let recalled = system_with_recall(
                        &system,
                        memory_clone.as_ref(),
                        &user_text,
                        Some(session_id.as_str()),
                    );
                    let out = zero_hermes::agent::run(
                        provider.as_ref(),
                        &registry,
                        Some(&recalled),
                        &mut history,
                        &user_text,
                        agent_limits,
                        &ctx,
                    )
                    .await;
                    match out {
                        Ok(text) => {
                            {
                                let mut guard = histories_clone.lock().await;
                                guard.insert(session_id.clone(), history.clone());
                            }
                            // Append only the delta: `history` was compacted in
                            // place by the agent loop, so saving the whole vector
                            // would delete the durable prefix.
                            let start = history_len_before.min(history.len());
                            if let Err(e) =
                                memory_clone.append_messages(&session_id, &history[start..])
                            {
                                tracing::warn!(error = %e, "saving durable session history failed");
                            }
                            if let Err(e) = reply.send(&chat_id, &text).await {
                                tracing::warn!(error = %e, "send reply failed");
                            }
                            drain_outbox(&memory_clone, &reply, &session_id, &chat_id).await;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "agent run failed");
                            // `run` pushes the user message before the first LLM
                            // call, so a failure leaves an orphan trailing user
                            // turn. Pop it so the next turn doesn't start with
                            // `user,user`, then persist the repaired history.
                            zero_hermes::agent::repair_history_after_failure(&mut history);
                            {
                                let mut guard = histories_clone.lock().await;
                                guard.insert(session_id.clone(), history.clone());
                            }
                            let start = history_len_before.min(history.len());
                            if let Err(e) = memory_clone.append_messages(&session_id, &history[start..]) {
                                tracing::warn!(error = %e, "saving durable session history failed");
                            }
                            // Never leave the user with silence: send a short
                            // error reply (truncated so a huge provider body
                            // can't blow past Telegram's message cap).
                            let reply_text = zero_hermes::util::truncate_bytes(
                                &format!("Sorry, I hit an error: {e}"),
                                400,
                            );
                            if let Err(e) = reply.send(&chat_id, &reply_text).await {
                                tracing::warn!(error = %e, "send error reply failed");
                            }
                        }
                    }
                });
            }
            Some(evt) = cron_rx.recv() => {
                // Cron ticks get a fresh history — each prompt stands alone,
                // so compaction has nothing to do and is switched off.
                // Late ticks (scheduler was down) are labelled so the model
                // knows it is catching up rather than firing on time.
                let lateness = chrono::Utc::now()
                    .signed_duration_since(evt.at)
                    .num_seconds();
                let prompt = if lateness > 120 {
                    format!(
                        "[missed scheduled run for {:?} at {} ({}s late); catching up now]\n{}",
                        evt.job,
                        evt.at.to_rfc3339(),
                        lateness,
                        evt.prompt
                    )
                } else {
                    evt.prompt.clone()
                };
                let session_id = format!("cron-{}", evt.job);
                if let Err(e) = memory.upsert_session(&session_id, "cron", &evt.job) {
                    tracing::warn!(error = %e, "upsert cron session failed");
                }
                let provider = provider.clone();
                let registry = registry.clone();
                let system = system.clone();
                let memory_clone = memory.clone();
                let reply = reply_channel.clone();
                let notify = notify_chats.clone();
                let job_name = evt.job.clone();
                tokio::spawn(async move {
                    let mut ctx = zero_hermes::agent::make_context_with_memory(None, memory_clone.clone());
                    ctx.session_id = Some(session_id.clone());
                    let mut history: Vec<Message> = Vec::new();
                    match zero_hermes::agent::run(
                        provider.as_ref(),
                        &registry,
                        Some(&system),
                        &mut history,
                        &prompt,
                        agent_limits.with_context_window(0),
                        &ctx,
                    )
                    .await
                    {
                        Ok(text) => {
                            tracing::info!(
                                job = %job_name,
                                result = %zero_hermes::util::truncate_bytes(&text, 500),
                                "cron job completed"
                            );
                            if let Err(e) = memory_clone.save_history(&session_id, &history) {
                                tracing::warn!(error = %e, "saving cron history failed");
                            }
                            // Best-effort delivery: with exactly one allowed
                            // chat there is an unambiguous recipient.
                            // Otherwise the result stays in memory under
                            // `cron-<job>` plus the info log above.
                            if notify.len() == 1 {
                                let chat_id = notify[0].to_string();
                                if let Err(e) = reply.send(&chat_id, &text).await {
                                    tracing::warn!(error = %e, "cron notify send failed");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(job = %job_name, error = %e, "cron job failed");
                        }
                    }
                });
            }
            // Surface channel failures instead of leaving the gateway
            // running against a channel that has silently given up (a
            // rejected bot token, for instance).
            joined = &mut channel_task => {
                match joined {
                    Ok(Ok(())) => tracing::info!(channel = %channel_name, "channel loop ended"),
                    Ok(Err(e)) => {
                        tracing::error!(channel = %channel_name, error = %e, "channel loop failed");
                        shutdown(cron_handle).await;
                        return Err(e);
                    }
                    Err(e) => tracing::error!(channel = %channel_name, error = %e, "channel task panicked"),
                }
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown signal received");
                break;
            }
            else => break,
        }
    }

    channel_task.abort();
    shutdown(cron_handle).await;
    Ok(())
}

/// Stop the cron scheduler, if one was started.
async fn shutdown(cron_handle: Option<zero_hermes::cron::SchedulerHandle>) {
    if let Some(h) = cron_handle {
        h.stop().await;
    }
}

fn cron_list(cfg: &Config) -> Result<()> {
    println!("Configured cron jobs (timezone: {:?}):", cfg.cron.timezone);
    for j in &cfg.cron.jobs {
        let job = zero_hermes::cron::CronJob::new(&j.name, &j.schedule, &j.prompt)?;
        let next = job
            .next_after_in(chrono::Utc::now(), cfg.cron.timezone)
            .map(|n| n.to_rfc3339())
            .unwrap_or_else(|| "-".into());
        println!("  {:<20} {:<20} -> {}", j.name, j.schedule, next);
    }
    Ok(())
}

fn cron_check(expr: &str) -> Result<()> {
    let next = zero_hermes::cron::describe(expr, chrono::Utc::now())?;
    println!("{expr} -> next: {next}");
    Ok(())
}

fn tools_list(cfg: &Config) -> Result<()> {
    // Reflects the *loaded* config, so `--config` and `[agent].enabled_tools`
    // are visible here rather than being silently ignored.
    // No provider call happens; the placeholder only satisfies
    // `SubAgentTool`'s signature.
    let placeholder: Arc<dyn LlmProvider> = Arc::new(MockProvider::text_only(""));
    let reg = build_tool_registry(cfg, placeholder);
    println!("Tool registry:");
    for name in reg.names() {
        println!("  - {name}");
    }
    Ok(())
}

fn init_config(force: bool) -> Result<()> {
    let dir = zero_hermes::config::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("zero_hermes.toml");
    if path.exists() && !force {
        anyhow::bail!(
            "{} already exists (use --force to overwrite)",
            path.display()
        );
    }
    let sample = r#"# zero-hermes configuration
#
# Two provider `kind`s are supported:
#   - "anthropic"      -> /v1/messages, x-api-key header
#                        (default; used by minimax at https://api.minimax.io/anthropic)
#   - "openai_compat"  -> /v1/chat/completions, Bearer auth
#                        (OpenAI, Together, Groq, OpenRouter, llama.cpp, ollama, ...)
#
# Swap by changing `kind` and `base_url`. All other fields stay the same.
# A leading `~` in any path below is expanded to your home directory.

[provider]
kind       = "anthropic"               # or "openai_compat"
base_url   = "https://api.minimax.io/anthropic"
api_key    = ""
model      = "MiniMax-M3"
max_tokens = 8192
# Seconds to wait for the connection, and between reads once the response
# has started. Without these a wedged provider stalls the whole gateway.
connect_timeout_secs = 10
read_timeout_secs    = 120
# OpenAI-compat-only (ignored when kind = "anthropic"):
# temperature = 0.7
# Ordered fallbacks: if the primary `complete` fails, each fallback is
# tried once in order. `api_key` may hold a `;`-separated pool that is
# rotated round-robin per call.
# [[provider.fallbacks]]
# kind = "openai_compat"
# base_url = "https://api.openai.com"
# api_key = "${OPENAI_API_KEY}"
# model = "gpt-4o-mini"

[telegram]
token = ""
poll_timeout = 30
# The agent can run shell commands, so leaving this empty would hand that
# capability to anyone who messages the bot. List the chat ids you trust;
# the gateway refuses to start with an empty list unless you also set
# `allow_all_chats = true`.
allowed_chats = []
# allow_all_chats = false
# allowed_commands = []

[memory]
path = "~/.local/share/zero-hermes/memory.sqlite"
# Durable Markdown memory, split Hermes-style and injected into every system
# prompt. USER.md holds stable facts about the human; MEMORY.md holds
# agent-learned conventions. Keep both files private.
user_path = "memory/USER.md"
markdown_path = "memory/MEMORY.md"

[agent]
max_iterations = 50
# Messages retained before the oldest are dropped. Compaction only ever
# cuts at a boundary that keeps tool calls paired with their results.
# Set to 0 to disable.
context_window = 50
enabled_tools  = []

[cron]
# "utc" (default) or "local" — with "local", `0 9 * * *` means 9am here.
timezone = "utc"

[[cron.jobs]]
name = "status"
schedule = "*/15 * * * *"
prompt = "summarise recent activity"
"#;
    std::fs::write(&path, sample)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// No-op channel used when no Telegram token is configured.
struct NoopChannel;

#[async_trait::async_trait]
impl Channel for NoopChannel {
    fn name(&self) -> &str {
        "noop"
    }
    async fn run(&self, _tx: tokio::sync::mpsc::Sender<InboundMessage>) -> Result<()> {
        tokio::time::sleep(std::time::Duration::from_secs(u64::MAX / 4)).await;
        Ok(())
    }
    async fn send(&self, _chat_id: &str, _text: &str) -> Result<()> {
        Ok(())
    }
}
