use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::{Config, LycheeConfig, Settings};
use crate::files::FileList;
use crate::linters::LinterOutput;
use crate::linters::env;
use crate::registry::{SpecialKind, StaticLinter};

const GITHUB_BASE_REF_ENV: &str = "GITHUB_BASE_REF";
const GITHUB_EVENT_NAME_ENV: &str = "GITHUB_EVENT_NAME";
const GITHUB_HEAD_REF_ENV: &str = "GITHUB_HEAD_REF";
const GITHUB_REPOSITORY_ENV: &str = "GITHUB_REPOSITORY";
const PR_HEAD_REPO_ENV: &str = "PR_HEAD_REPO";
const PR_LINK_REMAP_ENV_VARS: &[&str] = &[
    GITHUB_REPOSITORY_ENV,
    GITHUB_BASE_REF_ENV,
    GITHUB_HEAD_REF_ENV,
    PR_HEAD_REPO_ENV,
];

pub(crate) static LINTER: StaticLinter =
    StaticLinter::special_with_bin("lychee", "lychee", SpecialKind::Links, false);

pub async fn run(
    cfg: &LycheeConfig,
    settings: &Settings,
    file_list: &FileList,
    project_root: &Path,
    config_dir: &Path,
) -> LinterOutput {
    match validate_runtime_env(file_list) {
        Ok(Some(warning)) => eprintln!("{warning}"),
        Ok(None) => {}
        Err(stderr) => {
            return LinterOutput {
                ok: false,
                stdout: Vec::new(),
                stderr: stderr.into_bytes(),
            };
        }
    }

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
    let checkable_all_files = match lychee_checkable_files(project_root, settings) {
        Ok(files) => files,
        Err(e) => {
            return LinterOutput {
                ok: false,
                stdout: Vec::new(),
                stderr: format!("flint: links: failed to collect files: {e}\n").into_bytes(),
            };
        }
    };
    let checkable_all_file_refs: Vec<&str> =
        checkable_all_files.iter().map(String::as_str).collect();

    // Full mode: no merge base (shallow clone or --full flag)
    if file_list.merge_base.is_none() {
        return run_lychee_cmd(
            "Checking all links in all files",
            &lychee_cfg,
            &remap_args,
            &checkable_all_file_refs,
            false,
            project_root,
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
            &checkable_all_file_refs,
            false,
            project_root,
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
            project_root,
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
            &checkable_all_file_refs,
            true,
            project_root,
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

fn validate_runtime_env(file_list: &FileList) -> Result<Option<String>, String> {
    validate_runtime_env_from(file_list.full, github_remaps_enabled(), |name| {
        std::env::var(name).ok()
    })
}

fn validate_runtime_env_from<F>(
    full: bool,
    github_remaps_enabled: bool,
    env: F,
) -> Result<Option<String>, String>
where
    F: Fn(&str) -> Option<String>,
{
    let is_ci = env::is_ci_from(&env);
    let has_github_token = env::github_token_available(&env);

    let mut missing = Vec::new();
    if is_ci && !has_github_token {
        missing.push(env::GITHUB_TOKEN_ENV);
    }

    if is_ci && github_remaps_enabled && !full && is_github_pr_event(&env) {
        missing.extend(
            PR_LINK_REMAP_ENV_VARS
                .iter()
                .copied()
                .filter(|name| !env::env_non_empty(&env, name)),
        );
    }

    if !missing.is_empty() {
        return Err(missing_ci_env_message(&missing));
    }

    if !is_ci && !has_github_token {
        return Ok(Some(env::token_warning("lychee", env::GITHUB_TOKEN_ENV)));
    }

    Ok(None)
}

fn github_remaps_enabled() -> bool {
    std::env::var("LYCHEE_SKIP_GITHUB_REMAPS").as_deref() != Ok("true")
}

fn is_github_pr_event<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env(GITHUB_EVENT_NAME_ENV)
        .map(|event| matches!(event.as_str(), "pull_request" | "pull_request_target"))
        .unwrap_or_else(|| {
            env::env_non_empty(env, GITHUB_BASE_REF_ENV)
                || env::env_non_empty(env, GITHUB_HEAD_REF_ENV)
                || env::env_non_empty(env, PR_HEAD_REPO_ENV)
        })
}

fn missing_ci_env_message(missing: &[&str]) -> String {
    let noun = if missing.len() == 1 {
        "variable"
    } else {
        "variables"
    };
    let mut message = format!(
        "flint: links: missing required CI environment {noun}: {}\n",
        missing.join(", ")
    );
    if missing.contains(&env::GITHUB_TOKEN_ENV) {
        message.push_str(&format!(
            "  Set {token} so lychee can authenticate GitHub link checks in CI.\n",
            token = env::GITHUB_TOKEN_ENV,
        ));
    }
    if missing
        .iter()
        .any(|name| PR_LINK_REMAP_ENV_VARS.contains(name))
    {
        message.push_str(&format!(
            "  PR link remaps in CI require GitHub PR metadata; set {pr_head_repo} to github.event.pull_request.head.repo.full_name.\n",
            pr_head_repo = PR_HEAD_REPO_ENV,
        ));
    }
    message
}

fn lychee_checkable_files(project_root: &Path, settings: &Settings) -> anyhow::Result<Vec<String>> {
    let cfg = Config {
        settings: settings.clone(),
        ..Config::default()
    };
    let all_files = crate::files::all(project_root, &cfg)?;
    Ok(all_files
        .files
        .iter()
        .filter(|f| is_link_checkable(f))
        .map(|f| {
            f.strip_prefix(project_root)
                .unwrap_or(f)
                .to_string_lossy()
                .into_owned()
        })
        .collect())
}

async fn run_lychee_cmd(
    description: &str,
    lychee_cfg: &str,
    remap_args: &[String],
    files: &[&str],
    local_only: bool,
    project_root: &Path,
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

    let result = super::spawn_command(&argv, false)
        .current_dir(project_root)
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

/// Returns the GitHub server URL from `GITHUB_SERVER_URL`, defaulting to `https://github.com`.
fn github_server_url() -> String {
    std::env::var("GITHUB_SERVER_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://github.com".to_string())
}

/// Returns the base URL for raw file content.
/// GitHub.com uses a separate subdomain; GitHub Enterprise serves raw content at `{server}/raw`.
fn raw_content_base(server_url: &str) -> String {
    if server_url == "https://github.com" {
        "https://raw.githubusercontent.com".to_string()
    } else {
        format!("{server_url}/raw")
    }
}

async fn build_remap_args(project_root: &Path) -> Vec<String> {
    if std::env::var("LYCHEE_SKIP_GITHUB_REMAPS").as_deref() == Ok("true") {
        return vec![];
    }
    let server = github_server_url();
    let raw_base = raw_content_base(&server);
    let mut args = build_global_github_args(&server, &raw_base);
    args.extend(build_branch_remap_args(project_root, &server, &raw_base).await);
    args
}

fn build_global_github_args(server: &str, raw_base: &str) -> Vec<String> {
    let mut args = Vec::new();
    push_remap(
        &mut args,
        format!(r"^{server}/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#L[0-9]+.*$ {raw_base}/$1/$2/$3"),
    );
    push_remap(
        &mut args,
        format!(r"^{server}/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#:~:text=.*$ {raw_base}/$1/$2/$3"),
    );
    push_remap(
        &mut args,
        format!(r"^{server}/([^/]+/[^/]+)/blob/([^/]+)/(.*)$ {raw_base}/$1/$2/$3"),
    );
    push_remap(
        &mut args,
        format!(
            r"^{server}/([^/]+/[^/]+)/(issues|pull)/([0-9]+)#issuecomment-.*$ {server}/$1/$2/$3"
        ),
    );
    args
}

async fn build_branch_remap_args(project_root: &Path, server: &str, raw_base: &str) -> Vec<String> {
    let Some(repo) = resolve_repo(project_root, server).await else {
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
    let base_url = format!("{server}/{repo}");
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
        let raw_head = format!("{raw_base}/{head_repo}/{head_ref}");
        let head_url = format!("{server}/{head_repo}");
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

async fn resolve_repo(project_root: &Path, server: &str) -> Option<String> {
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.is_empty()
    {
        return Some(repo);
    }
    run_git_output(project_root, &["config", "--get", "remote.origin.url"])
        .await
        .and_then(|url| parse_github_repo(&url, server))
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

fn parse_github_repo(url: &str, server: &str) -> Option<String> {
    // HTTPS: https://<server>/owner/repo.git or https://<server>/owner/repo
    let https_prefix = format!("{server}/");
    if let Some(rest) = url.strip_prefix(https_prefix.as_str()) {
        let repo = rest.trim_end_matches(".git");
        if !repo.is_empty() {
            return Some(repo.to_string());
        }
    }
    // SSH: git@<hostname>:owner/repo.git or git@<hostname>:owner/repo
    let hostname = server.strip_prefix("https://").unwrap_or(server);
    let ssh_prefix = format!("git@{hostname}:");
    if let Some(rest) = url.strip_prefix(ssh_prefix.as_str()) {
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
    use crate::config::Settings;
    use std::collections::HashMap;

    fn validate_env(
        full: bool,
        github_remaps_enabled: bool,
        vars: &[(&str, &str)],
    ) -> Result<Option<String>, String> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        validate_runtime_env_from(full, github_remaps_enabled, |name| vars.get(name).cloned())
    }

    #[test]
    fn ci_requires_github_token_even_in_full_mode() {
        let err = validate_env(true, true, &[("CI", "true")]).unwrap_err();

        assert!(err.contains("GITHUB_TOKEN"), "unexpected error:\n{err}");
    }

    #[test]
    fn ci_pr_diff_requires_link_remap_env_vars() {
        let err = validate_env(
            false,
            true,
            &[
                ("CI", "true"),
                ("GITHUB_EVENT_NAME", "pull_request"),
                ("GITHUB_TOKEN", "token"),
            ],
        )
        .unwrap_err();

        for name in [
            "GITHUB_REPOSITORY",
            "GITHUB_BASE_REF",
            "GITHUB_HEAD_REF",
            "PR_HEAD_REPO",
        ] {
            assert!(err.contains(name), "missing {name} in error:\n{err}");
        }
    }

    #[test]
    fn ci_full_mode_does_not_require_link_remap_env_vars() {
        let result = validate_env(
            true,
            true,
            &[
                ("CI", "true"),
                ("GITHUB_EVENT_NAME", "pull_request"),
                ("GITHUB_TOKEN", "token"),
            ],
        );

        assert!(result.is_ok(), "unexpected validation error: {result:?}");
    }

    #[test]
    fn ci_pr_diff_allows_missing_link_remap_env_vars_when_remaps_are_disabled() {
        let result = validate_env(
            false,
            false,
            &[
                ("CI", "true"),
                ("GITHUB_EVENT_NAME", "pull_request"),
                ("GITHUB_TOKEN", "token"),
            ],
        );

        assert!(result.is_ok(), "unexpected validation error: {result:?}");
    }

    #[test]
    fn non_ci_missing_github_token_warns_without_failing() {
        let warning = validate_env(false, true, &[]).unwrap().unwrap();

        assert!(warning.contains("GITHUB_TOKEN"));
    }

    #[test]
    fn parse_github_repo_https() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo", "https://github.com"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_https_dotgit() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git", "https://github.com"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo", "https://github.com"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_ssh_dotgit() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo.git", "https://github.com"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_non_github() {
        assert_eq!(
            parse_github_repo("https://gitlab.com/owner/repo", "https://github.com"),
            None
        );
    }

    #[test]
    fn parse_github_repo_empty_path() {
        assert_eq!(
            parse_github_repo("https://github.com/", "https://github.com"),
            None
        );
    }

    #[test]
    fn parse_github_repo_ghe_https() {
        assert_eq!(
            parse_github_repo(
                "https://github.mycompany.com/owner/repo",
                "https://github.mycompany.com"
            ),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_github_repo_ghe_ssh() {
        assert_eq!(
            parse_github_repo(
                "git@github.mycompany.com:owner/repo.git",
                "https://github.mycompany.com"
            ),
            Some("owner/repo".to_string())
        );
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

    #[test]
    fn lychee_checkable_files_respects_flint_excludes() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("tests/cases/demo")).unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("README.md"), "# ok\n").unwrap();
        std::fs::write(tmp.path().join("docs/page.md"), "# doc\n").unwrap();
        std::fs::write(tmp.path().join("tests/cases/demo/README.md"), "# fixture\n").unwrap();

        let out = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let out = std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git add failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let files = lychee_checkable_files(
            tmp.path(),
            &Settings {
                base_branch: "main".to_string(),
                exclude: vec!["tests/cases/**".to_string()],
                setup_migration_version: crate::setup::V2_BASELINE_SETUP_VERSION,
            },
        )
        .unwrap();

        assert!(files.contains(&"README.md".to_string()));
        assert!(files.contains(&"docs/page.md".to_string()));
        assert!(!files.contains(&"tests/cases/demo/README.md".to_string()));
    }
}
