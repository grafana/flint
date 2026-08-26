use super::*;

#[test]
fn write_and_read_snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    let deps = snapshot(
        &[
            ("serde", None, None),
            ("tokio", None, None),
            ("express", None, None),
            ("lodash", None, None),
        ],
        &[
            ("Cargo.toml", &[("cargo", &["serde", "tokio"])]),
            ("package.json", &[("npm", &["express", "lodash"])]),
        ],
    );
    write_snapshot(&path, &deps).unwrap();
    let read_back = read_snapshot(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(deps, read_back);
}

#[test]
fn reads_legacy_snapshot_format() {
    let legacy = r#"{
  "package.json": {
    "npm": [
      "express"
    ]
  }
}
"#;
    let snapshot = read_snapshot(legacy).unwrap();
    assert!(snapshot.meta.is_empty());
    assert_eq!(
        snapshot.files,
        dep_files(&[("package.json", &[("npm", &["express"])])])
    );
}

#[test]
fn read_snapshot_normalizes_dep_order() {
    let current = r#"{
  "meta": {},
  "files": {
    "package.json": {
      "npm": [
        "zebra",
        "alpha",
        "moose"
      ]
    }
  }
}
"#;
    let snapshot = read_snapshot(current).unwrap();
    assert_eq!(
        snapshot.files,
        dep_files(&[("package.json", &[("npm", &["alpha", "moose", "zebra"])])])
    );
}

#[test]
fn write_snapshot_ends_with_newline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    write_snapshot(&path, &Snapshot::default()).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.ends_with('\n'));
}

#[test]
fn write_snapshot_serializes_canonical_dep_order() {
    use std::collections::BTreeMap;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    let deps = Snapshot {
        meta: BTreeMap::new(),
        action_meta: BTreeMap::new(),
        files: dep_files(&[("package.json", &[("npm", &["zebra", "alpha", "moose"])])]),
    };

    write_snapshot(&path, &deps).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    let written: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(
        written["files"]["package.json"]["npm"],
        serde_json::json!(["alpha", "moose", "zebra"])
    );
}

#[test]
fn merge_missing_meta_from_committed_keeps_existing_details() {
    let mut generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let committed = snapshot(
        &[(
            "actionlint",
            Some("rhysd/actionlint"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );

    merge_missing_meta_from_committed(&mut generated, &committed);

    assert_eq!(
        generated.meta["actionlint"].package_name.as_deref(),
        Some("rhysd/actionlint")
    );
    assert_eq!(
        generated.meta["actionlint"].datasource.as_deref(),
        Some("github-releases")
    );
}

#[test]
fn maybe_reuse_committed_meta_merges_missing_fields() {
    let mut generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let committed = snapshot(
        &[(
            "actionlint",
            Some("rhysd/actionlint"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );

    maybe_reuse_committed_meta(&mut generated, Some(&committed));

    assert_eq!(
        generated.meta["actionlint"].package_name.as_deref(),
        Some("rhysd/actionlint")
    );
}
