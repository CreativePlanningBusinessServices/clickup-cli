use std::sync::Mutex;

use clickup_cli::commands::skill::{self, EMBEDDED_SKILL};

/// `install_skips_home_fallback_when_claude_config_dir_missing` and
/// `install_creates_a_nonexistent_claude_config_dir_env_target` mutate the process-wide `HOME`/
/// `CLAUDE_CONFIG_DIR` env vars to exercise `install`'s home-fallback source (there's no way to
/// inject "home" through `install`'s public `dir_flag`-only signature). This lock serializes
/// just those two against each other; every other test here always passes an explicit `--dir`,
/// so `resolve_target` never consults the env/home it might transiently see mid-mutation.
static ENV_LOCK: Mutex<()> = Mutex::new(());

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
    assert_eq!(
        result["path"],
        link_dir.join("SKILL.md").display().to_string()
    );
}

// On Windows `directories` resolves the home dir through the Known Folder API,
// so overriding $HOME cannot redirect it into a tempdir — same limitation as
// tests/cli_smoke.rs, same gate.
#[cfg(not(windows))]
#[test]
fn install_skips_home_fallback_when_claude_config_dir_missing() {
    let _guard = ENV_LOCK.lock().unwrap();
    let home = tempfile::tempdir().unwrap();
    // Deliberately do NOT create home/.claude.
    let previous_home = std::env::var("HOME").ok();
    let previous_config_dir_env = std::env::var("CLAUDE_CONFIG_DIR").ok();
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    let result = skill::install(None);

    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_config_dir_env {
            Some(value) => std::env::set_var("CLAUDE_CONFIG_DIR", value),
            None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
        }
    }

    let result = result.unwrap();
    assert_eq!(result["installed"], false);
    assert_eq!(result["reason"], "no Claude config dir — pass --dir");
    assert_eq!(
        result["path"],
        home.path()
            .join(".claude")
            .join("skills")
            .join("clickup-cli")
            .join("SKILL.md")
            .display()
            .to_string()
    );
    assert!(
        !home.path().join(".claude").exists(),
        "install must not create ~/.claude on the un-created home fallback"
    );
}

#[test]
fn install_creates_a_nonexistent_claude_config_dir_env_target() {
    let _guard = ENV_LOCK.lock().unwrap();
    let base = tempfile::tempdir().unwrap();
    let config_dir = base.path().join("does-not-exist-yet");
    let previous_config_dir_env = std::env::var("CLAUDE_CONFIG_DIR").ok();
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", &config_dir);
    }

    let result = skill::install(None);

    unsafe {
        match previous_config_dir_env {
            Some(value) => std::env::set_var("CLAUDE_CONFIG_DIR", value),
            None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
        }
    }

    let result = result.unwrap();
    assert_eq!(result["installed"], true);
    let expected_path = config_dir
        .join("skills")
        .join("clickup-cli")
        .join("SKILL.md");
    assert!(expected_path.exists());
    assert_eq!(result["path"], expected_path.display().to_string());
}

#[test]
fn embedded_skill_has_frontmatter_name() {
    assert!(EMBEDDED_SKILL.starts_with("---"));
    assert!(EMBEDDED_SKILL.contains("name: clickup-cli"));
}
