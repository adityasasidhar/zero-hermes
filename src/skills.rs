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
        format!("- {}: {}", self.name, self.description)
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

    /// Build the registry by scanning `dir` for `*/SKILL.md` and
    /// `*/skill.md` (case-insensitive).
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::info!(?dir, "skills dir does not exist");
            return Ok(reg);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path();
            for name in ["SKILL.md", "skill.md"] {
                let candidate = path.join(name);
                if candidate.exists() {
                    let skill = Skill::from_file(&candidate)?;
                    reg.insert(skill);
                    break;
                }
            }
        }
        Ok(reg)
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

    /// Load a skill from a file. The `name` is inferred from the parent
    /// directory if the frontmatter does not provide one.
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("skill")
            .to_string();
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
}
