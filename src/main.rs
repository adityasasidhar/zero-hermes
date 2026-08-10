//! `zero-hermes` — minimal Rust reimplementation of Hermes Agent core.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};

use zero_hermes::agent::mock::MockProvider;
use zero_hermes::agent::{build_provider, LlmProvider, Message};
use zero_hermes::channels::{telegram, Channel, InboundMessage};
use zero_hermes::config::Config;
use zero_hermes::error::Result;
use zero_hermes::memory::Memory;
use zero_hermes::skills::SkillRegistry;
use zero_hermes::tools::ToolRegistry;

#[derive(Parser, Debug)]
#[command(name = "zero-hermes", version, about = "Minimal Hermes Agent in Rust")]
struct Cli {
    /// Path to the config file (default: $XDG_CONFIG_HOME/zero-hermes/zero_hermes.toml).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Use a mock provider (no network calls). Useful for `run` smoke testing.
    #[arg(long, global = true)]
    mock: bool,

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

    let cfg_path = cli
        .config
        .clone()
        .or(zero_hermes::config::config_file())
        .unwrap_or_else(|| PathBuf::from("zero_hermes.toml"));
    let cfg = zero_hermes::config::load(&cfg_path).context("loading config")?;

    match cli.command {
        Command::Gateway => run_gateway(cfg, cli.mock).await,
        Command::Run { message } => run_once(cfg, &message, cli.mock).await,
        Command::Cron { action } => match action {
            CronAction::List => cron_list(&cfg),
            CronAction::Check { expr } => cron_check(&expr),
        },
        Command::Tools => tools_list(),
        Command::InitConfig { force } => init_config(force),
        Command::ShowConfig => {
            let s = toml::to_string_pretty(&cfg).unwrap_or_else(|_| "<unparseable>".into());
            println!("{s}");
            Ok(())
        }
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

fn build_tool_registry(cfg: &Config) -> ToolRegistry {
    use zero_hermes::tools::builtin::{BashTool, FetchTool, MemoryTool, ReadTool, WriteTool};

    let mut reg = ToolRegistry::new();
    reg.insert(Arc::new(BashTool), &cfg.agent.enabled_tools);
    reg.insert(Arc::new(ReadTool), &cfg.agent.enabled_tools);
    reg.insert(Arc::new(WriteTool), &cfg.agent.enabled_tools);
    reg.insert(Arc::new(FetchTool::default()), &cfg.agent.enabled_tools);
    reg.insert(Arc::new(MemoryTool), &cfg.agent.enabled_tools);
    reg
}

fn build_system_prompt(cfg: &Config, skills: &SkillRegistry) -> String {
    let base = cfg
        .provider
        .system
        .clone()
        .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());
    base.replace("{{SKILLS}}", &skills.render_index())
        .replace("{{MEMORY}}", "(memory: in-process SQLite)")
}

const DEFAULT_SYSTEM_PROMPT: &str = "You are zero-hermes, a minimal Hermes Agent.\n\n\
Available skills (read SKILL.md on demand with the bash tool):\n{{SKILLS}}\n\n\
Memory: {{MEMORY}}\n\n\
When you need to use a tool, emit a tool_use block. When you have a final answer, \
respond with plain text only.";

async fn run_once(cfg: Config, message: &str, mock: bool) -> Result<()> {
    let skills = load_skills(&cfg);
    let registry = build_tool_registry(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::text_only("hello from mock"))
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };
    let system = build_system_prompt(&cfg, &skills);
    let ctx = zero_hermes::agent::make_context_with_memory(None, memory);

    let mut history: Vec<Message> = Vec::new();
    let out = zero_hermes::agent::run(
        provider.as_ref(),
        &registry,
        Some(&system),
        &mut history,
        message,
        cfg.agent.max_iterations,
        &ctx,
    )
    .await?;
    println!("{out}");
    Ok(())
}

async fn run_gateway(cfg: Config, mock: bool) -> Result<()> {
    if cfg.telegram.token.is_empty() && !mock {
        anyhow::bail!("telegram.token is required to run the gateway (or pass --mock)");
    }

    let skills = load_skills(&cfg);
    let registry = build_tool_registry(&cfg);
    let memory = Arc::new(Memory::open(cfg.memory.path.as_deref())?);
    let provider: Arc<dyn LlmProvider> = if mock {
        Arc::new(MockProvider::echo())
    } else {
        Arc::from(build_provider(&cfg.provider)?)
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<InboundMessage>(16);
    let system = build_system_prompt(&cfg, &skills);

    // Cron scheduler
    let mut cron_handle = None;
    let (cron_tx, mut cron_rx) = tokio::sync::mpsc::channel(16);
    let parsed_jobs: Vec<_> = cfg
        .cron
        .jobs
        .iter()
        .map(|j| zero_hermes::cron::CronJob::new(&j.name, &j.schedule, &j.prompt))
        .collect::<Result<Vec<_>>>()?;
    if !parsed_jobs.is_empty() {
        cron_handle = zero_hermes::cron::Scheduler::start(parsed_jobs, cron_tx);
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
    let channel_task = tokio::spawn(async move { channel_arc.run(tx_clone).await });
    let reply_channel = channel.clone();
    tracing::info!(channel = %channel_name, "gateway running");

    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                let ctx = zero_hermes::agent::make_context_with_memory(None, memory.clone());
                let mut history: Vec<Message> = Vec::new();
                memory
                    .upsert_session(&format!("{}-{}", msg.channel, msg.chat_id), &msg.channel, &msg.chat_id)
                    .ok();
                let user_text = match &msg.command {
                    Some(cmd) => match &msg.args {
                        Some(args) => format!("/{cmd} {args}"),
                        None => format!("/{cmd}"),
                    },
                    None => msg.text.clone(),
                };
                let out = zero_hermes::agent::run(
                    provider.as_ref(),
                    &registry,
                    Some(&system),
                    &mut history,
                    &user_text,
                    cfg.agent.max_iterations,
                    &ctx,
                )
                .await;
                match out {
                    Ok(text) => {
                        if let Err(e) = reply_channel.send(&msg.chat_id, &text).await {
                            tracing::warn!(error = %e, "send reply failed");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "agent run failed");
                    }
                }
            }
            Some(evt) = cron_rx.recv() => {
                let ctx = zero_hermes::agent::make_context_with_memory(None, memory.clone());
                let mut history: Vec<Message> = Vec::new();
                let prompt = format!("[cron:{}] tick", evt.job);
                let _ = zero_hermes::agent::run(
                    provider.as_ref(),
                    &registry,
                    Some(&system),
                    &mut history,
                    &prompt,
                    3,
                    &ctx,
                ).await;
            }
            else => break,
        }
    }

    drop(channel_task);
    if let Some(h) = cron_handle {
        h.stop().await;
    }
    Ok(())
}

fn load_skills(cfg: &Config) -> SkillRegistry {
    let dir = cfg
        .skills_dir
        .clone()
        .or_else(|| zero_hermes::config::config_dir().map(|d| d.join("skills")))
        .unwrap_or_else(|| PathBuf::from("skills"));
    SkillRegistry::load_dir(&dir).unwrap_or_default()
}

fn cron_list(cfg: &Config) -> Result<()> {
    println!("Configured cron jobs:");
    for j in &cfg.cron.jobs {
        let job = zero_hermes::cron::CronJob::new(&j.name, &j.schedule, &j.prompt)?;
        let next = job
            .next()
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

fn tools_list() -> Result<()> {
    let cfg = Config::default();
    let reg = build_tool_registry(&cfg);
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

[provider]
kind       = "anthropic"               # or "openai_compat"
base_url   = "https://api.minimax.io/anthropic"
api_key    = ""
model      = "MiniMax-M3"
max_tokens = 8192
# OpenAI-compat-only (ignored when kind = "anthropic"):
# temperature = 0.7
# stream      = false

[telegram]
token = ""
poll_timeout = 30

[memory]
path = "~/.local/share/zero-hermes/memory.sqlite"

[agent]
max_iterations = 10
context_window = 50
enabled_tools = []

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
