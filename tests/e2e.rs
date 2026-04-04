use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Runs the flint binary in the given directory with the given args.
fn flint(args: &[&str], cwd: &Path) -> Output {
    flint_with_env(args, cwd, &[])
}

/// Runs the flint binary with additional environment variables.
fn flint_with_env(args: &[&str], cwd: &Path, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_flint"));
    cmd.args(args)
        .env("MISE_PROJECT_ROOT", cwd)
        .env_remove("FLINT_CONFIG_DIR")
        .current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("failed to spawn flint")
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

/// Runs all fixture cases under tests/cases/.
/// Each case is a directory containing:
///   files/     — files to copy into the repo and stage
///   test.toml  — args, expected exit code, and golden output
///
/// test.toml format:
///   args             = "--full --auto shellcheck"
///   exit             = 1                          # optional, default 0
///   expected_stderr  = """..."""                  # optional, default ""
///   expected_stdout  = """..."""                  # optional, default ""
///
///   [env]                                         # optional extra env vars
///   KEY = "value"
///
///   [fake_bins]                                   # optional fake binaries (Unix only)
///   renovate = "#!/bin/sh\necho '...'\n"          # written as executable, prepended to PATH
///
/// Set UPDATE_SNAPSHOTS=1 to regenerate golden output in test.toml.
#[test]
fn cases() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();

    let mut entries: Vec<_> = std::fs::read_dir(&cases_dir)
        .expect("tests/cases/ not found")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let case = entry.path();
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        run_case(&case, &name, update);
    }
}

fn run_case(case: &Path, name: &str, update: bool) {
    let toml_path = case.join("test.toml");
    let raw =
        std::fs::read_to_string(&toml_path).unwrap_or_else(|_| panic!("{name}: missing test.toml"));
    let cfg: toml::Value =
        toml::from_str(&raw).unwrap_or_else(|e| panic!("{name}: invalid test.toml: {e}"));

    let args_str = cfg["args"]
        .as_str()
        .unwrap_or_else(|| panic!("{name}: missing args"));
    let args: Vec<&str> = args_str.split_whitespace().collect();
    let expected_exit = cfg.get("exit").and_then(|v| v.as_integer()).unwrap_or(0) as i32;

    let repo = git_repo();

    let files_dir = case.join("files");
    copy_dir_into(&files_dir, repo.path());
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo.path())
        .output()
        .expect("git add failed");

    let env_vars: Vec<(String, String)> = cfg
        .get("env")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    // Write fake binaries into a temp dir and prepend it to PATH.
    // The tempdir must stay alive until after flint_with_env returns.
    let fake_bin_dir = tempfile::tempdir().expect("fake_bin tempdir");
    let fake_path = setup_fake_bins(&cfg, name, fake_bin_dir.path());

    let mut env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    if let Some(ref p) = fake_path {
        env_refs.push(("PATH", p.as_str()));
    }

    let out = flint_with_env(&args, repo.path(), &env_refs);

    let repo_str = repo.path().to_string_lossy();
    let stderr =
        strip_ansi(&String::from_utf8_lossy(&out.stderr).replace(repo_str.as_ref(), "<REPO>"));
    let stdout =
        strip_ansi(&String::from_utf8_lossy(&out.stdout).replace(repo_str.as_ref(), "<REPO>"));

    if update {
        write_test_toml(
            &toml_path,
            &cfg,
            out.status.code().unwrap_or(0) as i32,
            &stderr,
            &stdout,
        );
        println!("{name}: snapshots updated");
        return;
    }

    let exp_stderr = cfg
        .get("expected_stderr")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let exp_stdout = cfg
        .get("expected_stdout")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(stderr, exp_stderr, "{name}: stderr mismatch");
    assert_eq!(stdout, exp_stdout, "{name}: stdout mismatch");
    assert_eq!(
        out.status.code(),
        Some(expected_exit),
        "{name}: exit code mismatch"
    );

    // Assert file contents written by flint (e.g. fix mode snapshots).
    if let Some(files) = cfg.get("expected_files").and_then(|v| v.as_table()) {
        for (rel_path, expected) in files {
            let expected = expected
                .as_str()
                .unwrap_or_else(|| panic!("{name}: expected_files.{rel_path} must be a string"));
            let actual = std::fs::read_to_string(repo.path().join(rel_path))
                .unwrap_or_else(|e| panic!("{name}: expected_files.{rel_path}: {e}"));
            assert_eq!(actual, expected, "{name}: {rel_path} content mismatch");
        }
    }
}

/// Rewrites test.toml updating snapshot fields (exit, expected_stderr, expected_stdout)
/// while preserving everything else (args, env, fake_bins, expected_files).
///
/// Scalars (args, exit, expected_*) are written before table sections ([env],
/// [fake_bins], [expected_files]) to satisfy TOML's scoping rules.
fn write_test_toml(path: &Path, cfg: &toml::Value, exit: i32, stderr: &str, stdout: &str) {
    let args_str = cfg["args"].as_str().unwrap_or("");
    let mut out = format!("args = \"{}\"\n", args_str.replace('"', "\\\""));
    out += &format!("exit = {exit}\n");

    if !stderr.is_empty() {
        out += &format!("\nexpected_stderr = \"\"\"\n{stderr}\"\"\"");
    }
    if !stdout.is_empty() {
        out += &format!("\nexpected_stdout = \"\"\"\n{stdout}\"\"\"");
    }

    if let Some(env) = cfg.get("env").and_then(|v| v.as_table()) {
        if !env.is_empty() {
            out += "\n\n[env]\n";
            for (k, v) in env {
                if let Some(s) = v.as_str() {
                    out += &format!("{k} = \"{}\"\n", s.replace('"', "\\\""));
                }
            }
        }
    }

    // Serialize as multiline literal strings so shell scripts stay readable.
    // TOML trims the first newline after ''', so '''\n{s}''' roundtrips cleanly.
    if let Some(bins) = cfg.get("fake_bins").and_then(|v| v.as_table()) {
        if !bins.is_empty() {
            out += "\n[fake_bins]\n";
            for (k, v) in bins {
                if let Some(s) = v.as_str() {
                    out += &format!("{k} = '''\n{s}'''\n");
                }
            }
        }
    }

    // Preserve [expected_files] — not updated by UPDATE_SNAPSHOTS, managed manually.
    if let Some(files) = cfg.get("expected_files").and_then(|v| v.as_table()) {
        if !files.is_empty() {
            out += "\n[expected_files]\n";
            for (k, v) in files {
                if let Some(s) = v.as_str() {
                    out += &format!("\"{k}\" = \"\"\"\n{s}\"\"\"");
                }
            }
        }
    }

    std::fs::write(path, out).unwrap();
}

/// Strips ANSI escape sequences (e.g. colour codes from cargo fmt diffs).
/// TOML strings cannot contain raw control characters, so these must be removed.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── renovate-deps tests ──────────────────────────────────────────────────────
//
// These tests inject a fake `renovate` binary via PATH so they don't need the
// real tool installed. Unix-only because the fake is a shell script.

#[cfg(unix)]
mod renovate_deps {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // The JSON log line the fake renovate emits.
    const RENOVATE_LOG: &str = r#"{"msg":"packageFiles with updates","config":{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"},{"depName":"lodash"}]}]}}"#;

    // What write_snapshot produces for that log (serde_json::to_string_pretty + \n).
    const SNAPSHOT: &str = "{\n  \"package.json\": {\n    \"npm\": [\n      \"express\",\n      \"lodash\"\n    ]\n  }\n}\n";

    /// Creates a temp dir containing a fake `renovate` script that emits `log_line`.
    fn fake_renovate_bin(log_line: &str) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("renovate");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", log_line),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }

    /// Sets up a minimal repo with renovate declared in mise.toml.
    /// Optionally writes a snapshot file.
    fn setup_repo(snapshot: Option<&str>) -> TempDir {
        let repo = git_repo();
        std::fs::create_dir_all(repo.path().join(".github")).unwrap();
        std::fs::write(
            repo.path().join("mise.toml"),
            "[tools]\nrenovate = \"latest\"\n",
        )
        .unwrap();
        std::fs::write(repo.path().join(".github").join("renovate.json5"), "{}").unwrap();
        std::fs::write(repo.path().join("package.json"), "{}").unwrap();
        if let Some(snap) = snapshot {
            std::fs::write(
                repo.path()
                    .join(".github")
                    .join("renovate-tracked-deps.json"),
                snap,
            )
            .unwrap();
        }
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        repo
    }

    fn prepend_path(dir: &Path) -> String {
        let orig = std::env::var("PATH").unwrap_or_default();
        format!("{}:{orig}", dir.display())
    }

    #[test]
    fn up_to_date() {
        let bin = fake_renovate_bin(RENOVATE_LOG);
        let repo = setup_repo(Some(SNAPSHOT));
        let path = prepend_path(bin.path());
        let out = flint_with_env(
            &["--full", "renovate-deps"],
            repo.path(),
            &[("PATH", &path)],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn out_of_date() {
        let stale = "{\n  \"package.json\": {\n    \"npm\": [\n      \"old-dep\"\n    ]\n  }\n}\n";
        let bin = fake_renovate_bin(RENOVATE_LOG);
        let repo = setup_repo(Some(stale));
        let path = prepend_path(bin.path());
        let out = flint_with_env(
            &["--full", "renovate-deps"],
            repo.path(),
            &[("PATH", &path)],
        );
        assert_eq!(out.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("out of date"), "stderr: {stderr}");
        assert!(
            stderr.contains("old-dep"),
            "diff missing in stderr: {stderr}"
        );
    }

    #[test]
    fn fix_creates_snapshot() {
        let bin = fake_renovate_bin(RENOVATE_LOG);
        let repo = setup_repo(None);
        let path = prepend_path(bin.path());
        let out = flint_with_env(
            &["--full", "--fix", "renovate-deps"],
            repo.path(),
            &[("PATH", &path)],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let written = std::fs::read_to_string(
            repo.path()
                .join(".github")
                .join("renovate-tracked-deps.json"),
        )
        .unwrap();
        assert_eq!(written, SNAPSHOT);
    }

    #[test]
    fn fix_updates_stale_snapshot() {
        let stale = "{\n  \"package.json\": {\n    \"npm\": [\n      \"old-dep\"\n    ]\n  }\n}\n";
        let bin = fake_renovate_bin(RENOVATE_LOG);
        let repo = setup_repo(Some(stale));
        let path = prepend_path(bin.path());
        let out = flint_with_env(
            &["--full", "--fix", "renovate-deps"],
            repo.path(),
            &[("PATH", &path)],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let written = std::fs::read_to_string(
            repo.path()
                .join(".github")
                .join("renovate-tracked-deps.json"),
        )
        .unwrap();
        assert_eq!(written, SNAPSHOT);
    }
}

/// Writes fake binaries from `[fake_bins]` in the test config into `bin_dir`,
/// makes them executable (Unix), and returns a PATH string that prepends
/// `bin_dir` to the current PATH. Returns `None` when no fake_bins are declared.
/// On non-Unix platforms fake_bins are silently ignored.
fn setup_fake_bins(cfg: &toml::Value, case_name: &str, bin_dir: &Path) -> Option<String> {
    let table = cfg.get("fake_bins")?.as_table()?;
    if table.is_empty() {
        return None;
    }

    for (bin_name, script) in table {
        let content = script
            .as_str()
            .unwrap_or_else(|| panic!("{case_name}: fake_bins.{bin_name} must be a string"));
        let path = bin_dir.join(bin_name);
        std::fs::write(&path, content)
            .unwrap_or_else(|e| panic!("{case_name}: failed to write fake bin {bin_name}: {e}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|e| panic!("{case_name}: chmod failed for {bin_name}: {e}"));
        }
    }

    let orig = std::env::var("PATH").unwrap_or_default();
    Some(format!("{}:{orig}", bin_dir.display()))
}

fn copy_dir_into(src: &Path, dst: &Path) {
    for entry in std::fs::read_dir(src).expect("files/ dir not found") {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir_into(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}
