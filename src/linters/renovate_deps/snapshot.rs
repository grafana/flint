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
}

impl Snapshot {
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty()
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

    let exclude: HashSet<&str> = exclude_managers.iter().map(String::as_str).collect();

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
            if exclude.contains(manager.as_str()) {
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
                    };
                    meta_by_dep
                        .entry(dep_name.to_string())
                        .or_default()
                        .merge(&next_meta);
                    deps_by_file
                        .entry(file_path.clone())
                        .or_default()
                        .entry(manager.clone())
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DepMetaAccumulator {
    package_names: BTreeSet<String>,
    datasources: BTreeSet<String>,
}

impl DepMetaAccumulator {
    fn merge(&mut self, next: &DepMeta) {
        if let Some(package_name) = next.package_name.as_ref() {
            self.package_names.insert(package_name.clone());
        }
        if let Some(datasource) = next.datasource.as_ref() {
            self.datasources.insert(datasource.clone());
        }
    }

    fn finish(self) -> DepMeta {
        DepMeta {
            package_name: collapse_unique(self.package_names),
            datasource: collapse_unique(self.datasources),
        }
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
