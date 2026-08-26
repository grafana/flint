use super::install_patch::configure_extract_workaround_env;
use super::mise_normalize::patch_semver_equivalent_mise_values;
use super::rules::{
    ComparablePackageRule, ExtractVersionMismatch, RuleMatcher, equivalent_version_shapes,
    incomplete_meta_for_rules, relevant_dep_names, validate_extract_version_consistency,
};
use super::snapshot::{ActionMeta, DepFiles, DepMeta, version_tag_compatibility};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

type FileManagers<'a> = [(&'a str, &'a [(&'a str, &'a [&'a str])])];

fn log(config_json: &str) -> Vec<u8> {
    format!(r#"{{"msg":"Extracted dependencies","packageFiles":{config_json}}}"#).into_bytes()
}

fn log_current(config_json: &str) -> Vec<u8> {
    let config_json = config_json.lines().map(str::trim).collect::<String>();
    format!(r#"{{"msg":"packageFiles with updates","config":{config_json}}}"#).into_bytes()
}

fn dep_files(entries: &FileManagers<'_>) -> DepFiles {
    entries
        .iter()
        .map(|(file, managers)| {
            let m = managers
                .iter()
                .map(|(mgr, deps)| {
                    (
                        mgr.to_string(),
                        deps.iter().map(|d| d.to_string()).collect(),
                    )
                })
                .collect();
            (file.to_string(), m)
        })
        .collect()
}

fn snapshot(meta: &[(&str, Option<&str>, Option<&str>)], files: &FileManagers<'_>) -> Snapshot {
    Snapshot {
        meta: meta
            .iter()
            .map(|(dep, package_name, datasource)| {
                (
                    dep.to_string(),
                    DepMeta {
                        package_name: package_name.map(ToOwned::to_owned),
                        datasource: datasource.map(ToOwned::to_owned),
                        current_value: None,
                        current_version: None,
                        extract_version: None,
                    },
                )
            })
            .collect(),
        action_meta: BTreeMap::new(),
        files: dep_files(files),
    }
}

fn validate_env(vars: &[(&str, &str)]) -> Result<Option<String>, String> {
    let vars: std::collections::HashMap<String, String> = vars
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    validate_runtime_env_from(|name| vars.get(name).cloned())
}

fn write_tmp(content: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), content).unwrap();
    file
}

mod configuration;
mod diagnostics;
mod extraction;
mod relevance;
mod rules;
mod snapshot;
