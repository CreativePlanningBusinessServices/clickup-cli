use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::CliError;

pub const EMBEDDED_SKILL: &str = include_str!("../../skills/clickup-cli/SKILL.md");

const SKILL_FILE: &str = "SKILL.md";
const SKILL_NAME: &str = "clickup-cli";

/// Writes the embedded SKILL.md to the resolved target, skipping (never failing) when the
/// target is a symlinked/repo-tracked copy, already holds identical content, or there's nowhere
/// to put it (no `--dir`, no `$CLAUDE_CONFIG_DIR`, no home directory).
pub fn install(dir_flag: Option<&Path>) -> Result<Value, CliError> {
    let claude_config_dir_env = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let home = directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
    let target = resolve_target(dir_flag, claude_config_dir_env.as_deref(), home.as_deref());

    let path = match target {
        Err(_) => {
            eprintln!(
                "skill not installed (no Claude config dir); place it yourself with \
                 `clickup-cli skill install --dir <your skills dir>`"
            );
            return Ok(skip_json("no Claude config dir"));
        }
        Ok(path) => path,
    };

    // Re-check symlink + content here (resolve_target is pure and cannot touch disk).
    if is_symlink(&path) || path.parent().is_some_and(is_symlink) {
        return Ok(skip_json("symlink — skill tracks its source repo via git"));
    }
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == EMBEDDED_SKILL) {
        return Ok(installed_json(&path, false, Some("already current")));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|io_error| {
            CliError::Usage(format!("cannot create {}: {io_error}", parent.display()))
        })?;
    }
    std::fs::write(&path, EMBEDDED_SKILL).map_err(|io_error| {
        CliError::Usage(format!("cannot write {}: {io_error}", path.display()))
    })?;
    Ok(installed_json(&path, true, None))
}

/// Pure precedence resolution for the install target: `--dir` wins, then `$CLAUDE_CONFIG_DIR`,
/// then the user's home directory (`~/.claude/skills/clickup-cli/SKILL.md`). No disk I/O, so
/// it's unit-testable without a filesystem. A directory that already names `SKILL.md` is used
/// verbatim rather than having it appended again. Errs only when none of the three sources is
/// available — `install` turns that into a non-fatal skip.
pub fn resolve_target(
    dir_flag: Option<&Path>,
    claude_config_dir_env: Option<&str>,
    home: Option<&Path>,
) -> Result<PathBuf, CliError> {
    if let Some(dir) = dir_flag {
        return Ok(target_path(dir));
    }
    if let Some(env_dir) = claude_config_dir_env.filter(|value| !value.is_empty()) {
        return Ok(target_path(
            &Path::new(env_dir).join("skills").join(SKILL_NAME),
        ));
    }
    if let Some(home_dir) = home {
        return Ok(target_path(
            &home_dir.join(".claude").join("skills").join(SKILL_NAME),
        ));
    }
    Err(CliError::Usage("no Claude config dir".into()))
}

fn target_path(dir: &Path) -> PathBuf {
    if dir.file_name().is_some_and(|name| name == SKILL_FILE) {
        dir.to_path_buf()
    } else {
        dir.join(SKILL_FILE)
    }
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

fn skip_json(reason: &str) -> Value {
    json!({"skill": SKILL_NAME, "installed": false, "reason": reason})
}

fn installed_json(path: &Path, installed: bool, reason: Option<&str>) -> Value {
    let mut result = json!({
        "skill": SKILL_NAME, "path": path.display().to_string(), "installed": installed,
    });
    if let Some(reason) = reason {
        result["reason"] = json!(reason);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn embedded_skill_carries_the_real_frontmatter() {
        // Tolerate CRLF: a Windows checkout without the .gitattributes eol=lf rule would
        // embed the file with \r\n. The .gitattributes rule keeps it LF everywhere, but the
        // assertion shouldn't itself be line-ending-fragile.
        let head: String = EMBEDDED_SKILL.chars().filter(|ch| *ch != '\r').collect();
        assert!(head.starts_with("---\nname: clickup-cli"));
        assert!(EMBEDDED_SKILL.len() > 1000);
    }

    #[test]
    fn resolve_target_prefers_dir_flag_over_env_and_home() {
        let path = resolve_target(
            Some(Path::new("/flag/dest")),
            Some("/env/dest"),
            Some(Path::new("/home/x")),
        )
        .unwrap();
        assert_eq!(path, PathBuf::from("/flag/dest/SKILL.md"));
    }

    #[test]
    fn resolve_target_prefers_env_over_home_when_no_dir_flag() {
        let path = resolve_target(None, Some("/env/dest"), Some(Path::new("/home/x"))).unwrap();
        assert_eq!(path, PathBuf::from("/env/dest/skills/clickup-cli/SKILL.md"));
    }

    #[test]
    fn resolve_target_falls_back_to_home_when_no_dir_flag_or_env() {
        let path = resolve_target(None, None, Some(Path::new("/home/x"))).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/x/.claude/skills/clickup-cli/SKILL.md")
        );
    }

    #[test]
    fn resolve_target_errs_when_nothing_is_available() {
        assert!(resolve_target(None, None, None).is_err());
    }

    #[test]
    fn dir_flag_that_already_names_skill_md_is_used_verbatim() {
        let path = resolve_target(Some(Path::new("/tmp/dest/SKILL.md")), None, None).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/dest/SKILL.md"));
    }
}
