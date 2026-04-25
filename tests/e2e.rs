use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

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

/// Creates a temp directory initialised as a git repo with branch `main`.
fn git_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@test.com"],
        vec!["config", "user.name", "Test"],
    ] {
        let out = Command::new("git")
            .args(&args)
            .current_dir(dir.path())
            .output()
            .expect("failed to spawn git");
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    dir
}

/// Runs all fixture cases under tests/cases/.
/// Each case is a directory containing:
///   files/     — files to copy into the repo and stage
///   test.toml  — args, expected exit code, and golden output
///
/// test.toml format:
///   [expected]
///   args   = "--full --fix shellcheck"
///   exit   = 1                          # optional, default 0
///   stderr = """..."""                  # optional, default ""
///   stdout = """..."""                  # optional, default ""
///   stderr_contains = ["..."]           # optional substring assertions
///   stdout_contains = ["..."]           # optional substring assertions
///
///   [expected.files]                    # optional file contents asserted after run
///   ".github/renovate-tracked-deps.json" = """..."""
///
///   [env]                               # optional extra env vars
///   KEY = "value"
///
///   [fake_bins]                         # optional fake binaries (Unix only)
///   renovate = '''
///   #!/bin/sh
///   echo '...'
///   '''
///
/// Set UPDATE_SNAPSHOTS=1 to regenerate golden output in test.toml.
/// Set FLINT_CASES=<dir> to run only cases under that directory (e.g. FLINT_CASES=shellcheck
/// or FLINT_CASES=shellcheck/clean). Top-level groups run in parallel.
///
/// Cases that declare `[fake_bins]` are skipped on non-Unix platforms because the
/// fake binaries are shell scripts. All other cases run on every platform.
#[test]
fn cases() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();
    let filter = std::env::var("FLINT_CASES").ok();

    let mut case_paths = collect_cases(&cases_dir);
    case_paths.sort();

    if let Some(ref f) = filter {
        case_paths.retain(|p| {
            let name = p.strip_prefix(&cases_dir).unwrap().to_string_lossy();
            name.starts_with(f.as_str())
        });
        if case_paths.is_empty() {
            panic!("FLINT_CASES={f}: no matching cases found");
        }
    }

    // Group by top-level directory (linter name) so each group runs in its own thread.
    let mut groups: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for path in case_paths {
        let top = path
            .strip_prefix(&cases_dir)
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        groups.entry(top).or_default().push(path);
    }

    let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = groups
        .into_values()
        .map(|paths| {
            let cases_dir = cases_dir.clone();
            let failures = Arc::clone(&failures);
            std::thread::spawn(move || {
                for case in &paths {
                    let name = case
                        .strip_prefix(&cases_dir)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_case(case, &name, update);
                    }));
                    if let Err(e) = result {
                        let msg = e
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| format!("panic in {name}"));
                        failures.lock().unwrap().push(format!(
                            "FAILED: {name}\n{msg}\n  → rerun: FLINT_CASES={name} cargo test cases"
                        ));
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let failures = failures.lock().unwrap();
    if !failures.is_empty() {
        panic!(
            "\n\n{}\n\n{} case(s) failed",
            failures.join("\n\n"),
            failures.len()
        );
    }
}

#[cfg(unix)]
#[test]
fn markdown_tool_ignores_biome_owned_jsonc() {
    let repo = git_repo();

    std::fs::write(
        repo.path().join("mise.toml"),
        r#"[tools]
rumdl = "0.1.78"
biome = "2.4.12"
"#,
    )
    .unwrap();
    std::fs::write(repo.path().join("README.md"), "# Test\n").unwrap();
    std::fs::write(
        repo.path().join("biome.jsonc"),
        r#"{
  // Keep JSON formatting aligned with the repo's two-space style.
  "formatter": {
    "indentStyle": "space",
    "indentWidth": 2,
  },
}
"#,
    )
    .unwrap();

    let out = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git add");
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git commit");
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let fake_bin_dir = tempfile::tempdir().expect("fake_bin tempdir");
    let rumdl = fake_bin_dir.path().join("rumdl");
    std::fs::write(
        &rumdl,
        r#"#!/bin/sh
set -eu

cmd="$1"
shift

if [ "$cmd" != "check" ]; then
  echo "unsupported rumdl invocation: $cmd $*" >&2
  exit 1
fi

for target in "$@"; do
  case "$target" in
    -*)
      continue
      ;;
  esac

  base="$(basename "$target")"
  if [ "$base" = "README.md" ]; then
    continue
  fi

  echo "rumdl unexpectedly targeted: $target" >&2
  exit 1
done

exit 0
"#,
    )
    .unwrap();

    let biome = fake_bin_dir.path().join("biome");
    std::fs::write(
        &biome,
        r#"#!/bin/sh
set -eu

config_dir=""
if [ "${1:-}" = "--config-path" ]; then
  config_dir="$2"
  shift 2
fi

cmd="$1"
shift

if [ -n "$config_dir" ] && [ ! -f "$config_dir/biome.jsonc" ]; then
  echo "missing biome config in $config_dir" >&2
  exit 1
fi

if [ "$cmd" = "format" ] && [ "${1:-}" = "--write" ]; then
  file="$2"
  cat >"$file" <<'EOF'
{
  // Keep JSON formatting aligned with the repo's two-space style.
  "formatter": {
    "indentStyle": "space",
    "indentWidth": 2
  }
}
EOF
  exit 0
fi

if [ "$cmd" = "format" ]; then
  file="$1"
  if grep -q '"indentWidth": 2$' "$file" && grep -q '^  }$' "$file"; then
    exit 0
  fi
  echo "formatting differs" >&2
  exit 1
fi

echo "unsupported biome invocation: $cmd $*" >&2
exit 1
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&rumdl, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&biome, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let fake_path = format!(
        "{}:{}",
        fake_bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let fix_out = flint_with_env(
        &["run", "--full", "--fix", "rumdl", "biome-format"],
        repo.path(),
        &[("PATH", &fake_path)],
    );
    assert_eq!(fix_out.status.code(), Some(1));
    let fix_stderr = String::from_utf8_lossy(&fix_out.stderr);
    assert!(
        fix_stderr.contains("flint: fixed: biome-format — commit before pushing"),
        "unexpected fix stderr:\n{fix_stderr}"
    );

    let biome_jsonc = std::fs::read_to_string(repo.path().join("biome.jsonc")).unwrap();
    assert!(
        biome_jsonc.contains("\"indentWidth\": 2\n")
            && !biome_jsonc.contains("\"indentWidth\": 2,\n"),
        "expected biome-owned formatting after fix:\n{biome_jsonc}"
    );

    let check_out = flint_with_env(
        &["run", "--full", "rumdl"],
        repo.path(),
        &[("PATH", &fake_path)],
    );
    assert_eq!(
        check_out.status.code(),
        Some(0),
        "rumdl should ignore biome-owned JSONC in full mode:\n{}",
        String::from_utf8_lossy(&check_out.stderr)
    );
}

#[cfg(unix)]
#[test]
fn rumdl_fix_hides_success_noise_when_another_file_fails() {
    let repo = git_repo();

    std::fs::write(
        repo.path().join("mise.toml"),
        r#"[tools]
rumdl = "0.1.78"
"#,
    )
    .unwrap();
    std::fs::write(repo.path().join("clean.md"), "# Clean\n").unwrap();
    std::fs::write(repo.path().join("failing.md"), "# Failing\n").unwrap();

    let out = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git add");
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git commit");
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let fake_bin_dir = tempfile::tempdir().expect("fake_bin tempdir");
    let rumdl = fake_bin_dir.path().join("rumdl");
    std::fs::write(
        &rumdl,
        r#"#!/bin/sh
set -eu

cmd="$1"
shift

if [ "$cmd" != "check" ]; then
  echo "unsupported rumdl invocation: $cmd $*" >&2
  exit 1
fi

for arg in "$@"; do
  case "$arg" in
    -*)
      continue
      ;;
    *clean.md)
      echo "Success: No issues found in 1 file (8ms)"
      ;;
    *failing.md)
      echo "failing.md:1:121: [MD013] Line length 140 exceeds 120 characters"
      exit 1
      ;;
    *)
      echo "unexpected rumdl target: $arg" >&2
      exit 1
      ;;
  esac
done

exit 0
"#,
    )
    .unwrap();

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&rumdl, std::fs::Permissions::from_mode(0o755)).unwrap();

    let fake_path = format!(
        "{}:{}",
        fake_bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = flint_with_env(
        &["run", "--full", "--fix", "rumdl"],
        repo.path(),
        &[("PATH", &fake_path)],
    );
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("failing.md:1:121: [MD013] Line length 140 exceeds 120 characters"),
        "unexpected fix stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Success: No issues found in 1 file (8ms)"),
        "unexpected rumdl success noise in fix stderr:\n{stderr}"
    );
}

/// Recursively finds all directories containing a `test.toml` file.
fn collect_cases(dir: &Path) -> Vec<PathBuf> {
    let mut cases = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return cases;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if path.join("test.toml").exists() {
                cases.push(path);
            } else {
                cases.extend(collect_cases(&path));
            }
        }
    }
    cases
}

fn run_case(case: &Path, name: &str, update: bool) {
    let toml_path = case.join("test.toml");
    let raw =
        std::fs::read_to_string(&toml_path).unwrap_or_else(|_| panic!("{name}: missing test.toml"));
    let cfg: toml::Value =
        toml::from_str(&raw).unwrap_or_else(|e| panic!("{name}: invalid test.toml: {e}"));

    let expected = cfg
        .get("expected")
        .unwrap_or_else(|| panic!("{name}: missing [expected] table"));
    let args_str = expected["args"]
        .as_str()
        .unwrap_or_else(|| panic!("{name}: missing expected.args"));
    let args: Vec<&str> = args_str.split_whitespace().collect();
    let expected_exit = expected
        .get("exit")
        .and_then(|v| v.as_integer())
        .unwrap_or(0) as i32;

    // Skip cases that use shell-script fake binaries on non-Unix platforms.
    #[cfg(not(unix))]
    if cfg
        .get("fake_bins")
        .and_then(|v| v.as_table())
        .is_some_and(|t| !t.is_empty())
    {
        eprintln!("{name}: skipped (fake_bins requires Unix)");
        return;
    }

    let repo = git_repo();

    let files_dir = case.join("files");
    copy_dir_into(&files_dir, repo.path());
    let out = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git add");
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(repo.path())
        .output()
        .expect("failed to spawn git commit");
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // If a `changes/` directory exists alongside `files/`, write those files
    // over the repo and stage them (but don't commit). This lets fixtures test
    // the changed-files code path (as opposed to --full / all-files mode).
    let changes_dir = case.join("changes");
    if changes_dir.exists() {
        copy_dir_into(&changes_dir, repo.path());
        let out = Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo.path())
            .output()
            .expect("failed to spawn git add");
        assert!(
            out.status.success(),
            "git add changes failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

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
    let runtime_dir = tempfile::tempdir().expect("runtime tempdir");
    let cache_dir = runtime_dir.path().join(".cache");
    let tmp_dir = runtime_dir.path().join(".tmp");
    let home_dir = runtime_dir.path().join(".home");
    std::fs::create_dir_all(cache_dir.join("go-build")).expect("create go cache dir");
    std::fs::create_dir_all(cache_dir.join("golangci-lint"))
        .expect("create golangci-lint cache dir");
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
    std::fs::create_dir_all(&home_dir).expect("create home dir");

    let mut env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut injected_env = vec![
        ("HOME".to_string(), home_dir.to_string_lossy().into_owned()),
        (
            "XDG_CACHE_HOME".to_string(),
            cache_dir.to_string_lossy().into_owned(),
        ),
        (
            "GOCACHE".to_string(),
            cache_dir.join("go-build").to_string_lossy().into_owned(),
        ),
        (
            "GOLANGCI_LINT_CACHE".to_string(),
            cache_dir
                .join("golangci-lint")
                .to_string_lossy()
                .into_owned(),
        ),
        ("TMPDIR".to_string(), tmp_dir.to_string_lossy().into_owned()),
    ];
    if !env_vars.iter().any(|(k, _)| k == "DOTNET_CLI_HOME") {
        injected_env.push((
            "DOTNET_CLI_HOME".to_string(),
            home_dir.join(".dotnet").to_string_lossy().into_owned(),
        ));
    }
    env_refs.extend(injected_env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    if let Some(ref p) = fake_path {
        env_refs.push(("PATH", p.as_str()));
    }

    let out = flint_with_env(&args, repo.path(), &env_refs);

    let repo_str = repo.path().to_string_lossy();
    let repo_canonical_str = canonical_repo_path(repo.path());
    let normalize =
        |s: String| -> String { normalize_output(s, repo_str.as_ref(), &repo_canonical_str) };
    let stderr =
        normalize_rust_compile_summaries(&normalize_tool_versions(&normalize_timing(&strip_ansi(
            &normalize(String::from_utf8_lossy(&out.stderr).into_owned()),
        ))));
    let stdout =
        normalize_rust_compile_summaries(&normalize_tool_versions(&normalize_timing(&strip_ansi(
            &normalize(String::from_utf8_lossy(&out.stdout).into_owned()),
        ))));

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

    let exp_stderr = expected
        .get("stderr")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let exp_stdout = expected
        .get("stdout")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if let Some(contains) = expected.get("stderr_contains").and_then(|v| v.as_array()) {
        for needle in contains {
            let needle = needle.as_str().unwrap_or_else(|| {
                panic!("{name}: expected.stderr_contains entries must be strings")
            });
            assert!(
                stderr.contains(needle),
                "{name}: stderr missing substring:\n{needle}\n\nactual stderr:\n{stderr}"
            );
        }
    } else {
        assert_eq!(stderr, exp_stderr, "{name}: stderr mismatch");
    }
    if let Some(contains) = expected.get("stdout_contains").and_then(|v| v.as_array()) {
        for needle in contains {
            let needle = needle.as_str().unwrap_or_else(|| {
                panic!("{name}: expected.stdout_contains entries must be strings")
            });
            assert!(
                stdout.contains(needle),
                "{name}: stdout missing substring:\n{needle}\n\nactual stdout:\n{stdout}"
            );
        }
    } else {
        assert_eq!(stdout, exp_stdout, "{name}: stdout mismatch");
    }
    assert_eq!(
        out.status.code(),
        Some(expected_exit),
        "{name}: exit code mismatch"
    );

    // Assert file contents written by flint (e.g. fix mode snapshots).
    if let Some(files) = expected.get("files").and_then(|v| v.as_table()) {
        for (rel_path, exp) in files {
            let exp = exp
                .as_str()
                .unwrap_or_else(|| panic!("{name}: expected.files.{rel_path} must be a string"));
            let actual = std::fs::read_to_string(repo.path().join(rel_path))
                .unwrap_or_else(|e| panic!("{name}: expected.files.{rel_path}: {e}"));
            assert_eq!(actual, exp, "{name}: {rel_path} content mismatch");
        }
    }
}

/// Rewrites test.toml updating snapshot fields ([expected].exit/stderr/stdout)
/// while preserving everything else (args, env, fake_bins, expected.files).
fn write_test_toml(path: &Path, cfg: &toml::Value, exit: i32, stderr: &str, stdout: &str) {
    let expected = &cfg["expected"];
    let args_str = expected["args"].as_str().unwrap_or("");
    let existing_files = expected.get("files").and_then(|v| v.as_table());
    let existing_stderr_contains = expected.get("stderr_contains").and_then(|v| v.as_array());
    let existing_stdout_contains = expected.get("stdout_contains").and_then(|v| v.as_array());

    let mut out = String::from("[expected]\n");
    out += &format!("args = \"{}\"\n", toml_escape(args_str));
    out += &format!("exit = {exit}\n");
    if let Some(contains) = existing_stderr_contains {
        out += &format!("stderr_contains = {}\n", toml_string_array(contains));
    } else if !stderr.is_empty() {
        out += &format!("stderr = '''\n{stderr}'''\n");
    }
    if let Some(contains) = existing_stdout_contains {
        out += &format!("stdout_contains = {}\n", toml_string_array(contains));
    } else if !stdout.is_empty() {
        out += &format!("stdout = '''\n{stdout}'''\n");
    }
    if let Some(files) = existing_files {
        out += "\n[expected.files]\n";
        for (k, v) in files {
            if let Some(s) = v.as_str() {
                // Literal multi-line strings ('''…''') to avoid escape processing.
                out += &format!("\"{k}\" = '''\n{s}'''\n");
            }
        }
    }

    if let Some(env) = cfg.get("env").and_then(|v| v.as_table())
        && !env.is_empty()
    {
        out += "\n\n[env]\n";
        for (k, v) in env {
            if let Some(s) = v.as_str() {
                out += &format!("{k} = \"{}\"\n", toml_escape(s));
            }
        }
    }

    // Serialize as multiline literal strings so shell scripts stay readable.
    // TOML trims the first newline after ''', so '''\n{s}''' roundtrips cleanly.
    if let Some(bins) = cfg.get("fake_bins").and_then(|v| v.as_table())
        && !bins.is_empty()
    {
        out += "\n[fake_bins]\n";
        for (k, v) in bins {
            if let Some(s) = v.as_str() {
                out += &format!("{k} = '''\n{s}'''\n");
            }
        }
    }

    std::fs::write(path, out).unwrap();
}

/// Escapes a string for use inside TOML basic double-quoted strings.
fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_string_array(values: &[toml::Value]) -> String {
    let values = values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .expect("expected string entries in TOML string array");
            format!("\"{}\"", toml_escape(value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

#[test]
fn write_test_toml_preserves_contains_assertions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.toml");
    let src = r#"[expected]
args = "run --full --verbose taplo"
exit = 1
stderr_contains = ["keep stderr"]
stdout_contains = ["keep stdout"]
"#;
    std::fs::write(&path, src).expect("write test.toml");

    let cfg: toml::Value = toml::from_str(src).expect("parse test.toml");
    write_test_toml(&path, &cfg, 0, "new stderr", "new stdout");

    let updated = std::fs::read_to_string(&path).expect("read updated test.toml");
    assert!(updated.contains("exit = 0"));
    assert!(updated.contains("stderr_contains = [\"keep stderr\"]"));
    assert!(updated.contains("stdout_contains = [\"keep stdout\"]"));
    assert!(!updated.contains("new stderr"));
    assert!(!updated.contains("new stdout"));
}

/// Normalises timing suffixes on check header lines so snapshots are stable.
/// `[name] 123ms` and `[name] 1.2s` both become `[name] Xms`.
fn normalize_timing(s: &str) -> String {
    use regex::Regex;
    // Flint check header lines: "[name] 123ms" or "[name] 1.2s"
    let re = Regex::new(r"(?m)^(\[[^\]]+\]) \d+(?:\.\d+)?(?:ms|s)$").unwrap();
    let s = re.replace_all(s, "$1 Xms");
    // Biome summary line: "Checked N file(s) in 1234µs. No fixes applied."
    let re2 = Regex::new(r"Checked \d+ files? in \d+(?:\.\d+)?(?:µs|ms|s)\.").unwrap();
    re2.replace_all(&s, "Checked N file(s) in Xµs.")
        .into_owned()
}

/// Replaces tool version banners with a stable `<VERSION>` placeholder so
/// snapshots don't need updating on every dependency bump.
fn normalize_tool_versions(s: &str) -> String {
    use regex::Regex;
    // flint X.Y.Z (version command output)
    let re = Regex::new(r"flint \d+\.\d+\.\d+").unwrap();
    let s = re.replace_all(s, "flint <VERSION>").into_owned();
    let old_markdownlint_banner =
        Regex::new(r"markdownlint-cli2 v\d+\.\d+\.\d+ \(markdownlint v\d+\.\d+\.\d+\)").unwrap();
    assert!(
        !old_markdownlint_banner.is_match(&s),
        "found stale markdownlint-era snapshot output; update the fixture instead of normalizing it"
    );
    let re = Regex::new(r"https://rust-lang\.github\.io/rust-clippy/rust-\d+\.\d+\.\d+/").unwrap();
    re.replace_all(
        &s,
        "https://rust-lang.github.io/rust-clippy/rust-<VERSION>/",
    )
    .into_owned()
}

/// Cargo may emit multiple "could not compile" summary lines in either order
/// when both lib and lib test targets fail. Sort only that contiguous block so
/// snapshots remain stable while preserving the surrounding output verbatim.
fn normalize_rust_compile_summaries(s: &str) -> String {
    let is_summary = |line: &str| {
        line.starts_with("error: could not compile `")
            && (line.ends_with(" previous error") || line.ends_with(" previous errors"))
    };
    let summary_rank = |line: &str| {
        if line.contains("(lib) ") {
            0_u8
        } else if line.contains("(lib test) ") {
            1_u8
        } else {
            2_u8
        }
    };

    let mut out = Vec::new();
    let mut block = Vec::new();

    for line in s.split_inclusive('\n') {
        if is_summary(line.trim_end_matches('\n')) {
            block.push(line);
            continue;
        }

        if !block.is_empty() {
            block.sort_unstable_by(|a, b| {
                summary_rank(a).cmp(&summary_rank(b)).then_with(|| a.cmp(b))
            });
            out.append(&mut block);
        }
        out.push(line);
    }

    if !block.is_empty() {
        block.sort_unstable_by(|a, b| summary_rank(a).cmp(&summary_rank(b)).then_with(|| a.cmp(b)));
        out.append(&mut block);
    }

    out.concat()
}

/// Strips ANSI/VT escape sequences (colour codes, character-set switches, etc.).
/// TOML strings cannot contain raw control characters, so these must be removed.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                // CSI sequence: ESC [ <params> <letter>
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            Some(next) if ('\x20'..='\x2f').contains(&next) => {
                // Two-byte sequence with intermediate: ESC <intermediate> <final>
                // e.g. ESC(B (select ASCII character set)
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('\x30'..='\x7e').contains(&next) {
                        break;
                    }
                }
            }
            _ => {} // bare ESC — drop it
        }
    }
    out
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

/// Returns the canonical form of the repo path with platform quirks stripped.
/// On macOS this resolves /private/... symlinks. On Windows it strips the \\?\
/// verbatim prefix so the result matches what tools actually emit.
fn canonical_repo_path(path: &std::path::Path) -> String {
    // dunce::canonicalize resolves symlinks (/private/... on macOS) and strips
    // the \\?\ verbatim prefix on Windows that tools don't emit.
    dunce::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Normalises tool output for snapshot comparison:
/// - CRLF → LF
/// - Windows path separators → forward slashes (both output and repo paths)
/// - Strip residual //?/ UNC prefix (after \ normalisation)
/// - Replace canonical and non-canonical repo path forms with <REPO>
/// - Collapse file:///<REPO> → file://<REPO> (lychee Windows URI form)
fn normalize_output(s: String, repo_str: &str, repo_canonical: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");

    #[cfg(windows)]
    let (s, canonical_cmp, repo_cmp) = {
        // Strip the Windows verbatim UNC prefix \\?\ before substitution.
        // Also strip //?/ which appears if the UNC prefix got mixed with forward slashes.
        let s = s.replace(r"\\?\", "").replace("//?/", "");
        // Substitute using both backslash and forward-slash forms so paths from
        // different tools (shfmt uses \, lychee uses /) are all collapsed.
        (s, repo_canonical.to_string(), repo_str.to_string())
    };
    #[cfg(not(windows))]
    let (s, canonical_cmp, repo_cmp) = (s, repo_canonical.to_string(), repo_str.to_string());

    // Substitute using both the canonical form (e.g. long name on Windows, /private/... on
    // macOS) and the raw form, in both backslash and forward-slash variants.
    let sub = |s: String, pat: &str| -> String {
        if pat.is_empty() {
            return s;
        }
        #[cfg(windows)]
        let s = s.replace(&pat.replace('\\', "/"), "<REPO>");
        s.replace(pat, "<REPO>")
    };
    let s = if canonical_cmp != repo_cmp {
        sub(s, &canonical_cmp)
    } else {
        s
    };
    let s = sub(s, &repo_cmp);

    // On Windows, normalize backslash path separators to forward slashes.
    // Skip content inside single quotes to preserve tool-specific notations
    // like dotnet's whitespace descriptions: Insert '\s\s\s\s'.
    #[cfg(windows)]
    let s = {
        let mut out = String::with_capacity(s.len());
        let mut in_single_quote = false;
        for ch in s.chars() {
            match ch {
                '\'' => {
                    in_single_quote = !in_single_quote;
                    out.push(ch);
                }
                '\\' if !in_single_quote => out.push('/'),
                other => out.push(other),
            }
        }
        out.replace("file:///<REPO>", "file://<REPO>")
    };
    s
}
