use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::snapshot::Snapshot;

const MATCH_PREFIX: &str = "match";
const MATCH_DEP_NAMES: &str = "matchDepNames";
const MATCH_PACKAGE_NAMES: &str = "matchPackageNames";
const CONTEXTUAL_MATCHERS: &[&str] = &[
    "matchCategories",
    "matchDatasources",
    "matchDepTypes",
    "matchFileNames",
    "matchManagers",
    "matchPaths",
    "matchRepositories",
    "matchSourceUrls",
];

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

#[derive(Debug)]
pub(crate) struct ComparablePackageRules {
    pub(crate) rules: Vec<ComparablePackageRule>,
    pub(crate) skipped_notes: Vec<String>,
}

pub(crate) fn comparable_package_rules_for_config(
    config_path: &Path,
) -> anyhow::Result<ComparablePackageRules> {
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
                .map(|(idx, rule)| comparable_package_rule(rule, idx))
                .collect::<anyhow::Result<Vec<_>>>()
                .map(|rules| {
                    let mut comparable = Vec::new();
                    let mut skipped_notes = Vec::new();
                    for rule in rules.into_iter().flatten() {
                        match rule {
                            ComparableRuleOutcome::Comparable(rule) => comparable.push(rule),
                            ComparableRuleOutcome::Skipped { note } => skipped_notes.push(note),
                        }
                    }
                    ComparablePackageRules {
                        rules: comparable,
                        skipped_notes,
                    }
                })
        })
        .transpose()?
        .unwrap_or_else(|| ComparablePackageRules {
            rules: Vec::new(),
            skipped_notes: Vec::new(),
        }))
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

enum ComparableRuleOutcome {
    Comparable(ComparablePackageRule),
    Skipped { note: String },
}

fn comparable_package_rule(
    rule: &serde_json::Value,
    idx: usize,
) -> anyhow::Result<Option<ComparableRuleOutcome>> {
    let extra_matchers: Vec<_> = rule
        .as_object()
        .into_iter()
        .flat_map(|obj| obj.keys())
        .filter(|key| key.starts_with(MATCH_PREFIX))
        .filter(|key| *key != MATCH_DEP_NAMES && *key != MATCH_PACKAGE_NAMES)
        .cloned()
        .collect();
    let contextual_matchers: Vec<_> = extra_matchers
        .iter()
        .filter(|key| requires_contextual_matching(key))
        .cloned()
        .collect();

    let dep_names = optional_matcher_values(rule, idx, MATCH_DEP_NAMES)?;
    let package_names = optional_matcher_values(rule, idx, MATCH_PACKAGE_NAMES)?;

    let label = rule_label(rule, idx);

    match (&dep_names, &package_names) {
        (Some(_), Some(_)) => {
            anyhow::bail!(
                "package rule {label} declares both matchDepNames and matchPackageNames; flint requires exactly one for rule-coverage checks"
            );
        }
        (None, None) => return Ok(None),
        _ => {}
    }

    if !contextual_matchers.is_empty() {
        return Ok(Some(ComparableRuleOutcome::Skipped {
            note: format!(
                "skipped package rule {label} for coverage validation because it uses context-sensitive matchers [{}]",
                contextual_matchers.join(", "),
            ),
        }));
    }

    let matcher = if let Some(names) = dep_names {
        RuleMatcher::DepNames(names)
    } else if let Some(names) = package_names {
        RuleMatcher::PackageNames(names)
    } else {
        unreachable!("handled by the match above")
    };

    Ok(Some(ComparableRuleOutcome::Comparable(
        ComparablePackageRule { label, matcher },
    )))
}

fn rule_label(rule: &serde_json::Value, idx: usize) -> String {
    rule["groupName"]
        .as_str()
        .map(|group| format!("group {group:?}"))
        .or_else(|| {
            rule["description"]
                .as_str()
                .map(|desc| format!("description {desc:?}"))
        })
        .unwrap_or_else(|| format!("index {idx}"))
}

fn optional_matcher_values(
    rule: &serde_json::Value,
    idx: usize,
    key: &'static str,
) -> anyhow::Result<Option<BTreeSet<String>>> {
    let Some(value) = rule.get(key) else {
        return Ok(None);
    };

    let names = value.as_array().ok_or_else(|| {
        anyhow::anyhow!("package rule index {idx} must declare {key} as an array")
    })?;

    let mut out = BTreeSet::new();
    for (name_idx, value) in names.iter().enumerate() {
        let name = value.as_str().ok_or_else(|| {
            anyhow::anyhow!("package rule index {idx} must declare {key}[{name_idx}] as a string")
        })?;
        out.insert(name.to_string());
    }
    Ok(Some(out))
}

fn requires_contextual_matching(key: &str) -> bool {
    CONTEXTUAL_MATCHERS.contains(&key)
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
