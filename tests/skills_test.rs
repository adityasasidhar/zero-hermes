//! Integration tests for the SKILL.md loader.

use std::fs;
use std::path::PathBuf;

use zero_hermes::skills::{split_frontmatter, Skill, SkillRegistry};

/// Recursively count `SKILL.md` / `skill.md` files under `dir`.
fn count_skill_files(dir: &std::path::Path) -> usize {
    let mut n = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            n += count_skill_files(&path);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("skill.md"))
        {
            n += 1;
        }
    }
    n
}

#[test]
fn loads_bundled_hermes_skills() {
    // The repo vendors the nested Hermes skill pack. The registry keeps
    // duplicate frontmatter names under directory aliases, so a complete
    // recursive scan must yield exactly one entry per SKILL.md on disk; this
    // catches a wiped category or a loader that stops descending.
    let dir = PathBuf::from("skills");
    let reg = SkillRegistry::load_dir(&dir).unwrap();
    let files = count_skill_files(&dir);
    assert!(files > 100, "vendored skill pack looks truncated: {files}");
    assert_eq!(
        reg.names().len(),
        files,
        "recursive loader did not register every SKILL.md"
    );
    // Representative skills across categories prove recursive discovery and
    // declared-name parsing (frontmatter `name:` overrides the directory).
    assert!(reg.get("systematic-debugging").is_some());
    assert!(reg.get("github-code-review").is_some());
    let obsidian = reg.get("obsidian").unwrap();
    assert!(!obsidian.description.is_empty());
    assert!(obsidian.body.contains("Obsidian"));
}

#[test]
fn parses_frontmatter_with_blank_lines() {
    let raw = "---\nname: demo\ndescription: hello\n---\n\nbody\n";
    let (front, body) = split_frontmatter(raw);
    assert!(front.contains("description: hello"));
    assert!(body.trim_start().starts_with("body"));
}

#[test]
fn no_frontmatter_returns_body_only() {
    let raw = "# Plain doc\n\nhello\n";
    let (front, body) = split_frontmatter(raw);
    assert!(front.is_empty());
    assert!(body.contains("Plain doc"));
}

#[test]
fn loads_from_temp_dir() {
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
        b.join("SKILL.md"),
        "---\ndescription: beta skill\n---\nbeta body\n",
    )
    .unwrap();
    let reg = SkillRegistry::load_dir(tmp.path()).unwrap();
    assert_eq!(reg.names().len(), 2);
    assert_eq!(reg.get("alpha").unwrap().description, "alpha skill");
}

#[test]
fn render_index_empty() {
    let reg = SkillRegistry::new();
    assert!(reg.render_index().contains("no skills"));
}

#[test]
fn parse_minimal_skill() {
    let s = Skill::parse(
        "demo".into(),
        PathBuf::from("/x/SKILL.md"),
        "---\ndescription: d\n---\nbody",
    )
    .unwrap();
    assert_eq!(s.name, "demo");
    assert_eq!(s.description, "d");
    assert!(s.body.contains("body"));
}

#[test]
fn missing_dir_returns_empty_registry() {
    let reg = SkillRegistry::load_dir(&PathBuf::from("/nope/does/not/exist")).unwrap();
    assert!(reg.names().is_empty());
}
