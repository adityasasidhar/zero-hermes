//! Skill curator: `skill archive`, the `.archive` exclusion, the curator
//! prompt and the `curate` subcommand.

use serde_json::json;
use zero_hermes::agent::tool::{Tool, ToolContext};
use zero_hermes::skills::{curator_prompt, SkillRegistry};
use zero_hermes::tools::builtin::SkillTool;

fn skill_md(name: &str) -> String {
    format!("---\nname: {name}\ndescription: does {name}\n---\n\nbody of {name}\n")
}

fn write_skill(root: &std::path::Path, rel: &str, name: &str) {
    let dir = root.join(rel);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), skill_md(name)).unwrap();
}

#[tokio::test]
async fn archive_moves_skill_out_of_the_registry_but_keeps_it_on_disk() {
    let root = tempfile::tempdir().unwrap();
    write_skill(root.path(), "alpha", "alpha");
    write_skill(root.path(), "cat/beta", "beta");
    let tool = SkillTool::new(root.path().to_path_buf());
    let ctx = ToolContext::default();

    let out = tool
        .execute(json!({"action": "archive", "name": "beta"}), &ctx)
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);

    let reg = SkillRegistry::load_dir(root.path()).unwrap();
    assert!(reg.get("beta").is_none(), "archived skill must not load");
    assert!(reg.get("alpha").is_some(), "other skills untouched");
    let archived: Vec<_> = std::fs::read_dir(root.path().join(".archive"))
        .unwrap()
        .collect();
    assert_eq!(archived.len(), 1, "skill is parked, not deleted");
    assert!(
        root.path().join("cat").exists(),
        "category dir is left alone"
    );
}

#[tokio::test]
async fn archive_refuses_unknown_traversal_and_root_level_skills() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("SKILL.md"), skill_md("rootish")).unwrap();
    write_skill(root.path(), "ok", "ok");
    let tool = SkillTool::new(root.path().to_path_buf());
    let ctx = ToolContext::default();

    for name in ["nope", "../etc", "a/b"] {
        let refused = match tool
            .execute(json!({"action": "archive", "name": name}), &ctx)
            .await
        {
            Err(_) => true,
            Ok(o) => o.is_error,
        };
        assert!(refused, "{name} must be refused");
    }
    let out = tool
        .execute(json!({"action": "archive", "name": "rootish"}), &ctx)
        .await
        .unwrap();
    assert!(
        out.is_error,
        "a SKILL.md at the root must not be archivable"
    );
    assert!(root.path().join("SKILL.md").exists());
    assert!(root.path().join("ok").exists());
}

#[test]
fn curator_prompt_lists_skills_and_forbids_deleting() {
    let root = tempfile::tempdir().unwrap();
    write_skill(root.path(), "alpha", "alpha");
    let reg = SkillRegistry::load_dir(root.path()).unwrap();
    let p = curator_prompt(&reg, root.path());
    assert!(p.contains("alpha"), "{p}");
    assert!(p.contains("archive"), "{p}");
    assert!(p.contains("never delete"), "{p}");
    let empty = curator_prompt(&SkillRegistry::new(), root.path());
    assert!(empty.contains("no skills yet"), "{empty}");
}

#[test]
fn curate_subcommand_runs_a_turn() {
    let sandbox = tempfile::tempdir().unwrap();
    let skills = sandbox.path().join("skills");
    write_skill(&skills, "alpha", "alpha");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_zero-hermes"))
        .args(["curate", "--mock"])
        .current_dir(sandbox.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", sandbox.path())
        .env("XDG_CONFIG_HOME", sandbox.path().join("c"))
        .env("XDG_DATA_HOME", sandbox.path().join("d"))
        .env("ZERO_HERMES_SKILLS_DIR", &skills)
        .env("RUST_LOG", "off")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("hello from mock"));
}
