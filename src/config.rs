//! Configuration loading for `zero-hermes`.
//!
//! Config is an optional TOML file at `~/.config/zero-hermes/zero_hermes.toml`.
//! When the file is absent we fall back to defaults so the binary still runs
//! (matches the "minimal binary should always start" intent of the brief).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Top-level configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// LLM provider settings.
    #[serde(default)]
    pub provider: ProviderConfig,
    /// Telegram gateway settings.
    #[serde(default)]
    pub telegram: TelegramConfig,
    /// Memory (sqlite) settings.
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Agent loop settings.
    #[serde(default)]
    pub agent: AgentConfig,
    /// Cron scheduler settings.
    #[serde(default)]
    pub cron: CronConfig,
    /// Skills directory (defaults to `<config_dir>/skills`).
    #[serde(default)]
    pub skills_dir: Option<PathBuf>,
}

/// LLM provider configuration. The `kind` field selects the wire format:
/// `anthropic` (the default — minimax-compatible `Messages` API) or
/// `openai_compat` (any OpenAI-compatible `/v1/chat/completions` endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Which provider implementation to use.
    #[serde(default = "default_provider_kind")]
    pub kind: ProviderKind,
    /// Base URL for the chosen endpoint.
    pub base_url: String,
    /// API key (sent as `Authorization: Bearer` for openai-compat, `x-api-key` for anthropic).
    pub api_key: String,
    /// Model id, e.g. `MiniMax-M3` or `gpt-4o-mini`.
    pub model: String,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Optional override for system prompt.
    pub system: Option<String>,
    /// OpenAI-compat-only: optional `temperature` (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// OpenAI-compat-only: request `stream: true` (not yet used by the loop; reserved).
    #[serde(default)]
    pub stream: bool,
}

fn default_provider_kind() -> ProviderKind {
    ProviderKind::Anthropic
}

/// Which provider implementation the agent loop should build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic Messages API (default; minimax uses this).
    Anthropic,
    /// Any OpenAI-compatible `/v1/chat/completions` endpoint.
    OpenaiCompat,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::Anthropic,
            base_url: "https://api.minimax.io/anthropic".to_string(),
            api_key: String::new(),
            model: "MiniMax-M3".to_string(),
            max_tokens: 8192,
            system: None,
            temperature: None,
            stream: false,
        }
    }
}

/// Telegram gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot token from @BotFather.
    pub token: String,
    /// Restrict inbound messages to these chat ids (empty = any).
    #[serde(default)]
    pub allowed_chats: Vec<i64>,
    /// Long-poll timeout in seconds.
    #[serde(default = "default_poll_timeout")]
    pub poll_timeout: u32,
    /// Allowed bot commands (slash-prefixed). Empty = all.
    #[serde(default)]
    pub allowed_commands: Vec<String>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            allowed_chats: Vec::new(),
            poll_timeout: default_poll_timeout(),
            allowed_commands: Vec::new(),
        }
    }
}

fn default_poll_timeout() -> u32 {
    30
}

/// Memory store configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Path to the SQLite database file. `None` -> in-memory.
    pub path: Option<PathBuf>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            path: Some(default_memory_path()),
        }
    }
}

/// Agent loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum number of LLM/tool iterations per user turn.
    pub max_iterations: usize,
    /// Soft cap on conversation messages before compacting.
    pub context_window: usize,
    /// Allow-list of tool names; empty = all registered tools.
    pub enabled_tools: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            context_window: 50,
            enabled_tools: Vec::new(),
        }
    }
}

/// Cron scheduler configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronConfig {
    /// Pre-defined jobs to register at startup.
    #[serde(default)]
    pub jobs: Vec<CronJobConfig>,
}

/// A configured cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobConfig {
    /// Unique name.
    pub name: String,
    /// 5-field cron expression (`* * * * *`).
    pub schedule: String,
    /// Free-form prompt sent to the agent.
    pub prompt: String,
}

/// Default memory location: `$XDG_DATA_HOME/zero-hermes/memory.sqlite` or
/// `~/.local/share/zero-hermes/memory.sqlite`.
pub fn default_memory_path() -> PathBuf {
    if let Some(base) = dirs::data_dir() {
        return base.join("zero-hermes").join("memory.sqlite");
    }
    PathBuf::from("memory.sqlite")
}

/// Locate the user's config directory (`~/.config/zero-hermes`).
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("zero-hermes"))
}

/// Path to the canonical config file.
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("zero_hermes.toml"))
}

/// Load config from `path`, falling back to defaults if the file is missing.
pub fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        tracing::info!(?path, "config file not found, using defaults");
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let cfg = Config::default();
        assert!(cfg.provider.base_url.starts_with("https://"));
        assert_eq!(cfg.agent.max_iterations, 10);
        assert!(cfg.memory.path.is_some());
    }

    #[test]
    fn load_missing_returns_default() {
        let cfg = load(Path::new("/tmp/__definitely_missing__.toml")).unwrap();
        assert_eq!(cfg.agent.max_iterations, 10);
    }

    #[test]
    fn parse_minimal_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("zero_hermes.toml");
        std::fs::write(
            &p,
            r#"
[provider]
base_url = "https://example.test/v1"
api_key = "sk-test"
model = "test-model"
max_tokens = 4096

[agent]
max_iterations = 4
context_window = 16
enabled_tools = ["bash", "read"]

[[cron.jobs]]
name = "ping"
schedule = "*/5 * * * *"
prompt = "say pong"
"#,
        )
        .unwrap();
        let cfg = load(&p).unwrap();
        assert_eq!(cfg.provider.max_tokens, 4096);
        assert_eq!(cfg.agent.max_iterations, 4);
        assert_eq!(cfg.cron.jobs.len(), 1);
        assert_eq!(cfg.cron.jobs[0].name, "ping");
    }
}
