use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Runs the flint binary in the given directory with the given args.
fn flint(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flint"))
        .args(args)
        .env("MISE_PROJECT_ROOT", cwd)
        .current_dir(cwd)
        .output()
        .expect("failed to spawn flint")
}

/// Creates a temp directory initialised as a git repo.
fn git_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for args in [
        vec!["init"],
        vec!["config", "user.email", "test@test.com"],
        vec!["config", "user.name", "Test"],
    ] {
        Command::new("git")
            .args(&args)
            .current_dir(dir.path())
            .output()
            .expect("git failed");
    }
    dir
}

// Helper to stage a file so it appears in `git ls-files` (used by --full).
fn stage(path: &Path, content: &str, repo: &Path) {
    std::fs::write(path, content).unwrap();
    Command::new("git")
        .args(["add", path.to_str().unwrap()])
        .current_dir(repo)
        .output()
        .expect("git add failed");
}

#[test]
fn shellcheck_failure_shows_check_name_header() {
    let repo = git_repo();

    // SC2086: unquoted variable — reliable shellcheck violation.
    stage(
        &repo.path().join("bad.sh"),
        "#!/bin/bash\necho $1\n",
        repo.path(),
    );

    let out = flint(&["--full", "shellcheck"], repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    println!("=== stdout ===\n{stdout}");
    eprintln!("=== stderr ===\n{stderr}");

    assert!(!out.status.success(), "flint should fail");
    assert!(
        stderr.contains("[shellcheck]"),
        "expected [shellcheck] header, got:\n{stderr}"
    );
}

#[test]
fn shellcheck_clean_script_passes() {
    let repo = git_repo();

    // A well-formed shell script — no violations.
    stage(
        &repo.path().join("good.sh"),
        "#!/bin/bash\necho \"$1\"\n",
        repo.path(),
    );

    let out = flint(&["--full", "shellcheck"], repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    println!("=== stdout ===\n{stdout}");
    eprintln!("=== stderr ===\n{stderr}");

    assert!(
        out.status.success(),
        "flint should pass, got:\n{stderr}"
    );
}
