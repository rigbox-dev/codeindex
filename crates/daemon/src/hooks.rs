// Git hook integration — implemented in Task 17

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result};

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# codeindex: auto-generated git hook
if command -v codeindex >/dev/null 2>&1; then
    codeindex index --incremental --quiet &
fi
"#;

const HOOK_NAMES: &[&str] = &["post-checkout", "post-merge", "post-commit"];

/// Install codeindex git hooks into the project's `.git/hooks` directory.
///
/// For each hook:
/// - If the hook file already contains "codeindex", it is left unchanged.
/// - If the hook file exists without "codeindex", the codeindex lines are appended.
/// - If the hook file does not exist, the full `HOOK_SCRIPT` is written.
///
/// All hook files are set to mode 0o755.
pub fn install_hooks(project_root: &Path) -> Result<()> {
    let hooks_dir = project_root.join(".git").join("hooks");
    anyhow::ensure!(
        hooks_dir.is_dir(),
        "git hooks directory not found at {}",
        hooks_dir.display()
    );

    for hook_name in HOOK_NAMES {
        let hook_path = hooks_dir.join(hook_name);

        if hook_path.exists() {
            let content = fs::read_to_string(&hook_path)
                .with_context(|| format!("failed to read hook {}", hook_path.display()))?;

            if content.contains("codeindex") {
                // Already managed — skip.
                continue;
            }

            // Append codeindex lines (strip the shebang from HOOK_SCRIPT since the file
            // already has its own interpreter line).
            let lines_to_append: String = HOOK_SCRIPT
                .lines()
                .skip(1) // skip "#!/bin/sh"
                .map(|l| format!("{}\n", l))
                .collect();

            let new_content = format!("{}\n{}", content.trim_end(), lines_to_append);
            fs::write(&hook_path, new_content)
                .with_context(|| format!("failed to write hook {}", hook_path.display()))?;
        } else {
            fs::write(&hook_path, HOOK_SCRIPT)
                .with_context(|| format!("failed to write hook {}", hook_path.display()))?;
        }

        // Ensure the hook is executable.
        let mut perms = fs::metadata(&hook_path)
            .with_context(|| format!("failed to stat hook {}", hook_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .with_context(|| format!("failed to chmod hook {}", hook_path.display()))?;
    }

    Ok(())
}

/// Remove codeindex git hooks from the project's `.git/hooks` directory.
///
/// For each hook:
/// - If the hook file's content is exactly `HOOK_SCRIPT`, the file is removed.
/// - If the hook file contains other content plus the codeindex lines, only the
///   codeindex lines are filtered out.
pub fn uninstall_hooks(project_root: &Path) -> Result<()> {
    let hooks_dir = project_root.join(".git").join("hooks");

    for hook_name in HOOK_NAMES {
        let hook_path = hooks_dir.join(hook_name);

        if !hook_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&hook_path)
            .with_context(|| format!("failed to read hook {}", hook_path.display()))?;

        if !content.contains("codeindex") {
            continue;
        }

        // If content is exactly our script, just remove the file.
        if content == HOOK_SCRIPT {
            fs::remove_file(&hook_path)
                .with_context(|| format!("failed to remove hook {}", hook_path.display()))?;
            continue;
        }

        // Otherwise, filter out codeindex-related lines.
        let filtered: String = content
            .lines()
            .filter(|line| !line.contains("codeindex"))
            .map(|line| format!("{}\n", line))
            .collect();

        let trimmed = filtered.trim_end().to_string();
        if trimmed.is_empty() || trimmed == "#!/bin/sh" {
            fs::remove_file(&hook_path)
                .with_context(|| format!("failed to remove hook {}", hook_path.display()))?;
        } else {
            fs::write(&hook_path, format!("{}\n", trimmed))
                .with_context(|| format!("failed to write hook {}", hook_path.display()))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_git_hooks_dir(tmp: &TempDir) -> std::path::PathBuf {
        let hooks_dir = tmp.path().join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).expect("create .git/hooks");
        tmp.path().to_path_buf()
    }

    #[test]
    fn install_creates_hook_files() {
        let tmp = TempDir::new().expect("tempdir");
        let project_root = setup_git_hooks_dir(&tmp);

        install_hooks(&project_root).expect("install_hooks should succeed");

        for hook_name in HOOK_NAMES {
            let hook_path = project_root.join(".git").join("hooks").join(hook_name);
            assert!(hook_path.exists(), "hook file {} should exist", hook_name);

            let content = fs::read_to_string(&hook_path)
                .unwrap_or_else(|_| panic!("should read hook {}", hook_name));
            assert!(
                content.contains("codeindex index --incremental --quiet"),
                "hook {} should contain the codeindex command",
                hook_name
            );
        }
    }

    #[test]
    fn install_skips_existing_codeindex_hooks() {
        let tmp = TempDir::new().expect("tempdir");
        let project_root = setup_git_hooks_dir(&tmp);

        // Pre-install.
        install_hooks(&project_root).expect("first install");

        // Write a sentinel to detect if the file is rewritten.
        let hook_path = project_root
            .join(".git")
            .join("hooks")
            .join("post-commit");
        let original_content = fs::read_to_string(&hook_path).expect("read hook");

        // Second install should leave the file unchanged.
        install_hooks(&project_root).expect("second install");

        let after_content = fs::read_to_string(&hook_path).expect("read hook after");
        assert_eq!(
            original_content, after_content,
            "hook should not be modified on re-install"
        );
    }

    #[test]
    fn install_appends_to_existing_hook() {
        let tmp = TempDir::new().expect("tempdir");
        let project_root = setup_git_hooks_dir(&tmp);

        let hook_path = project_root
            .join(".git")
            .join("hooks")
            .join("post-commit");

        // Write an existing hook without codeindex.
        fs::write(&hook_path, "#!/bin/sh\necho hello\n").expect("write existing hook");

        install_hooks(&project_root).expect("install_hooks");

        let content = fs::read_to_string(&hook_path).expect("read hook");
        assert!(content.contains("echo hello"), "existing content preserved");
        assert!(
            content.contains("codeindex index --incremental --quiet"),
            "codeindex lines appended"
        );
    }

    #[test]
    fn uninstall_removes_hook_files() {
        let tmp = TempDir::new().expect("tempdir");
        let project_root = setup_git_hooks_dir(&tmp);

        install_hooks(&project_root).expect("install");
        uninstall_hooks(&project_root).expect("uninstall");

        for hook_name in HOOK_NAMES {
            let hook_path = project_root.join(".git").join("hooks").join(hook_name);
            assert!(
                !hook_path.exists(),
                "hook file {} should be removed",
                hook_name
            );
        }
    }

    #[test]
    fn uninstall_preserves_non_codeindex_content() {
        let tmp = TempDir::new().expect("tempdir");
        let project_root = setup_git_hooks_dir(&tmp);

        let hook_path = project_root
            .join(".git")
            .join("hooks")
            .join("post-commit");

        // Write an existing hook without codeindex, then install to append.
        fs::write(&hook_path, "#!/bin/sh\necho hello\n").expect("write existing hook");
        install_hooks(&project_root).expect("install");
        uninstall_hooks(&project_root).expect("uninstall");

        assert!(hook_path.exists(), "hook file should still exist");
        let content = fs::read_to_string(&hook_path).expect("read hook");
        assert!(content.contains("echo hello"), "non-codeindex content preserved");
        assert!(
            !content.contains("codeindex"),
            "codeindex lines removed"
        );
    }
}
