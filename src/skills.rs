//! SKILL.md loader and registry.
//!
//! A SKILL.md file looks like:
//!
//! ```markdown
//! ---
//! name: git
//! description: Run git commands...
//! ---
//!
//! # Body
//! ...
//! ```
//!
//! The loader splits the file into frontmatter (TOML/INI-ish) and the body.
//! For v1 we only support a `key: value` frontmatter format (one per line),
//! which is what all real Hermes skills ship.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// A single loaded skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Skill name (from `name:` frontmatter, or the directory name).
    pub name: String,
    /// One-line description (from `description:` frontmatter).
    pub description: String,
    /// Raw markdown body (below the frontmatter).
    pub body: String,
    /// Source file path.
    pub path: PathBuf,
}

impl Skill {
    /// Render a short index entry for the system prompt.
    pub fn index_line(&self) -> String {
        format!(
            "- {}: {} ({})",
            self.name,
            self.description,
            self.path.display()
        )
    }
}

/// In-memory skill registry.
#[derive(Debug, Default, Clone)]
pub struct SkillRegistry {
    skills: BTreeMap<String, Skill>,
}

impl SkillRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a skill (overwrites if `name` already exists).
    pub fn insert(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Names of all registered skills.
    pub fn names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    /// Iterate skills in alphabetical order.
    pub fn iter(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values()
    }

    /// Render a markdown index for the system prompt.
    pub fn render_index(&self) -> String {
        if self.skills.is_empty() {
            return "(no skills loaded)".to_string();
        }
        let mut out = String::new();
        for s in self.skills.values() {
            out.push_str(&s.index_line());
            out.push('\n');
        }
        out
    }

    /// Render a budgeted index for the system prompt (E24).
    ///
    /// The full index is ~7k tokens for the vendored pack; every prompt
    /// would pay that. This lists entries alphabetically until `max_chars`
    /// would be exceeded, then appends a `+N more` hint pointing at the
    /// `skill` tool's `list` action for discovery. At least one entry is
    /// always shown so a tiny budget still names something. `render_index`
    /// remains the complete listing used by the `skill(list)` tool itself.
    pub fn render_index_capped(&self, max_chars: usize) -> String {
        if self.skills.is_empty() {
            return "(no skills loaded)".to_string();
        }
        let total = self.skills.len();
        let mut out = String::new();
        let mut shown = 0usize;
        for s in self.skills.values() {
            let line = format!("{}\n", s.index_line());
            if shown > 0 && out.len() + line.len() > max_chars {
                break;
            }
            out.push_str(&line);
            shown += 1;
            if out.len() >= max_chars {
                break;
            }
        }
        if shown < total {
            out.push_str(&format!(
                "... +{} more; use skill(list) for the full index\n",
                total - shown
            ));
        }
        out
    }

    /// Build the registry by recursively scanning `dir` for `SKILL.md` and
    /// `skill.md` files. This accepts the nested category layout shipped by
    /// Hermes Agent as well as zero-hermes' original one-directory-per-skill
    /// layout.
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::info!(?dir, "skills dir does not exist");
            return Ok(reg);
        }
        Self::load_dir_recursive(dir, &mut reg)?;
        Ok(reg)
    }

    fn load_dir_recursive(dir: &Path, reg: &mut Self) -> Result<()> {
        let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                Self::load_dir_recursive(&path, reg)?;
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("skill.md"))
            {
                let mut skill = Skill::from_file(&path)?;
                if let Some(previous) = reg.get(&skill.name) {
                    let duplicate_name = path
                        .parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("skill")
                        .to_string();
                    tracing::warn!(
                        name = %skill.name,
                        previous = ?previous.path,
                        replacement = ?skill.path,
                        alias = %duplicate_name,
                        "duplicate skill name; loading the later path under its directory alias"
                    );
                    skill.name = Self::unique_name(duplicate_name, reg);
                }
                reg.insert(skill);
            }
        }
        Ok(())
    }

    /// Produce a collision-free skill name, preserving the source directory
    /// name as the readable base for a skill with duplicate frontmatter.
    fn unique_name(base: String, reg: &Self) -> String {
        if reg.get(&base).is_none() {
            return base;
        }
        let mut suffix = 2usize;
        loop {
            let candidate = format!("{base}-{suffix}");
            if reg.get(&candidate).is_none() {
                return candidate;
            }
            suffix += 1;
        }
    }
}

impl Skill {
    /// Parse a SKILL.md from a string. The first YAML-ish frontmatter block
    /// is extracted; the remainder is returned as the body.
    pub fn parse(name: String, path: PathBuf, raw: &str) -> Result<Self> {
        let (front, body) = split_frontmatter(raw);
        let mut description = String::new();
        for line in front.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("description:") {
                description = rest.trim().trim_matches(['"', '\'']).to_string();
            }
        }
        if description.is_empty() {
            description = format!("(skill: {name})");
        }
        Ok(Self {
            name,
            description,
            body,
            path,
        })
    }

    /// Load a skill from a file. The `name` is read from frontmatter when
    /// present, otherwise inferred from the parent directory.
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let fallback_name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("skill")
            .to_string();
        let (front, _) = split_frontmatter(&raw);
        let name = front
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix("name:"))
            .map(|name| name.trim().trim_matches(['"', '\'']).to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or(fallback_name);
        Self::parse(name, path.to_path_buf(), &raw)
    }
}

/// Split a SKILL.md into (frontmatter, body). Frontmatter is delimited by
/// `---\n` lines at the very start and end of the file.
pub fn split_frontmatter(raw: &str) -> (String, String) {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return (String::new(), raw.to_string());
    }
    // Skip the opening `---` line.
    let after_open = trimmed.trim_start_matches("---");
    let after_open = after_open.trim_start_matches('\n');
    if let Some(close_idx) = find_frontmatter_close(after_open) {
        let front = after_open[..close_idx].to_string();
        // Skip past the closing delimiter line and any following newlines.
        let mut body = &after_open[close_idx..];
        // Skip the closing `---` itself.
        body = body
            .strip_prefix("---")
            .or_else(|| body.strip_prefix("..."))
            .unwrap_or(body);
        body = body.trim_start_matches('\n');
        (front, body.to_string())
    } else {
        (String::new(), raw.to_string())
    }
}

fn find_frontmatter_close(s: &str) -> Option<usize> {
    let mut idx = 0usize;
    for line in s.split_inclusive('\n') {
        if line.trim() == "---" || line.trim() == "...\n" {
            return Some(idx);
        }
        idx += line.len();
    }
    None
}

// --- Skills hub (E17) ----------------------------------------------------

/// One entry in a hub manifest: where to fetch a missing `SKILL.md`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HubSkillEntry {
    /// Skill name (letters, digits, `_`, `-` only).
    pub name: String,
    /// One-line description shown before sync.
    #[serde(default)]
    pub description: String,
    /// URL of the raw `SKILL.md` to download.
    pub url: String,
}

/// Remote manifest listing skills the hub offers.
///
/// JSON shape: `{skills:[{name, description, url}]}`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HubManifest {
    /// Skills advertised by the hub.
    #[serde(default)]
    pub skills: Vec<HubSkillEntry>,
}

/// Hub URL precedence: explicit config value, else
/// `ZERO_HERMES_SKILLS_HUB` env var. `None` means the vendored pack is
/// used as-is (no network).
pub fn resolve_hub_url(configured: Option<&str>) -> Option<String> {
    if let Some(url) = configured {
        if !url.trim().is_empty() {
            return Some(url.trim().to_string());
        }
    }
    std::env::var("ZERO_HERMES_SKILLS_HUB")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Fetch and validate a hub manifest (JSON).
///
/// Only `http(s)://` URLs are accepted; anything else errors without
/// network access so tests stay offline.
pub async fn fetch_hub_manifest(url: &str) -> Result<HubManifest> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        anyhow::bail!("hub manifest URL must start with http:// or https://: {url}");
    }
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| anyhow::anyhow!("hub client build: {e}"))?
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("hub fetch failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("hub returned {status}"));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("hub body read: {e}"))?;
    let manifest: HubManifest =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("hub manifest not JSON: {e}"))?;
    validate_hub_manifest(&manifest)?;
    Ok(manifest)
}

/// Reject manifests with unsafe skill names or non-http(s) file URLs.
pub fn validate_hub_manifest(manifest: &HubManifest) -> Result<()> {
    for entry in &manifest.skills {
        if entry.name.is_empty()
            || !entry
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            anyhow::bail!("hub entry has invalid skill name: {:?}", entry.name);
        }
        if !(entry.url.starts_with("http://") || entry.url.starts_with("https://")) {
            anyhow::bail!(
                "hub entry {:?} has non-http(s) url: {}",
                entry.name,
                entry.url
            );
        }
    }
    Ok(())
}

/// Download every hub skill missing from `root`.
///
/// Each missing entry is fetched from its `url` and written to
/// `<root>/<name>/SKILL.md`. Returns the names that were added.
///
/// TODO(curator): run this as a background cron custom job (Wave D owns
/// the scheduler) that periodically diffs the vendored pack against the
/// hub manifest and files an update note instead of syncing inline, so
/// upstream drift (29 changed / 32 neither per bug 17) is surfaced
/// without blocking agent turns.
pub async fn sync_missing_skills(root: &Path, manifest_url: &str) -> Result<Vec<String>> {
    let manifest = fetch_hub_manifest(manifest_url).await?;
    let existing = SkillRegistry::load_dir(root).unwrap_or_default();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| anyhow::anyhow!("hub client build: {e}"))?;
    let mut added = Vec::new();
    for entry in manifest.skills {
        if existing.get(&entry.name).is_some() {
            continue;
        }
        if root.join(&entry.name).join("SKILL.md").exists() {
            continue;
        }
        let resp = client
            .get(&entry.url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("hub download {} failed: {e}", entry.name))?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "hub download {} returned {}",
                entry.name,
                resp.status()
            ));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("hub body read: {e}"))?;
        let dest = root.join(&entry.name).join("SKILL.md");
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, body)?;
        added.push(entry.name);
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let raw = "---\nname: demo\ndescription: A demo skill\n---\n\n# Demo\n\nbody line\n";
        let (front, body) = split_frontmatter(raw);
        assert!(front.contains("description: A demo skill"));
        assert!(body.contains("# Demo"));
    }

    #[test]
    fn no_frontmatter() {
        let raw = "# Just a doc\n";
        let (front, body) = split_frontmatter(raw);
        assert!(front.is_empty());
        assert!(body.contains("Just a doc"));
    }

    #[test]
    fn loads_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("alpha");
        let b = tmp.path().join("beta");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(
            a.join("SKILL.md"),
            "---\ndescription: alpha skill\n---\nalpha body\n",
        )
        .unwrap();
        fs::write(
            b.join("skill.md"),
            "---\ndescription: beta skill\n---\nbeta body\n",
        )
        .unwrap();
        let reg = SkillRegistry::load_dir(tmp.path()).unwrap();
        assert_eq!(reg.names(), vec!["alpha".to_string(), "beta".to_string()]);
        let a = reg.get("alpha").unwrap();
        assert_eq!(a.description, "alpha skill");
        assert!(a.body.contains("alpha body"));
    }

    #[test]
    fn loads_nested_hermes_layout_and_frontmatter_name() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("development").join("debugging");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: systematic-debugging\ndescription: debug carefully\n---\nbody\n",
        )
        .unwrap();

        let reg = SkillRegistry::load_dir(tmp.path()).unwrap();
        assert_eq!(reg.names(), vec!["systematic-debugging"]);
        assert_eq!(
            reg.get("systematic-debugging").unwrap().description,
            "debug carefully"
        );
    }

    #[test]
    fn keeps_duplicate_frontmatter_names_under_directory_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        for dir in ["first", "patch"] {
            let skill_dir = tmp.path().join(dir);
            fs::create_dir_all(&skill_dir).unwrap();
            fs::write(
                skill_dir.join("SKILL.md"),
                "---\nname: shared\ndescription: duplicate\n---\nbody\n",
            )
            .unwrap();
        }

        let reg = SkillRegistry::load_dir(tmp.path()).unwrap();
        assert_eq!(reg.names(), vec!["patch", "shared"]);
    }

    #[test]
    fn render_index_lists_all() {
        let mut reg = SkillRegistry::new();
        reg.insert(Skill {
            name: "x".into(),
            description: "x skill".into(),
            body: "".into(),
            path: PathBuf::from("/x"),
        });
        let s = reg.render_index();
        assert!(s.contains("x: x skill"));
    }

    #[test]
    fn render_index_capped_hints_at_more() {
        let mut reg = SkillRegistry::new();
        for name in ["aaa", "bbb", "ccc", "ddd", "eee", "fff", "ggg", "hhh"] {
            reg.insert(Skill {
                name: name.into(),
                description: format!("{name} skill"),
                body: String::new(),
                path: PathBuf::from(format!("/{name}")),
            });
        }
        let full = reg.render_index();
        assert!(full.contains("aaa") && full.contains("ddd"));
        // Tiny budget: first entry plus a "+N more" hint.
        let capped = reg.render_index_capped(60);
        assert!(capped.contains("aaa"), "at least one entry: {capped}");
        assert!(
            capped.contains("+") && capped.contains("skill(list)"),
            "must hint at discovery: {capped}"
        );
        assert!(capped.len() < full.len());
        // Generous budget: everything, no hint.
        let wide = reg.render_index_capped(10_000);
        assert!(!wide.contains("+"), "no truncation expected: {wide}");
        assert!(wide.contains("ddd"));
    }

    #[test]
    fn render_index_capped_empty_registry() {
        let reg = SkillRegistry::new();
        assert_eq!(reg.render_index_capped(2000), "(no skills loaded)");
    }

    #[tokio::test]
    async fn hub_rejects_non_http_manifest_url_without_network() {
        let err = fetch_hub_manifest("file:///tmp/manifest.json")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("http"), "{err}");
        let err = fetch_hub_manifest("not-a-url").await.unwrap_err();
        assert!(err.to_string().contains("http"), "{err}");
    }

    #[test]
    fn hub_manifest_validation_rejects_unsafe_entries() {
        let bad_name = HubManifest {
            skills: vec![HubSkillEntry {
                name: "../evil".into(),
                description: "x".into(),
                url: "https://example.test/evil/SKILL.md".into(),
            }],
        };
        assert!(validate_hub_manifest(&bad_name).is_err());
        let bad_url = HubManifest {
            skills: vec![HubSkillEntry {
                name: "ok".into(),
                description: "x".into(),
                url: "file:///etc/passwd".into(),
            }],
        };
        assert!(validate_hub_manifest(&bad_url).is_err());
        let good = HubManifest {
            skills: vec![HubSkillEntry {
                name: "ok-name".into(),
                description: "fine".into(),
                url: "https://example.test/ok/SKILL.md".into(),
            }],
        };
        assert!(validate_hub_manifest(&good).is_ok());
    }

    #[test]
    fn hub_url_prefers_config_over_env() {
        std::env::remove_var("ZERO_HERMES_SKILLS_HUB");
        assert_eq!(resolve_hub_url(None), None);
        assert_eq!(
            resolve_hub_url(Some("https://example.test/manifest.json")).as_deref(),
            Some("https://example.test/manifest.json")
        );
        std::env::set_var("ZERO_HERMES_SKILLS_HUB", "https://env.test/m.json");
        assert_eq!(
            resolve_hub_url(None).as_deref(),
            Some("https://env.test/m.json")
        );
        // Explicit config wins over env.
        assert_eq!(
            resolve_hub_url(Some("https://cfg.test/m.json")).as_deref(),
            Some("https://cfg.test/m.json")
        );
        std::env::remove_var("ZERO_HERMES_SKILLS_HUB");
    }
}
