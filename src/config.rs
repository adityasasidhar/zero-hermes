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
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// API key (sent as `Authorization: Bearer` for openai-compat, `x-api-key` for anthropic).
    #[serde(default)]
    pub api_key: String,
    /// Model id, e.g. `MiniMax-M3` or `gpt-4o-mini`.
    #[serde(default = "default_model")]
    pub model: String,
    /// Maximum tokens to generate.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Optional override for system prompt.
    #[serde(default)]
    pub system: Option<String>,
    /// OpenAI-compat-only: optional `temperature` (0.0–2.0).
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Seconds to wait for the TCP+TLS connection to the provider.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// Seconds to wait between reads once the response has started.
    ///
    /// This is a *per-read* timeout, not a whole-request deadline, so a
    /// long streaming completion is fine but a provider that stops talking
    /// mid-response is not. Without it a wedged provider hangs the gateway
    /// forever, taking every chat and cron tick with it.
    #[serde(default = "default_read_timeout")]
    pub read_timeout_secs: u64,
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_read_timeout() -> u64 {
    120
}

fn default_provider_kind() -> ProviderKind {
    ProviderKind::Anthropic
}

fn default_base_url() -> String {
    "https://api.minimax.io/anthropic".to_string()
}

fn default_model() -> String {
    "MiniMax-M3".to_string()
}

fn default_max_tokens() -> u32 {
    8192
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
            kind: default_provider_kind(),
            base_url: default_base_url(),
            api_key: String::new(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            system: None,
            temperature: None,
            connect_timeout_secs: default_connect_timeout(),
            read_timeout_secs: default_read_timeout(),
        }
    }
}

/// Telegram gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot token from @BotFather.
    #[serde(default)]
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
    /// Opt in to serving *every* chat that messages the bot.
    ///
    /// The agent can run arbitrary shell commands, so an empty
    /// `allowed_chats` means anyone who finds the bot gets code execution
    /// on this host. The gateway refuses to start in that configuration
    /// unless this is explicitly set to `true`.
    #[serde(default)]
    pub allow_all_chats: bool,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            allowed_chats: Vec::new(),
            poll_timeout: default_poll_timeout(),
            allowed_commands: Vec::new(),
            allow_all_chats: false,
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
    #[serde(default = "default_memory_path_opt")]
    pub path: Option<PathBuf>,
    /// Optional Markdown memory injected into the system prompt on every turn.
    #[serde(default = "default_memory_markdown_path")]
    pub markdown_path: Option<PathBuf>,
}

fn default_memory_path_opt() -> Option<PathBuf> {
    Some(default_memory_path())
}

fn default_memory_markdown_path() -> Option<PathBuf> {
    Some(PathBuf::from("memory/MEMORY.md"))
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            path: Some(default_memory_path()),
            markdown_path: default_memory_markdown_path(),
        }
    }
}

/// Agent loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum number of LLM/tool iterations per user turn.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// Soft cap on conversation messages before compacting.
    #[serde(default = "default_context_window")]
    pub context_window: usize,
    /// Approximate token budget for retained conversation context. `0`
    /// disables token-aware compaction.
    #[serde(default = "default_context_tokens")]
    pub context_tokens: usize,
    /// Allow-list of tool names; empty = all registered tools.
    #[serde(default)]
    pub enabled_tools: Vec<String>,
}

fn default_max_iterations() -> usize {
    50
}

fn default_context_window() -> usize {
    50
}

fn default_context_tokens() -> usize {
    24_000
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            context_window: default_context_window(),
            context_tokens: default_context_tokens(),
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
    /// Which clock cron expressions are interpreted against.
    #[serde(default)]
    pub timezone: CronTimezone,
}

/// Clock used to evaluate cron expressions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronTimezone {
    /// Interpret schedules as UTC (the default, and what v0.1 always did).
    #[default]
    Utc,
    /// Interpret schedules against the host's local timezone, so
    /// `0 9 * * *` means 9am where the machine is.
    Local,
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
    let mut cfg: Config = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
    cfg.expand_paths();
    cfg.expand_env();
    Ok(cfg)
}

impl Config {
    /// Expand a leading `~` in every user-supplied path.
    ///
    /// TOML has no shell, so `path = "~/x"` arrives as a literal tilde and
    /// would otherwise create a `./~/` directory next to the process CWD.
    pub fn expand_paths(&mut self) {
        if let Some(p) = &self.memory.path {
            self.memory.path = Some(crate::util::expand_tilde(p));
        }
        if let Some(p) = &self.memory.markdown_path {
            self.memory.markdown_path = Some(crate::util::expand_tilde(p));
        }
        if let Some(p) = &self.skills_dir {
            self.skills_dir = Some(crate::util::expand_tilde(p));
        }
    }

    /// Expand `${VAR}` / `$VAR` references in the fields that commonly hold
    /// secrets or per-machine values.
    ///
    /// Keeps credentials out of `zero_hermes.toml`, which is the file
    /// people copy around and paste into issues:
    ///
    /// ```toml
    /// [provider]
    /// api_key = "${MINIMAX_API_KEY}"
    /// ```
    pub fn expand_env(&mut self) {
        use crate::util::expand_env_vars;
        self.provider.api_key = expand_env_vars(&self.provider.api_key);
        self.provider.base_url = expand_env_vars(&self.provider.base_url);
        self.provider.model = expand_env_vars(&self.provider.model);
        self.telegram.token = expand_env_vars(&self.telegram.token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let cfg = Config::default();
        assert!(cfg.provider.base_url.starts_with("https://"));
        assert_eq!(cfg.agent.max_iterations, 50);
        assert!(cfg.memory.path.is_some());
    }

    #[test]
    fn load_missing_returns_default() {
        let cfg = load(Path::new("/tmp/__definitely_missing__.toml")).unwrap();
        assert_eq!(cfg.agent.max_iterations, 50);
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

    #[test]
    fn tilde_in_memory_path_is_expanded_on_load() {
        let home = dirs::home_dir().expect("home dir");
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("zero_hermes.toml");
        std::fs::write(
            &p,
            "skills_dir = \"~/skills\"\n\n[memory]\npath = \"~/.local/share/zero-hermes/memory.sqlite\"\n",
        )
        .unwrap();
        let cfg = load(&p).unwrap();
        assert_eq!(
            cfg.memory.path.unwrap(),
            home.join(".local/share/zero-hermes/memory.sqlite"),
            "a literal ~ would create ./~/ under the CWD"
        );
        assert_eq!(cfg.skills_dir.unwrap(), home.join("skills"));
    }

    #[test]
    fn provider_timeouts_have_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.provider.connect_timeout_secs, 10);
        assert_eq!(cfg.provider.read_timeout_secs, 120);
        // ...and are overridable.
        let cfg: Config =
            toml::from_str("[provider]\nbase_url=\"u\"\napi_key=\"k\"\nmodel=\"m\"\nmax_tokens=1\nread_timeout_secs=5\n")
                .unwrap();
        assert_eq!(cfg.provider.read_timeout_secs, 5);
        assert_eq!(cfg.provider.connect_timeout_secs, 10);
    }

    #[test]
    fn open_bot_requires_explicit_opt_in() {
        assert!(
            !Config::default().telegram.allow_all_chats,
            "defaulting to an open bot would hand bash to anyone"
        );
    }

    #[test]
    fn cron_timezone_defaults_to_utc() {
        assert_eq!(Config::default().cron.timezone, CronTimezone::Utc);
        let cfg: Config = toml::from_str("[cron]\ntimezone = \"local\"\n").unwrap();
        assert_eq!(cfg.cron.timezone, CronTimezone::Local);
    }

    #[test]
    fn partial_sections_fall_back_per_field() {
        // Every section is `#[serde(default)]`, but the fields inside them
        // were mandatory — so writing just one key in `[agent]` failed with
        // "missing field max_iterations" instead of using the defaults.
        let cfg: Config = toml::from_str(
            "[agent]\nenabled_tools = [\"read\"]\n\n[provider]\napi_key = \"sk-x\"\n\n[telegram]\npoll_timeout = 5\n",
        )
        .expect("a partial config should parse");
        assert_eq!(cfg.agent.enabled_tools, vec!["read".to_string()]);
        assert_eq!(cfg.agent.max_iterations, 50);
        assert_eq!(cfg.agent.context_window, 50);
        assert_eq!(cfg.provider.api_key, "sk-x");
        assert_eq!(cfg.provider.model, "MiniMax-M3");
        assert_eq!(cfg.provider.max_tokens, 8192);
        assert_eq!(cfg.telegram.poll_timeout, 5);
        assert!(cfg.telegram.token.is_empty());
        assert!(cfg.memory.path.is_some(), "memory path keeps its default");
    }

    #[test]
    fn empty_sections_match_defaults() {
        let cfg: Config = toml::from_str("[agent]\n[provider]\n[telegram]\n[memory]\n[cron]\n")
            .expect("empty sections should parse");
        let d = Config::default();
        assert_eq!(cfg.agent.max_iterations, d.agent.max_iterations);
        assert_eq!(cfg.provider.base_url, d.provider.base_url);
        assert_eq!(cfg.memory.path, d.memory.path);
    }

    #[test]
    fn env_references_are_expanded_in_secrets_and_urls() {
        std::env::set_var("ZH_CFG_KEY", "sk-from-env");
        std::env::set_var("ZH_CFG_URL", "https://example.test");
        std::env::set_var("ZH_CFG_TOKEN", "123:abc");
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("zero_hermes.toml");
        std::fs::write(
            &p,
            "[provider]\napi_key = \"${ZH_CFG_KEY}\"\nbase_url = \"${ZH_CFG_URL}\"\n\n[telegram]\ntoken = \"${ZH_CFG_TOKEN}\"\n",
        )
        .unwrap();
        let cfg = load(&p).unwrap();
        assert_eq!(cfg.provider.api_key, "sk-from-env");
        assert_eq!(cfg.provider.base_url, "https://example.test");
        assert_eq!(cfg.telegram.token, "123:abc");
    }

    #[test]
    fn literal_keys_still_work() {
        let mut cfg = Config::default();
        cfg.provider.api_key = "sk-literal".into();
        cfg.expand_env();
        assert_eq!(cfg.provider.api_key, "sk-literal");
    }
}
