use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::RenovateDepsConfig;

const SCRIPT: &str = include_str!("../../tasks/lint/renovate-deps.py");

pub async fn run(
    cfg: &RenovateDepsConfig,
    fix: bool,
    project_root: &Path,
) -> (bool, Vec<u8>, Vec<u8>) {
    let pid = std::process::id();
    let tmp_path = format!("/tmp/flint-renovate-deps-{pid}.py");

    if let Err(e) = std::fs::write(&tmp_path, SCRIPT) {
        let stderr =
            format!("flint: renovate-deps: failed to write temp script: {e}\n").into_bytes();
        return (false, vec![], stderr);
    }

    let mut cmd = Command::new("python3");
    cmd.arg(&tmp_path)
        .current_dir(project_root)
        .stdin(Stdio::null())
        .env("MISE_PROJECT_ROOT", project_root);

    if fix {
        cmd.env("AUTOFIX", "true");
    }

    if !cfg.exclude_managers.is_empty() {
        cmd.env(
            "RENOVATE_TRACKED_DEPS_EXCLUDE",
            cfg.exclude_managers.join(","),
        );
    }

    let result = cmd.output().await;

    // Remove temp file regardless of outcome
    let _ = std::fs::remove_file(&tmp_path);

    match result {
        Ok(out) => {
            let ok = out.status.success();
            (ok, out.stdout, out.stderr)
        }
        Err(e) => {
            let stderr =
                format!("flint: renovate-deps: failed to spawn python3: {e}\n").into_bytes();
            (false, vec![], stderr)
        }
    }
}
