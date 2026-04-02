use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::LycheeConfig;
use crate::files::FileList;

pub async fn run(
    cfg: &LycheeConfig,
    file_list: &FileList,
    project_root: &Path,
) -> (bool, Vec<u8>, Vec<u8>) {
    let lychee_cfg = cfg
        .config
        .as_deref()
        .unwrap_or(".github/config/lychee.toml")
        .to_string();

    let remap_args = build_remap_args(project_root);

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
    let config_changed = file_list.files.iter().any(|f| {
        let rel = f.strip_prefix(project_root).unwrap_or(f);
        rel == Path::new(&lychee_cfg)
    });

    if config_changed {
        let mut stderr = b"Config changes detected, falling back to full check.\n".to_vec();
        let (ok, stdout, extra_stderr) = run_lychee_cmd(
            "Checking all links in all files",
            &lychee_cfg,
            &remap_args,
            &["."],
            false,
        )
        .await;
        stderr.extend_from_slice(&extra_stderr);
        return (ok, stdout, stderr);
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
        let (ok, stdout, stderr) = run_lychee_cmd(
            "Checking all links in modified files",
            &lychee_cfg,
            &remap_args,
            &file_refs,
            false,
        )
        .await;
        if !ok {
            all_ok = false;
        }
        combined_stdout.extend_from_slice(&stdout);
        combined_stderr.extend_from_slice(&stderr);
    } else {
        combined_stdout.extend_from_slice(b"No modified files to check for all links.\n");
    }

    if cfg.check_all_local {
        let (ok, stdout, stderr) = run_lychee_cmd(
            "Checking local links in all files",
            &lychee_cfg,
            &remap_args,
            &["."],
            true,
        )
        .await;
        if !ok {
            all_ok = false;
        }
        combined_stdout.extend_from_slice(&stdout);
        combined_stderr.extend_from_slice(&stderr);
    }

    (all_ok, combined_stdout, combined_stderr)
}

async fn run_lychee_cmd(
    description: &str,
    lychee_cfg: &str,
    remap_args: &[String],
    files: &[&str],
    local_only: bool,
) -> (bool, Vec<u8>, Vec<u8>) {
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

    let mut stdout_prefix = format!("==> {description}\n").into_bytes();

    let result = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) => {
            stdout_prefix.extend_from_slice(&out.stdout);
            let ok = out.status.success();
            (ok, stdout_prefix, out.stderr)
        }
        Err(e) => {
            let stderr = format!("flint: links: failed to spawn lychee: {e}\n").into_bytes();
            (false, stdout_prefix, stderr)
        }
    }
}

fn build_remap_args(project_root: &Path) -> Vec<String> {
    if std::env::var("LYCHEE_SKIP_GITHUB_REMAPS").as_deref() == Ok("true") {
        return vec![];
    }

    let mut args = build_global_github_args();
    args.extend(build_branch_remap_args(project_root));
    args
}

fn build_global_github_args() -> Vec<String> {
    vec![
        "--remap".to_string(),
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#L[0-9]+.*$ https://raw.githubusercontent.com/$1/$2/$3".to_string(),
        "--remap".to_string(),
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#:~:text=.*$ https://raw.githubusercontent.com/$1/$2/$3".to_string(),
        "--remap".to_string(),
        r"^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*)$ https://raw.githubusercontent.com/$1/$2/$3".to_string(),
        "--remap".to_string(),
        r"^https://github.com/([^/]+/[^/]+)/(issues|pull)/([0-9]+)#issuecomment-.*$ https://github.com/$1/$2/$3".to_string(),
    ]
}

fn build_branch_remap_args(project_root: &Path) -> Vec<String> {
    let repo = match resolve_repo(project_root) {
        Some(r) => r,
        None => return vec![],
    };

    let base_ref = resolve_base_ref(project_root);

    let head_ref = match resolve_head_ref(project_root) {
        Some(r) => r,
        None => return vec![],
    };

    // Skip if on the base branch
    if head_ref == base_ref {
        return vec![];
    }

    let head_repo = std::env::var("PR_HEAD_REPO").unwrap_or_else(|_| repo.clone());

    let base_url = format!("https://github.com/{repo}");

    if head_repo == repo {
        // Same-repo PR: remap to local file paths
        let pwd = project_root.to_string_lossy();
        vec![
            // /blob/ rule 1: line-number anchors
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*?)#L[0-9]+.*$ file://{pwd}/$1"),
            // /blob/ rule 2: scroll to text fragments
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*?)#:~:text=.*$ file://{pwd}/$1"),
            // /blob/ rule 3: all other blob URLs
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*)$ file://{pwd}/$1"),
            // /tree/ rule 4: line-number anchors on tree URLs
            "--remap".to_string(),
            format!("^{base_url}/tree/{base_ref}/(.*?)#L[0-9]+.*$ file://{pwd}/$1"),
            // /tree/ rule 5: non-fragment tree URLs
            "--remap".to_string(),
            format!("^{base_url}/tree/{base_ref}/(.*)$ file://{pwd}/$1"),
        ]
    } else {
        // Fork PR: remap to raw.githubusercontent.com and github.com head branch
        let raw_head = format!("https://raw.githubusercontent.com/{head_repo}/{head_ref}");
        let head_url = format!("https://github.com/{head_repo}");
        vec![
            // /blob/ rule 1: line-number anchors
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*?)#L[0-9]+.*$ {raw_head}/$1"),
            // /blob/ rule 2: scroll to text fragments
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*?)#:~:text=.*$ {raw_head}/$1"),
            // /blob/ rule 3: all other blob URLs
            "--remap".to_string(),
            format!("^{base_url}/blob/{base_ref}/(.*)$ {raw_head}/$1"),
            // /tree/ rule 4: line-number anchors on tree URLs
            "--remap".to_string(),
            format!("^{base_url}/tree/{base_ref}/(.*?)#L[0-9]+.*$ {head_url}/tree/{head_ref}/$1"),
            // /tree/ rule 5: non-fragment tree URLs
            "--remap".to_string(),
            format!("^{base_url}/tree/{base_ref}/(.*)$ {head_url}/tree/{head_ref}/$1"),
        ]
    }
}

fn resolve_repo(project_root: &Path) -> Option<String> {
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.is_empty()
    {
        return Some(repo);
    }

    let out = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_github_repo(&url)
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

fn resolve_base_ref(project_root: &Path) -> String {
    if let Ok(base) = std::env::var("GITHUB_BASE_REF")
        && !base.is_empty()
    {
        return base;
    }

    let out = std::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(project_root)
        .output();

    if let Ok(out) = out
        && out.status.success()
    {
        let full = String::from_utf8_lossy(&out.stdout).trim().to_string();
        // refs/remotes/origin/main → main
        if let Some(branch) = full.rsplit('/').next()
            && !branch.is_empty()
        {
            return branch.to_string();
        }
    }

    "main".to_string()
}

fn resolve_head_ref(project_root: &Path) -> Option<String> {
    if let Ok(head) = std::env::var("GITHUB_HEAD_REF")
        && !head.is_empty()
    {
        return Some(head);
    }

    let out = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
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
