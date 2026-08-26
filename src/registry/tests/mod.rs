use super::*;

fn package_rule_by_group_name<'a>(
    parsed: &'a serde_json::Value,
    group_name: &str,
) -> Option<&'a serde_json::Value> {
    parsed["packageRules"]
        .as_array()?
        .iter()
        .find(|rule| rule["groupName"].as_str() == Some(group_name))
}

fn extract_version_rules(parsed: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut rules: Vec<_> = parsed["packageRules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| !rule["extractVersion"].is_null())
        .cloned()
        .collect();
    rules.sort_by(|left, right| {
        left["description"]
            .as_str()
            .cmp(&right["description"].as_str())
    });
    rules
}

fn package_names(rule: &serde_json::Value) -> Vec<&str> {
    rule["matchPackageNames"]
        .as_array()
        .expect("package rule must declare matchPackageNames")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("package rule matchPackageNames entries must be strings")
        })
        .collect()
}

fn dep_names(rule: &serde_json::Value) -> Vec<&str> {
    rule["matchDepNames"]
        .as_array()
        .expect("package rule must declare matchDepNames")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("package rule matchDepNames entries must be strings")
        })
        .collect()
}

fn sorted_dep_names(rule: &serde_json::Value) -> Vec<&str> {
    let mut names = dep_names(rule);
    names.sort_unstable();
    names
}

fn rule_name_field(rule: &serde_json::Value) -> &'static str {
    match (
        rule.get("matchDepNames").is_some(),
        rule.get("matchPackageNames").is_some(),
    ) {
        (true, false) => "matchDepNames",
        (false, true) => "matchPackageNames",
        (true, true) => {
            panic!("package rule must not declare both matchDepNames and matchPackageNames")
        }
        (false, false) => {
            panic!("package rule must declare matchDepNames or matchPackageNames")
        }
    }
}

fn rule_names(rule: &serde_json::Value) -> Vec<&str> {
    match rule_name_field(rule) {
        "matchDepNames" => dep_names(rule),
        "matchPackageNames" => package_names(rule),
        _ => unreachable!("unexpected rule_name_field result"),
    }
}

fn sorted_rule_names(rule: &serde_json::Value) -> Vec<&str> {
    let mut names = rule_names(rule);
    names.sort_unstable();
    names
}

/// Verifies README and docs overview tables plus the metadata on each
/// dedicated linter page stay in sync with the registry.
///
/// Run `mise run generate` to regenerate.
mod docs;
mod migrations;
mod registry;
