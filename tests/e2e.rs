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

/// Writes a minimal mise.toml declaring the given tools so flint's availability
/// check passes. Version strings are arbitrary — these checks have no version_range.
fn write_mise_toml(repo: &TempDir, tools: &[&str]) {
    let entries: String = tools
        .iter()
        .map(|t| format!("{t} = \"latest\"\n"))
        .collect();
    std::fs::write(repo.path().join("mise.toml"), format!("[tools]\n{entries}")).unwrap();
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
    write_mise_toml(&repo, &["shellcheck"]);

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
fn cargo_fmt_diff_shows_check_name_header() {
    let repo = git_repo();
    write_mise_toml(&repo, &["rust"]);

    // Minimal Cargo project with a badly formatted Rust file.
    std::fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let src = repo.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    // Poorly formatted: fields on one line, which rustfmt will expand.
    stage(
        &src.join("lib.rs"),
        "pub struct Foo { pub a: u32, pub b: u32 }\n",
        repo.path(),
    );

    let out = flint(&["--full", "cargo-fmt"], repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    println!("=== stdout ===\n{stdout}");
    eprintln!("=== stderr ===\n{stderr}");

    assert!(!out.status.success(), "flint should fail");
    assert!(
        stderr.contains("[cargo-fmt]"),
        "expected [cargo-fmt] header, got:\n{stderr}"
    );
}

#[test]
fn auto_fixes_and_reports_summary() {
    let repo = git_repo();
    write_mise_toml(&repo, &["rust"]);

    // Poorly formatted Rust — cargo-fmt is fixable.
    std::fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let src = repo.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    stage(
        &src.join("lib.rs"),
        "pub struct Foo { pub a: u32, pub b: u32 }\n",
        repo.path(),
    );

    let out = flint(&["--full", "--auto", "cargo-fmt"], repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    println!("=== stdout ===\n{stdout}");
    eprintln!("=== stderr ===\n{stderr}");

    // --auto fixes cargo-fmt but exits 1 — fixed files must be committed before pushing.
    assert!(
        !out.status.success(),
        "flint --auto should exit 1 when fixes were applied"
    );
    assert!(
        stderr.contains("fixed: cargo-fmt"),
        "expected 'fixed: cargo-fmt' in summary, got:\n{stderr}"
    );
    assert!(
        stderr.contains("commit before pushing"),
        "expected 'commit before pushing' hint, got:\n{stderr}"
    );
}

#[test]
fn auto_reports_unfixable_as_review() {
    let repo = git_repo();
    write_mise_toml(&repo, &["shellcheck"]);

    // SC2086: unquoted variable — shellcheck violation with no auto-fix.
    stage(
        &repo.path().join("bad.sh"),
        "#!/bin/bash\necho $1\n",
        repo.path(),
    );

    let out = flint(&["--full", "--auto", "shellcheck"], repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    println!("=== stdout ===\n{stdout}");
    eprintln!("=== stderr ===\n{stderr}");

    // --auto should exit 1 for non-fixable failures and surface them under review:.
    assert!(
        !out.status.success(),
        "flint --auto should exit 1 for unfixable checks"
    );
    // Linter output must appear inline so the caller doesn't need a second invocation.
    assert!(
        stderr.contains("[shellcheck]"),
        "expected inline [shellcheck] output, got:\n{stderr}"
    );
    assert!(
        stderr.contains("review: shellcheck"),
        "expected 'review: shellcheck' in summary, got:\n{stderr}"
    );
}

#[test]
fn shellcheck_clean_script_passes() {
    let repo = git_repo();
    write_mise_toml(&repo, &["shellcheck"]);

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

    assert!(out.status.success(), "flint should pass, got:\n{stderr}");
}
