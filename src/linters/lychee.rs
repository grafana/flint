use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::LycheeConfig;
use crate::files::FileList;
use crate::linters::LinterOutput;

pub async fn run(
    cfg: &LycheeConfig,
    file_list: &FileList,
    project_root: &Path,
    config_dir: &Path,
) -> LinterOutput {
    let lychee_cfg_raw = cfg.config.as_deref().unwrap_or("lychee.toml");
    let lychee_cfg = if Path::new(lychee_cfg_raw).is_relative() {
        config_dir
            .join(lychee_cfg_raw)
            .to_string_lossy()
            .into_owned()
    } else {
        lychee_cfg_raw.to_string()
    };

    let remap_args = build_remap_args(project_root).await;

    // Full mode: no merge base (shallow clone or --full flag)
    if file_list.merge_base.is_none() {
        return run_lychee_cmd(
            "Checking all links in all files",
            &lychee_cfg,
            &remap_args,
            &["."],
            false,
        )
        .await;
    }

    // Check if lychee config is in the changed file list
    let config_changed = file_list
        .files
        .iter()
        .any(|f| f.as_path() == Path::new(&lychee_cfg));

    if config_changed {
        let mut out = run_lychee_cmd(
            "Checking all links in all files",
            &lychee_cfg,
            &remap_args,
            &["."],
            false,
        )
        .await;
        let mut stderr = b"Config changes detected, falling back to full check.\n".to_vec();
        stderr.extend_from_slice(&out.stderr);
        out.stderr = stderr;
        return out;
    }

    // Diff mode: filter changed files to link-checkable extensions
    let checkable: Vec<String> = file_list
        .files
        .iter()
        .filter(|f| is_link_checkable(f))
        .map(|f| {
            f.strip_prefix(project_root)
                .unwrap_or(f)
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let mut all_ok = true;
    let mut combined_stdout = Vec::new();
    let mut combined_stderr = Vec::new();

    if !checkable.is_empty() {
        let file_refs: Vec<&str> = checkable.iter().map(String::as_str).collect();
        let out = run_lychee_cmd(
            "Checking all links in modified files",
            &lychee_cfg,
            &remap_args,
            &file_refs,
            false,
        )
        .await;
        all_ok &= out.ok;
        combined_stdout.extend_from_slice(&out.stdout);
        combined_stderr.extend_from_slice(&out.stderr);
    } else {
        combined_stdout.extend_from_slice(b"No modified files to check for all links.\n");
    }

    if cfg.check_all_local {
        let out = run_lychee_cmd(
            "Checking local links in all files",
            &lychee_cfg,
            &remap_args,
            &["."],
            true,
        )
        .await;
        all_ok &= out.ok;
        combined_stdout.extend_from_slice(&out.stdout);
        combined_stderr.extend_from_slice(&out.stderr);
    }

    LinterOutput {
        ok: all_ok,
        stdout: combined_stdout,
        stderr: combined_stderr,
    }
}

async fn run_lychee_cmd(
    description: &str,
    lychee_cfg: &str,
    remap_args: &[String],
    files: &[&str],
    local_only: bool,
) -> LinterOutput {
    let mut argv: Vec<String> = vec![
        "lychee".to_string(),
        "--config".to_string(),
        lychee_cfg.to_string(),
    ];

    if local_only {
        argv.push("--scheme".to_string());
        argv.push("file".to_string());
        argv.push("--include-fragments".to_string());
    }

    argv.extend_from_slice(remap_args);
    argv.push("--".to_string());
    argv.extend(files.iter().map(|s| s.to_string()));

    let mut stdout = format!("==> {description}\n").into_bytes();

    let result = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) => {
            stdout.extend_from_slice(&out.stdout);
            LinterOutput {
                ok: out.status.success(),
                stdout,
                stderr: out.stderr,
            }
        }
        Err(e) => LinterOutput {
            ok: false,
            stdout,
            stderr: format!("flint: links: failed to spawn lychee: {e}\n").into_bytes(),
        },
    }
}

async fn build_remap_args(project_root: &Path) -> Vec<String> {
    if std::env::var("LYCHEE_SKIP_GITHUB_REMAPS").as_deref() == Ok("true") {
        return vec![];
    }
    let mut args = build_global_github_args();
    args.extend(build_branch_remap_args(project_root).await);
    args
}

fn build_global_github_args() -> Vec<String> {
    let mut args = Vec::new();
    push_remap(
        &mut args,
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#L[0-9]+.*$ https://raw.githubusercontent.com/$1/$2/$3",
    );
    push_remap(
        &mut args,
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#:~:text=.*$ https://raw.githubusercontent.com/$1/$2/$3",
    );
    push_remap(
        &mut args,
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*)$ https://raw.githubusercontent.com/$1/$2/$3",
    );
    push_remap(
        &mut args,
        r"^https://github.com/([^/]+/[^/]+)/(issues|pull)/([0-9]+)#issuecomment-.*$ https://github.com/$1/$2/$3",
    );
    args
}

async fn build_branch_remap_args(project_root: &Path) -> Vec<String> {
    let Some(repo) = resolve_repo(project_root).await else {
        return vec![];
    };
    let base_ref = resolve_base_ref(project_root).await;
    let Some(head_ref) = resolve_head_ref(project_root).await else {
        return vec![];
    };

    if head_ref == base_ref {
        return vec![];
    }

    let head_repo = std::env::var("PR_HEAD_REPO").unwrap_or_else(|_| repo.clone());
    let base_url = format!("https://github.com/{repo}");
    let mut args = Vec::new();

    if head_repo == repo {
        let pwd = project_root.to_string_lossy();
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*?)#L[0-9]+.*$ file://{pwd}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*?)#:~:text=.*$ file://{pwd}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*)$ file://{pwd}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/tree/{base_ref}/(.*?)#L[0-9]+.*$ file://{pwd}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/tree/{base_ref}/(.*)$ file://{pwd}/$1"),
        );
    } else {
        let raw_head = format!("https://raw.githubusercontent.com/{head_repo}/{head_ref}");
        let head_url = format!("https://github.com/{head_repo}");
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*?)#L[0-9]+.*$ {raw_head}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*?)#:~:text=.*$ {raw_head}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/blob/{base_ref}/(.*)$ {raw_head}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/tree/{base_ref}/(.*?)#L[0-9]+.*$ {head_url}/tree/{head_ref}/$1"),
        );
        push_remap(
            &mut args,
            format!("^{base_url}/tree/{base_ref}/(.*)$ {head_url}/tree/{head_ref}/$1"),
        );
    }

    args
}

fn push_remap(args: &mut Vec<String>, pattern: impl Into<String>) {
    args.push("--remap".to_string());
    args.push(pattern.into());
}

/// Runs a git command and returns its trimmed stdout, or `None` if it fails or is empty.
async fn run_git_output(project_root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

async fn resolve_repo(project_root: &Path) -> Option<String> {
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.is_empty()
    {
        return Some(repo);
    }
    run_git_output(project_root, &["config", "--get", "remote.origin.url"])
        .await
        .and_then(|url| parse_github_repo(&url))
}

async fn resolve_base_ref(project_root: &Path) -> String {
    if let Ok(base) = std::env::var("GITHUB_BASE_REF")
        && !base.is_empty()
    {
        return base;
    }
    run_git_output(project_root, &["symbolic-ref", "refs/remotes/origin/HEAD"])
        .await
        .as_deref()
        .and_then(|s| s.rsplit('/').next())
        .map(String::from)
        .unwrap_or_else(|| "main".to_string())
}

async fn resolve_head_ref(project_root: &Path) -> Option<String> {
    if let Ok(head) = std::env::var("GITHUB_HEAD_REF")
        && !head.is_empty()
    {
        return Some(head);
    }
    run_git_output(project_root, &["rev-parse", "--abbrev-ref", "HEAD"]).await
}

fn parse_github_repo(url: &str) -> Option<String> {
    // HTTPS: https://github.com/owner/repo.git or https://github.com/owner/repo
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let repo = rest.trim_end_matches(".git");
        if !repo.is_empty() {
            return Some(repo.to_string());
        }
    }
    // SSH: git@github.com:owner/repo.git or git@github.com:owner/repo
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let repo = rest.trim_end_matches(".git");
        if !repo.is_empty() {
            return Some(repo.to_string());
        }
    }
    None
}

fn is_link_checkable(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "md" | "mkd"
            | "mdx"
            | "mdown"
            | "mdwn"
            | "mkdn"
            | "mkdown"
            | "markdown"
            | "html"
            | "htm"
            | "txt"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_repo_https() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_https_dotgit() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_ssh_dotgit() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_non_github() {
        assert_eq!(parse_github_repo("https://gitlab.com/owner/repo"), None);
    }

    #[test]
    fn parse_github_repo_empty_path() {
        assert_eq!(parse_github_repo("https://github.com/"), None);
    }

    #[test]
    fn is_link_checkable_md() {
        assert!(is_link_checkable(Path::new("README.md")));
        assert!(is_link_checkable(Path::new("docs/page.markdown")));
        assert!(is_link_checkable(Path::new("file.html")));
        assert!(is_link_checkable(Path::new("notes.txt")));
    }

    #[test]
    fn is_link_checkable_case_insensitive() {
        assert!(is_link_checkable(Path::new("README.MD")));
        assert!(is_link_checkable(Path::new("page.HTML")));
    }

    #[test]
    fn is_link_checkable_non_checkable() {
        assert!(!is_link_checkable(Path::new("main.rs")));
        assert!(!is_link_checkable(Path::new("config.toml")));
        assert!(!is_link_checkable(Path::new("script.sh")));
        assert!(!is_link_checkable(Path::new("Makefile")));
    }
}
