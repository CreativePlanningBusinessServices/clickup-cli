use clickup_cli::commands::skill::{self, EMBEDDED_SKILL};

#[test]
fn install_writes_skill_into_dir_override() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("skills").join("clickup-cli");
    let result = skill::install(Some(&skill_dir)).unwrap();
    assert_eq!(result["installed"], true);
    let expected_path = skill_dir.join("SKILL.md");
    let written = std::fs::read_to_string(&expected_path).unwrap();
    assert_eq!(written, EMBEDDED_SKILL);
    assert_eq!(result["path"], expected_path.display().to_string());
}

#[test]
fn install_skips_when_content_current() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("skills").join("clickup-cli");
    let first = skill::install(Some(&skill_dir)).unwrap();
    assert_eq!(first["installed"], true);
    let second = skill::install(Some(&skill_dir)).unwrap();
    assert_eq!(second["installed"], false);
    assert_eq!(second["reason"], "already current");
}

#[test]
fn install_skips_a_symlinked_target_dir() {
    let base = tempfile::tempdir().unwrap();
    let real_dir = base.path().join("real");
    std::fs::create_dir_all(&real_dir).unwrap();
    let link_dir = base.path().join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&real_dir, &link_dir).unwrap();

    let result = skill::install(Some(&link_dir)).unwrap();
    assert_eq!(result["installed"], false);
    assert_eq!(
        result["reason"],
        "symlink — skill tracks its source repo via git"
    );
}

#[test]
fn embedded_skill_has_frontmatter_name() {
    assert!(EMBEDDED_SKILL.starts_with("---"));
    assert!(EMBEDDED_SKILL.contains("name: clickup-cli"));
}
