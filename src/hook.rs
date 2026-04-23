use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

const HOOK_CONTENT: &str = "#!/bin/sh\n\
# Installed by flint — run `flint hook install` to reinstall\n\
mise exec -- flint run --fix --fast-only\n";

/// Returns the repository-local pre-commit hook path for this git checkout.
pub(crate) fn pre_commit_path(project_root: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(project_root)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            anyhow::bail!(
                "git rev-parse --git-common-dir failed with status {}",
                output.status
            );
        } else {
            anyhow::bail!(
                "git rev-parse --git-common-dir failed with status {}: {}",
                output.status,
                stderr
            );
        }
    }
    let path = String::from_utf8(output.stdout)?;
    let path = PathBuf::from(path.trim()).join("hooks/pre-commit");
    Ok(if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    })
}

/// Writes the repository-local git `hooks/pre-commit`. Skips silently if the hook already exists.
pub fn install(project_root: &Path) -> Result<()> {
    let hook_path = pre_commit_path(project_root)?;
    if let Some(hooks_dir) = hook_path.parent() {
        std::fs::create_dir_all(hooks_dir)?;
    }
    if hook_path.exists() {
        println!("pre-commit hook already installed");
        return Ok(());
    }
    std::fs::write(&hook_path, HOOK_CONTENT)?;
    set_executable(&hook_path)?;
    println!("installed pre-commit hook (.git/hooks/pre-commit)");
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HOOK_CONTENT, install, pre_commit_path};
    use std::process::Command;

    fn git(dir: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn install_supports_linked_worktree_gitfile() {
        let tmp = tempfile::TempDir::new().unwrap();
        let main = tmp.path().join("main");
        let worktree = tmp.path().join("worktree");
        std::fs::create_dir(&main).unwrap();

        git(&main, &["init", "-b", "main"]);
        git(&main, &["config", "user.email", "flint@example.com"]);
        git(&main, &["config", "user.name", "flint"]);
        std::fs::write(main.join("README.md"), "# demo\n").unwrap();
        git(&main, &["add", "README.md"]);
        git(&main, &["commit", "-m", "init"]);
        git(&main, &["worktree", "add", worktree.to_str().unwrap()]);

        install(&worktree).unwrap();
        let hook_path = pre_commit_path(&worktree).unwrap();
        assert_eq!(std::fs::read_to_string(hook_path).unwrap(), HOOK_CONTENT);
    }
}
