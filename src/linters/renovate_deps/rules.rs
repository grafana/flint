use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::snapshot::Snapshot;

#[derive(Debug)]
pub(crate) enum RuleMatcher {
    DepNames(BTreeSet<String>),
    PackageNames(BTreeSet<String>),
}

#[derive(Debug)]
pub(crate) struct ComparablePackageRule {
    pub(crate) label: String,
    pub(crate) matcher: RuleMatcher,
}

pub(crate) fn comparable_package_rules_for_config(
    config_path: &Path,
) -> anyhow::Result<Vec<ComparablePackageRule>> {
    let config = std::fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let parsed: serde_json::Value = json5::from_str(&config)
        .with_context(|| format!("failed to parse {}", config_path.display()))?;

    Ok(parsed["packageRules"]
        .as_array()
        .map(|rules| {
            rules
                .iter()
                .enumerate()
                .filter_map(|(idx, rule)| comparable_package_rule(rule, idx))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default())
}

pub(crate) fn validate_rule_coverage(
    snapshot: &Snapshot,
    rules: &[ComparablePackageRule],
) -> anyhow::Result<()> {
    let package_groups = package_groups(snapshot);
    let mut errors = vec![];

    for ((package_name, datasource), deps) in package_groups {
        if deps.len() < 2 {
            continue;
        }
        for rule in rules {
            let matched: Vec<_> = deps
                .iter()
                .filter(|dep| rule.matches(dep, snapshot))
                .collect();
            if matched.is_empty() || matched.len() == deps.len() {
                continue;
            }
            let matched = matched
                .into_iter()
                .map(|dep| dep.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let unmatched = deps
                .iter()
                .filter(|dep| !rule.matches(dep, snapshot))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            errors.push(format!(
                "package rule {} matches package {} inconsistently: matched [{}], unmatched [{}] (datasource {})",
                rule.label,
                package_name,
                matched,
                unmatched,
                datasource.as_deref().unwrap_or("unknown"),
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(errors.join("\n"))
    }
}

pub(crate) fn trim_snapshot_meta(snapshot: &mut Snapshot, rules: &[ComparablePackageRule]) {
    let relevant = relevant_dep_names(snapshot, rules);
    snapshot
        .meta
        .retain(|dep_name, _| relevant.contains(dep_name));
}

fn comparable_package_rule(rule: &serde_json::Value, idx: usize) -> Option<ComparablePackageRule> {
    let extra_matchers = rule
        .as_object()
        .into_iter()
        .flat_map(|obj| obj.keys())
        .filter(|key| key.starts_with("match"))
        .filter(|key| *key != "matchDepNames" && *key != "matchPackageNames")
        .count();
    if extra_matchers > 0 {
        return None;
    }

    let label = rule["groupName"]
        .as_str()
        .map(|group| format!("group {group:?}"))
        .or_else(|| {
            rule["description"]
                .as_str()
                .map(|desc| format!("description {desc:?}"))
        })
        .unwrap_or_else(|| format!("index {idx}"));

    let matcher = if let Some(names) = rule.get("matchDepNames").and_then(|v| v.as_array()) {
        RuleMatcher::DepNames(
            names
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .expect("package rule matchDepNames entries must be strings")
                        .to_string()
                })
                .collect(),
        )
    } else if let Some(names) = rule.get("matchPackageNames").and_then(|v| v.as_array()) {
        RuleMatcher::PackageNames(
            names
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .expect("package rule matchPackageNames entries must be strings")
                        .to_string()
                })
                .collect(),
        )
    } else {
        return None;
    };

    Some(ComparablePackageRule { label, matcher })
}

impl ComparablePackageRule {
    fn matches(&self, dep_name: &str, snapshot: &Snapshot) -> bool {
        match &self.matcher {
            RuleMatcher::DepNames(names) => names.contains(dep_name),
            RuleMatcher::PackageNames(names) => snapshot
                .meta
                .get(dep_name)
                .and_then(|meta| meta.package_name.as_deref())
                .is_some_and(|package_name| names.contains(package_name)),
        }
    }
}

fn package_groups(snapshot: &Snapshot) -> BTreeMap<(String, Option<String>), BTreeSet<String>> {
    let mut groups = BTreeMap::new();
    for dep_name in snapshot
        .files
        .values()
        .flat_map(|managers| managers.values())
        .flatten()
    {
        let Some(meta) = snapshot.meta.get(dep_name) else {
            continue;
        };
        let Some(package_name) = meta.package_name.as_ref() else {
            continue;
        };
        groups
            .entry((package_name.clone(), meta.datasource.clone()))
            .or_insert_with(BTreeSet::new)
            .insert(dep_name.clone());
    }
    groups
}

pub(crate) fn relevant_dep_names(
    snapshot: &Snapshot,
    rules: &[ComparablePackageRule],
) -> BTreeSet<String> {
    let mut relevant = BTreeSet::new();
    let extracted_dep_names: BTreeSet<_> = snapshot
        .files
        .values()
        .flat_map(|managers| managers.values())
        .flatten()
        .cloned()
        .collect();

    for rule in rules {
        match &rule.matcher {
            RuleMatcher::DepNames(names) => {
                relevant.extend(
                    names
                        .iter()
                        .filter(|dep_name| extracted_dep_names.contains(*dep_name))
                        .cloned(),
                );
            }
            RuleMatcher::PackageNames(names) => {
                relevant.extend(snapshot.meta.iter().filter_map(|(dep_name, meta)| {
                    meta.package_name
                        .as_deref()
                        .is_some_and(|package_name| names.contains(package_name))
                        .then_some(dep_name.clone())
                }));
            }
        }
    }

    let package_groups = package_groups(snapshot);
    for deps in package_groups.values() {
        if deps.iter().any(|dep_name| relevant.contains(dep_name)) {
            relevant.extend(deps.iter().cloned());
        }
    }

    relevant
}
