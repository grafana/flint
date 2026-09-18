use clap_usage::usage::Spec;
use std::process::Command;

#[test]
fn usage_is_parseable_without_a_repository_or_valid_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("flint.toml"), "invalid toml [").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flint"))
        .arg("usage")
        .current_dir(dir.path())
        .env("FLINT_CONFIG_DIR", dir.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let spec: Spec = String::from_utf8(output.stdout).unwrap().parse().unwrap();
    assert_eq!(spec.name, "flint");
    assert_eq!(spec.bin, "flint");
    assert_eq!(spec.about.as_deref(), Some("flint — fast lint"));
    for name in [
        "run",
        "changed-files",
        "checker",
        "linters",
        "init",
        "hook",
        "version",
        "usage",
    ] {
        assert!(spec.cmd.subcommands.contains_key(name), "missing {name}");
    }
    let run = &spec.cmd.subcommands["run"];
    assert!(
        run.flags
            .iter()
            .any(|flag| flag.long.iter().any(|name| name == "fix"))
    );
    let rev = run
        .flags
        .iter()
        .find(|flag| flag.long.iter().any(|name| name == "new-from-rev"))
        .unwrap();
    assert!(rev.arg.is_some());
    assert!(
        spec.cmd.subcommands["hook"]
            .subcommands
            .contains_key("install")
    );
}

#[test]
fn usage_rejects_unrecognized_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_flint"))
        .args(["usage", "--not-a-flag"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
