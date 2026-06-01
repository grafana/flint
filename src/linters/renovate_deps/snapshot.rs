use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

const PACKAGE_FILES_MSGS: &[&str] = &["Extracted dependencies", "packageFiles with updates"];
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value", "invalid-version"];

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
pub(crate) type DepFiles = BTreeMap<String, BTreeMap<String, Vec<String>>>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Snapshot {
    pub(crate) meta: BTreeMap<String, DepMeta>,
    pub(crate) files: DepFiles,
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

    pub(crate) fn strip_lookup_meta(&mut self) {
        for meta in self.meta.values_mut() {
            meta.clear_version_context();
        }
    }
}

pub(crate) fn read_snapshot(contents: &str) -> anyhow::Result<Snapshot> {
    let parsed: serde_json::Value = serde_json::from_str(contents)?;
    if parsed.get("files").is_some() || parsed.get("meta").is_some() {
        return Ok(serde_json::from_value(parsed)?);
    }
    Ok(Snapshot {
        meta: BTreeMap::new(),
        files: serde_json::from_value(parsed)?,
    })
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

    Ok(Snapshot { meta, files })
}

fn canonical_manager_name(manager: &str) -> &str {
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
    let json = serde_json::to_string_pretty(deps)?;
    std::fs::write(path, json + "\n")?;
    Ok(())
}

pub(crate) fn unified_diff(old: &Snapshot, new: &Snapshot, committed_display: &str) -> String {
    let old_text = serde_json::to_string_pretty(old).unwrap_or_default() + "\n";
    let new_text = serde_json::to_string_pretty(new).unwrap_or_default() + "\n";

    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
    diff.unified_diff()
        .header(committed_display, "generated")
        .to_string()
}
