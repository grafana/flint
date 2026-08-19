use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

const PACKAGE_FILES_MSGS: &[&str] = &["Extracted dependencies", "packageFiles with updates"];
// `invalid-version` is intentionally NOT filtered: it is set at lookup time when
// Renovate's versioning rejects the resolved `currentVersion` (e.g. mise.lock
// forwarding `temurin-*` as currentVersion for java-jdk). The dep is still
// declared in the config and must remain tracked; filtering it here caused
// `--fix` (lookup) to silently drop deps that verify (extract) keeps.
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value"];

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
pub(crate) type DepFiles = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Snapshot {
    pub(crate) meta: BTreeMap<String, DepMeta>,
    pub(crate) files: DepFiles,
    /// Stable identity and ref-shape metadata for reusable GitHub Actions.
    ///
    /// This is deliberately separate from `meta`: Renovate uses the same
    /// package name for every path in a monorepo, while each action path may
    /// have its own tag namespace (or be a branch-pinned workflow).
    #[serde(rename = "actionMeta")]
    pub(crate) action_meta: BTreeMap<String, ActionMeta>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ActionMeta {
    #[serde(rename = "packageName")]
    pub(crate) package_name: String,
    #[serde(rename = "refKind")]
    pub(crate) ref_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) compatibility: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub(crate) ref_: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct DepMeta {
    #[serde(rename = "packageName", skip_serializing_if = "Option::is_none")]
    pub(crate) package_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) datasource: Option<String>,
    #[serde(rename = "currentValue", skip_serializing_if = "Option::is_none")]
    pub(crate) current_value: Option<String>,
    #[serde(rename = "currentVersion", skip_serializing_if = "Option::is_none")]
    pub(crate) current_version: Option<String>,
    #[serde(rename = "extractVersion", skip_serializing_if = "Option::is_none")]
    pub(crate) extract_version: Option<String>,
}

impl DepMeta {
    pub(crate) fn version_context(&self) -> Option<(&str, &str, &str)> {
        Some((
            self.current_value.as_deref()?,
            self.current_version.as_deref()?,
            self.extract_version.as_deref()?,
        ))
    }

    pub(crate) fn clear_version_context(&mut self) {
        self.current_value = None;
        self.current_version = None;
        self.extract_version = None;
    }
}

impl Snapshot {
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub(crate) fn normalize(&mut self) {
        for managers in self.files.values_mut() {
            for deps in managers.values_mut() {
                deps.sort();
            }
        }
    }

    pub(crate) fn strip_lookup_meta(&mut self) {
        for meta in self.meta.values_mut() {
            meta.clear_version_context();
        }
    }
}

pub(crate) fn read_snapshot(contents: &str) -> anyhow::Result<Snapshot> {
    let parsed: serde_json::Value = serde_json::from_str(contents)?;
    let mut snapshot = if parsed.get("files").is_some() || parsed.get("meta").is_some() {
        serde_json::from_value(parsed)?
    } else {
        Snapshot {
            meta: BTreeMap::new(),
            files: serde_json::from_value(parsed)?,
            action_meta: BTreeMap::new(),
        }
    };
    snapshot.normalize();
    Ok(snapshot)
}

/// Parses Renovate's NDJSON log and returns the dependency snapshot.
pub(crate) fn extract_deps(
    log_bytes: &[u8],
    exclude_managers: &[String],
) -> anyhow::Result<Snapshot> {
    let log = std::str::from_utf8(log_bytes)?;

    let exclude: HashSet<_> = exclude_managers
        .iter()
        .map(|manager| canonical_manager_name(manager).to_string())
        .collect();

    // Find the last "packageFiles with updates" log entry — Renovate emits it
    // once per run with the full resolved config.
    let mut config_obj: Option<serde_json::Value> = None;
    for line in log.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry
            .get("msg")
            .and_then(|v| v.as_str())
            .is_some_and(|msg| PACKAGE_FILES_MSGS.contains(&msg))
        {
            let extracted_config = entry
                .get("packageFiles")
                .cloned()
                .or_else(|| entry.get("config").cloned());
            if extracted_config.is_some() {
                config_obj = extracted_config;
            }
        }
    }

    let config = config_obj
        .ok_or_else(|| anyhow::anyhow!("none of {:?} found in Renovate log", PACKAGE_FILES_MSGS))?;

    let mut deps_by_file: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    let mut meta_by_dep: BTreeMap<String, DepMetaAccumulator> = BTreeMap::new();
    let mut action_meta = BTreeMap::new();

    if let Some(obj) = config.as_object() {
        for (manager, manager_files) in obj {
            let manager = canonical_manager_name(manager);
            if exclude.contains(manager) {
                continue;
            }
            let Some(files) = manager_files.as_array() else {
                continue;
            };
            for pkg_file in files {
                let file_path = pkg_file
                    .get("packageFile")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let Some(deps) = pkg_file.get("deps").and_then(|v| v.as_array()) else {
                    continue;
                };
                for dep in deps {
                    let skip_reason = dep.get("skipReason").and_then(|v| v.as_str());
                    if SKIP_REASONS.contains(&skip_reason.unwrap_or("")) {
                        continue;
                    }
                    let Some(dep_name) = dep.get("depName").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let next_meta = DepMeta {
                        package_name: dep
                            .get("packageName")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                        datasource: dep
                            .get("datasource")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                        current_value: dep
                            .get("currentValue")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                        current_version: dep
                            .get("currentVersion")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                        extract_version: dep
                            .get("extractVersion")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                    };
                    if manager == "github-actions"
                        && let Some(next_action) = action_meta_from_dep(dep)?
                        && let Some(previous) =
                            action_meta.insert(next_action.0.clone(), next_action.1.clone())
                        && previous != next_action.1
                    {
                        anyhow::bail!(
                            "GitHub Action {} has conflicting stable metadata: {:?} vs {:?}",
                            next_action.0,
                            previous,
                            next_action.1
                        );
                    }
                    meta_by_dep
                        .entry(dep_name.to_string())
                        .or_default()
                        .merge(&next_meta);
                    deps_by_file
                        .entry(file_path.clone())
                        .or_default()
                        .entry(manager.to_string())
                        .or_default()
                        .insert(dep_name.to_string());
                }
            }
        }
    }

    // BTreeMap + BTreeSet already sorted; convert sets to vecs.
    let files = deps_by_file
        .into_iter()
        .map(|(file, managers)| {
            let managers = managers
                .into_iter()
                .map(|(m, deps)| (m, deps.into_iter().collect::<Vec<_>>()))
                .collect();
            (file, managers)
        })
        .collect();

    let meta = meta_by_dep
        .into_iter()
        .map(|(dep_name, meta)| (dep_name, meta.finish()))
        .collect();

    Ok(Snapshot {
        meta,
        files,
        action_meta,
    })
}

/// Extract stable metadata for a reusable action from Renovate's dependency
/// record. The github-actions manager keeps the action sub-path in
/// `replaceString`, while `depName` remains the repository name. Tracking the
/// complete target prevents paths in a monorepo from collapsing together.
fn action_meta_from_dep(dep: &serde_json::Value) -> anyhow::Result<Option<(String, ActionMeta)>> {
    let package_name = dep
        .get("packageName")
        .and_then(|v| v.as_str())
        .or_else(|| dep.get("depName").and_then(|v| v.as_str()));
    let Some(package_name) = package_name else {
        return Ok(None);
    };
    let Some(replace_string) = dep.get("replaceString").and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    // replaceString is the source fragment, e.g.
    // `grafana/shared-workflows/actions/create-github-app-token@<sha> # ...`.
    let target = replace_string
        .split('@')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['"', '\'']);
    let path = target
        .strip_prefix(&format!("{package_name}/"))
        .unwrap_or_else(|| if target == package_name { "" } else { target });
    if path == target && target != package_name {
        // The source fragment is not the package Renovate extracted (for
        // example a registry alias); do not invent an action identity.
        return Ok(None);
    }

    let current_ref = dep
        .get("currentValue")
        .and_then(|v| v.as_str())
        .or_else(|| {
            replace_string
                .split_once('#')
                .map(|(_, comment)| comment.trim())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            replace_string
                .split('@')
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(current_ref) = current_ref else {
        return Ok(None);
    };
    // A full SHA without a Renovate tag/comment gives us no stable namespace
    // to validate. Lookup still validates the digest itself.
    if is_sha(current_ref) {
        return Ok(None);
    }

    let target_key = if path.is_empty() {
        package_name.to_string()
    } else {
        format!("{package_name}/{path}")
    };

    // Do not infer a namespace from the action path: repositories are free to
    // use repo-level tags (v1), component tags (foo/v1), or branch refs for
    // nested actions. The extracted ref itself is the source of truth for the
    // stable shape. A subsequent change from foo/v1 to v1 is caught because
    // `compatibility` changes from Some("foo") to None.
    if let Some(compatibility) = version_tag_compatibility(current_ref) {
        return Ok(Some((
            target_key,
            ActionMeta {
                package_name: package_name.to_string(),
                ref_kind: "version-tag".to_string(),
                compatibility,
                ref_: None,
            },
        )));
    }

    Ok(Some((
        target_key,
        ActionMeta {
            package_name: package_name.to_string(),
            ref_kind: "branch".to_string(),
            compatibility: None,
            ref_: Some(current_ref.to_string()),
        },
    )))
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Returns the optional compatibility prefix for a semver-like action ref.
/// For `v1.2.3` this is `None`; for `component/v1.2.3` or
/// `component-v1.2.3` it is `Some("component")`.
pub(crate) fn version_tag_compatibility(value: &str) -> Option<Option<String>> {
    let value = value
        .strip_prefix(|c| c == 'v' || c == 'V')
        .unwrap_or(value);
    if is_numeric_version(value) {
        return Some(None);
    }

    // Tags with a namespace are separated from their semver suffix by a
    // slash or hyphen. Try every separator so prerelease hyphens are not
    // mistaken for the namespace delimiter.
    for (separator, _) in value
        .char_indices()
        .filter(|(_, character)| *character == '/' || *character == '-')
    {
        let (prefix, suffix) = value.split_at(separator);
        let suffix = suffix.get(1..).unwrap_or_default();
        let suffix = suffix
            .strip_prefix(|c| c == 'v' || c == 'V')
            .unwrap_or(suffix);
        if !prefix.is_empty() && is_numeric_version(suffix) {
            return Some(Some(prefix.to_string()));
        }
    }
    None
}

fn is_numeric_version(value: &str) -> bool {
    let core_end = value.find(['-', '+']).unwrap_or(value.len());
    let core = &value[..core_end];
    let suffix = &value[core_end..];
    let parts: Vec<_> = core.split('.').collect();
    (1..=3).contains(&parts.len())
        && !core.is_empty()
        && (suffix.is_empty() || suffix.len() > 1)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

pub(super) fn canonical_manager_name(manager: &str) -> &str {
    match manager {
        "renovate-config-presets" => "renovate-config",
        _ => manager,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DepMetaAccumulator {
    package_names: BTreeSet<String>,
    datasources: BTreeSet<String>,
    current_values: BTreeSet<String>,
    current_versions: BTreeSet<String>,
    extract_versions: BTreeSet<String>,
}

impl DepMetaAccumulator {
    fn merge(&mut self, next: &DepMeta) {
        insert_if_some(&mut self.package_names, next.package_name.as_ref());
        insert_if_some(&mut self.datasources, next.datasource.as_ref());
        insert_if_some(&mut self.current_values, next.current_value.as_ref());
        insert_if_some(&mut self.current_versions, next.current_version.as_ref());
        insert_if_some(&mut self.extract_versions, next.extract_version.as_ref());
    }

    fn finish(self) -> DepMeta {
        DepMeta {
            package_name: collapse_unique(self.package_names),
            datasource: collapse_unique(self.datasources),
            current_value: collapse_unique(self.current_values),
            current_version: collapse_unique(self.current_versions),
            extract_version: collapse_unique(self.extract_versions),
        }
    }
}

fn insert_if_some(set: &mut BTreeSet<String>, value: Option<&String>) {
    if let Some(value) = value {
        set.insert(value.clone());
    }
}

fn collapse_unique(values: BTreeSet<String>) -> Option<String> {
    if values.len() == 1 {
        values.into_iter().next()
    } else {
        None
    }
}

pub(crate) fn write_snapshot(path: &Path, deps: &Snapshot) -> anyhow::Result<()> {
    let mut deps = deps.clone();
    deps.normalize();
    let json = serde_json::to_string_pretty(&deps)?;
    std::fs::write(path, json + "\n")?;
    Ok(())
}

pub(crate) fn unified_diff(old: &Snapshot, new: &Snapshot, committed_display: &str) -> String {
    let mut old = old.clone();
    old.normalize();
    let mut new = new.clone();
    new.normalize();

    let old_text = serde_json::to_string_pretty(&old).unwrap_or_default() + "\n";
    let new_text = serde_json::to_string_pretty(&new).unwrap_or_default() + "\n";

    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
    diff.unified_diff()
        .header(committed_display, "generated")
        .to_string()
}
