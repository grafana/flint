use anyhow::Result;
use std::path::Path;

const HOOK_CONTENT: &str = "#!/bin/sh\n\
# Installed by flint — run `flint hook install` to reinstall\n\
flint run --fix --fast-only\n";

/// Writes `.git/hooks/pre-commit`. Skips silently if the hook already exists.
pub fn install(project_root: &Path) -> Result<()> {
    let git_dir = project_root.join(".git");
    if !git_dir.exists() {
        anyhow::bail!("not a git repository (no .git directory found)");
    }
    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-commit");
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
