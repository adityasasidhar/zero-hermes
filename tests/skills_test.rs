//! Integration tests for the SKILL.md loader.

use std::fs;
use std::path::PathBuf;

use zero_hermes::skills::{split_frontmatter, Skill, SkillRegistry};

#[test]
fn loads_nested_category_layout() {
    // The Hermes pack nests skills under category directories
    // (`<category>/<skill>/SKILL.md`). The registry keeps duplicate
    // frontmatter names under directory aliases, so a complete recursive scan
    // must yield exactly one entry per SKILL.md on disk.
    let tmp = tempfile::tempdir().unwrap();
    let write = |rel: &str, raw: &str| {
        let dir = tmp.path().join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), raw).unwrap();
    };
    write(
        "devops/deploy",
        "---\ndescription: ship it\n---\nDeploy body\n",
    );
    // Frontmatter `name:` overrides the directory name. The `a-` / `z-`
    // prefixes pin traversal order (entries sort by file name), so this
    // skill reliably wins the declared name and the one below takes the
    // directory alias — without them the duplicate winner depends on sort
    // order and the assertions below flip.
    write(
        "research/a-papers/arxiv",
        "---\nname: paper-search\ndescription: find papers\n---\nArxiv body\n",
    );
    // A second skill declaring an already-taken name is kept under its
    // directory alias rather than dropped.
    write(
        "research/z-other/dup",
        "---\nname: paper-search\ndescription: duplicate\n---\ndup body\n",
    );

    let reg = SkillRegistry::load_dir(tmp.path()).unwrap();
    assert_eq!(
        reg.names().len(),
        3,
        "recursive loader did not register every SKILL.md"
    );
    assert_eq!(reg.get("deploy").unwrap().description, "ship it");
    assert!(reg.get("paper-search").is_some());
    assert!(reg.get("arxiv").is_none(), "declared name must win");
    assert!(reg.get("dup").is_some(), "duplicate kept under dir alias");
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
