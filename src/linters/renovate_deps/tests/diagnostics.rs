use super::*;

#[test]
fn extract_failure_snippet_prefers_error_lines() {
    let log = "\
{\"level\":20,\"msg\":\"Parsing configs\"}\n\
{\"level\":30,\"msg\":\"Renovate started\"}\n\
{\"level\":50,\"msg\":\"Failed\",\"err\":{\"message\":\"boom\"}}\n\
{\"level\":20,\"msg\":\"trailing debug\"}\n";
    let snippet = extract_failure_snippet(log);
    assert_eq!(snippet, "level=50 Failed: boom");
}

#[test]
fn extract_failure_snippet_handles_missing_msg() {
    let log = "\
{\"level\":50,\"err\":{\"message\":\"boom\"}}\n\
{\"level\":60,\"msg\":\"\",\"err\":{\"message\":\"fatal\"}}\n\
{\"level\":40,\"msg\":\"warn only\"}\n";
    let snippet = extract_failure_snippet(log);
    assert_eq!(snippet, "level=60 fatal");
}

#[test]
fn extract_failure_snippet_omits_startup_warning_when_fatal_error_exists() {
    let log = "\
{\"level\":40,\"msg\":\"RE2 not usable, falling back to RegExp\"}\n\
{\"level\":60,\"msg\":\"Could not parse config file\"}\n";
    let snippet = extract_failure_snippet(log);
    assert_eq!(snippet, "level=60 Could not parse config file");
}

#[test]
fn extract_failure_snippet_falls_back_to_tail() {
    let mut log = String::new();
    for i in 0..30 {
        log.push_str(&format!("{{\"level\":20,\"msg\":\"line {i}\"}}\n"));
    }
    let snippet = extract_failure_snippet(&log);
    let lines: Vec<&str> = snippet.lines().collect();
    assert_eq!(lines.len(), 20);
    assert!(lines.last().unwrap().contains("line 29"));
    assert!(lines.first().unwrap().contains("line 10"));
}

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn sparse_test_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    run_git(dir.path(), &["init", "-q", "-b", "main"]);
    run_git(dir.path(), &["config", "user.email", "flint@example.com"]);
    run_git(dir.path(), &["config", "user.name", "flint"]);
    std::fs::create_dir(dir.path().join("included")).unwrap();
    std::fs::create_dir(dir.path().join("omitted")).unwrap();
    std::fs::write(dir.path().join("included/deps.txt"), "included\n").unwrap();
    std::fs::write(dir.path().join("omitted/deps.txt"), "omitted\n").unwrap();
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "-qm", "initial"]);
    dir
}

#[test]
fn detects_sparse_checkout_using_effective_git_config() {
    let repo = sparse_test_repo();
    assert!(!sparse_checkout_detected(repo.path()));

    run_git(repo.path(), &["sparse-checkout", "set", "included"]);
    assert!(sparse_checkout_detected(repo.path()));

    run_git(repo.path(), &["sparse-checkout", "disable"]);
    assert!(!sparse_checkout_detected(repo.path()));
}

#[test]
fn detects_sparse_checkout_in_linked_worktree_git_dir() {
    let repo = sparse_test_repo();
    let linked = repo.path().join("linked");
    run_git(
        repo.path(),
        &["worktree", "add", "-q", linked.to_str().unwrap()],
    );
    assert!(!sparse_checkout_detected(&linked));

    run_git(&linked, &["sparse-checkout", "set", "included"]);
    assert!(sparse_checkout_detected(&linked));
}

#[test]
fn mismatch_guard_rejects_omitted_snapshot_path_marked_skip_worktree() {
    let repo = sparse_test_repo();
    run_git(repo.path(), &["sparse-checkout", "set", "included"]);
    let committed = snapshot(
        &[("omitted", None, None)],
        &[("omitted/deps.txt", &[("npm", &["omitted"])])],
    );
    let generated = Snapshot::default();

    let error = ensure_snapshot_not_truncated(repo.path(), Some(&committed), &generated)
        .expect_err("sparse snapshot should be rejected");
    assert!(error.to_string().contains("git sparse-checkout disable"));
}

#[test]
fn mismatch_guard_does_not_reject_normal_missing_snapshot_path() {
    let repo = sparse_test_repo();
    let committed = snapshot(
        &[("omitted", None, None)],
        &[("omitted/deps.txt", &[("npm", &["omitted"])])],
    );
    let generated = Snapshot::default();

    ensure_snapshot_not_truncated(repo.path(), Some(&committed), &generated)
        .expect("normal tracked files are not sparse paths");
}
