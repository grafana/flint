use super::*;

#[test]
fn resolves_supported_renovate_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".renovaterc.json");
    std::fs::write(&config_path, "{}\n").unwrap();

    let resolved = resolve_renovate_config_path(dir.path()).unwrap();

    assert_eq!(resolved, config_path);
}

#[test]
fn missing_supported_renovate_config_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();

    let err = resolve_renovate_config_path(dir.path()).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("no supported Renovate config file found"));
    assert!(
        RENOVATE_CONFIG_PATTERNS
            .iter()
            .all(|path| msg.contains(path))
    );
}

#[test]
fn committed_path_uses_same_dir_as_found_config() {
    assert_eq!(
        committed_path_for_config(Path::new("renovate.json5")),
        PathBuf::from("renovate-tracked-deps.json")
    );
    assert_eq!(
        committed_path_for_config(Path::new(".github/renovate.json5")),
        PathBuf::from(".github/renovate-tracked-deps.json")
    );
}

fn file_list(paths: &[&str], full: bool) -> FileList {
    FileList {
        files: paths.iter().map(PathBuf::from).collect(),
        changed_paths: paths.iter().map(|path| path.to_string()).collect(),
        merge_base: Some("base".to_string()),
        full,
    }
}

#[test]
fn relevant_when_full_mode() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_relevant(&file_list(&[], true), dir.path()));
}

#[test]
fn relevant_when_renovate_config_changed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_relevant(
        &file_list(
            &[dir.path().join(".github/renovate.json5").to_str().unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn relevant_when_snapshot_changed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate-tracked-deps.json"),
        "{}\n",
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/renovate-tracked-deps.json")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn relevant_when_tracked_manifest_changed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(&[dir.path().join("package.json").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn relevant_when_tracked_manifest_was_deleted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    let file_list = FileList {
        files: vec![],
        changed_paths: vec!["package.json".to_string()],
        merge_base: Some("base".to_string()),
        full: false,
    };

    assert!(is_relevant(&file_list, dir.path()));
}

#[test]
fn not_relevant_for_untracked_change() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn relevant_when_new_file_matches_inline_custom_manager_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{
          customManagers: [
            {
              customType: "regex",
              managerFilePatterns: ["/(^|/)\\.github/workflows/.+\\.ya?ml$/"],
              matchStrings: ["version: (?<currentValue>.+)"],
              datasourceTemplate: "github-releases",
              depNameTemplate: "example",
            },
          ],
        }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    // A brand-new workflow file, not yet present in the committed snapshot.
    assert!(is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/workflows/new.yml")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn relevant_when_extends_is_a_single_string() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::create_dir_all(dir.path().join("shared")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{ extends: "local>shared/renovate" }"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("shared/renovate.json5"),
        r#"{ customManagers: [{ managerFilePatterns: ["**/*.yml"] }] }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/workflows/new.yml")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn not_relevant_when_bundled_flint_preset_has_no_custom_manager_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{ extends: ["config:recommended", "github>grafana/flint#v1.2.3"] }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/workflows/new.yml")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn not_relevant_when_extend_escapes_project_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{ extends: ["../shared/renovate"] }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("shared")).unwrap();
    std::fs::write(
        dir.path().join("shared/renovate.json5"),
        r#"{ customManagers: [{ managerFilePatterns: ["**/*.yml"] }] }"#,
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/workflows/new.yml")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn not_relevant_when_extend_uses_named_preset_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{ extends: ["local>preset:unsafe"] }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/workflows/new.yml")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn not_relevant_when_extend_is_unresolvable_remote_preset() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate.json5"),
        r#"{ extends: ["github>some-org/some-repo"] }"#,
    )
    .unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn relevant_when_snapshot_is_unparsable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate-tracked-deps.json"),
        "{not json}\n",
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
        dir.path()
    ));
}
