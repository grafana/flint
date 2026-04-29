use anyhow::{Context, Result};
use std::io;
use std::path::Path;
use std::process::Command;

use super::detection::parse_tool_keys;

fn run_mise_use(project_root: &Path, key: &str, version: &str) {
    let _ = Command::new("mise")
        .args(["use", "--pin", &format!("{key}@{version}")])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn pin_tool_via_mise_with(
    project_root: &Path,
    key: &str,
    version: &str,
    mut runner: impl FnMut(&Path, &str, &str),
) -> bool {
    let mise_path = project_root.join("mise.toml");
    let before = std::fs::read_to_string(&mise_path).unwrap_or_default();
    runner(project_root, key, version);
    let after = std::fs::read_to_string(&mise_path).unwrap_or_default();
    after != before && parse_tool_keys(&after).contains(key)
}

/// True when `[tools]` contains at least one `npm:*` key but no `node` entry.
/// The npm backend needs a Node.js runtime; without an explicit pin, mise falls
/// back to system node — may be absent, wrong version, or drift across machines.
pub(crate) fn needs_node_for_npm(content: &str) -> bool {
    let Ok(doc) = content.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    let Some(tools) = doc.get("tools").and_then(|t| t.as_table()) else {
        return false;
    };
    let has_npm = tools.iter().any(|(k, _)| k.starts_with("npm:"));
    let has_node = tools.contains_key("node");
    has_npm && !has_node
}

/// Ensures a `node` entry exists in mise.toml when any `npm:*` backend tool is
/// present. Prefers `mise use --pin node@lts` so the version resolves to a
/// concrete release at write time; falls back to writing `node = "lts"` directly
/// via toml_edit when mise isn't available.
///
/// Returns `true` if a `node` entry was added.
pub(crate) fn ensure_node_for_npm(project_root: &Path) -> Result<bool> {
    ensure_node_for_npm_with(project_root, run_mise_use)
}

fn ensure_node_for_npm_with(
    project_root: &Path,
    runner: impl FnMut(&Path, &str, &str),
) -> Result<bool> {
    let mise_path = project_root.join("mise.toml");
    let content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    if !needs_node_for_npm(&content) {
        return Ok(false);
    }
    if pin_tool_via_mise_with(project_root, "node", "lts", runner) {
        return Ok(true);
    }
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse mise.toml")?;
    let tools = doc["tools"]
        .as_table_mut()
        .context("[tools] is not a table")?;
    tools.insert("node", toml_edit::value("lts"));
    std::fs::write(&mise_path, doc.to_string())?;
    Ok(true)
}

/// Pins `github:grafana/flint` in mise.toml at the calling binary's version so
/// contributors all run the same flint release. Skips when the key already
/// exists (any pin — never overwrite the user's explicit choice). Pre-release
/// suffixes are stripped to match the Renovate preset tag format.
///
/// Returns `true` if a flint entry was added.
pub(crate) fn ensure_flint_self_pin(project_root: &Path, flint_rev: Option<&str>) -> Result<bool> {
    ensure_flint_self_pin_with(project_root, flint_rev, run_mise_use)
}

fn ensure_flint_self_pin_with(
    project_root: &Path,
    flint_rev: Option<&str>,
    runner: impl FnMut(&Path, &str, &str),
) -> Result<bool> {
    const RELEASE_KEY: &str = "github:grafana/flint";
    const CARGO_KEY: &str = "cargo:https://github.com/grafana/flint";
    let mise_path = project_root.join("mise.toml");
    let content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let ver = env!("CARGO_PKG_VERSION");
    let ver = ver.split('-').next().unwrap_or(ver);
    let mut doc: toml_edit::DocumentMut = if content.is_empty() {
        "[tools]\n".parse().unwrap()
    } else {
        content.parse().context("failed to parse mise.toml")?
    };
    if doc.get("tools").is_none() {
        doc["tools"] = toml_edit::table();
    }
    let keys_to_remove = doc
        .get("tools")
        .and_then(|t| t.as_table())
        .map(|tools| {
            tools
                .iter()
                .filter_map(|(key, _)| {
                    should_remove_existing_flint_pin(key, flint_rev, RELEASE_KEY, CARGO_KEY)
                        .then_some(key.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let removing_flint_key = !keys_to_remove.is_empty();
    let mut changed = false;
    let tools = doc["tools"]
        .as_table_mut()
        .context("[tools] is not a table")?;
    for key in keys_to_remove {
        if tools.remove(&key).is_some() {
            changed = true;
        }
    }

    match flint_rev {
        Some(rev) => {
            let rev = rev.strip_prefix("rev:").unwrap_or(rev);
            let value = format!("rev:{rev}");
            if tools.get(CARGO_KEY).and_then(|item| item.as_str()) != Some(value.as_str()) {
                tools.insert(CARGO_KEY, toml_edit::value(value));
                changed = true;
            }
        }
        None => {
            if !tools.contains_key(RELEASE_KEY) {
                if !removing_flint_key
                    && pin_tool_via_mise_with(project_root, RELEASE_KEY, ver, runner)
                {
                    return Ok(true);
                } else {
                    tools.insert(RELEASE_KEY, toml_edit::value(ver));
                    changed = true;
                }
            }
        }
    }

    if changed {
        std::fs::write(&mise_path, doc.to_string())?;
    }
    Ok(changed)
}

fn should_remove_existing_flint_pin(
    key: &str,
    flint_rev: Option<&str>,
    release_key: &str,
    cargo_key: &str,
) -> bool {
    if !is_flint_tool_key(key) {
        return false;
    }

    match flint_rev {
        Some(_) => key != cargo_key,
        None => key != release_key,
    }
}

fn is_flint_tool_key(key: &str) -> bool {
    key == "github:grafana/flint"
        || key.starts_with("cargo:https://github.com/grafana/flint")
        || key.starts_with("cargo:https://github.com/grafana/flint.git")
}

/// Replaces obsolete tool keys in mise.toml with their modern equivalents,
/// preserving the existing version value. Returns the list of replacements made
/// as `(old_key, new_key)` pairs. No-ops if the file doesn't exist or has no
/// obsolete keys.
pub(crate) fn replace_obsolete_keys(
    project_root: &Path,
    obsolete: &[(&str, &str)],
) -> Result<Vec<(String, String)>> {
    let path = project_root.join("mise.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse mise.toml")?;

    let mut replaced = vec![];
    if let Some(tools) = doc.get_mut("tools").and_then(|t| t.as_table_mut()) {
        for &(old_key, new_key) in obsolete {
            if let Some(value) = tools.remove(old_key) {
                tools.insert(new_key, value);
                replaced.push((old_key.to_string(), new_key.to_string()));
            }
        }
    }

    if !replaced.is_empty() {
        std::fs::write(&path, doc.to_string()).context("failed to write mise.toml")?;
    }
    Ok(replaced)
}

/// Removes specific tool keys from mise.toml.
/// Returns the list of removed keys. No-ops if the file doesn't exist or none are present.
pub(crate) fn remove_tool_keys(project_root: &Path, keys: &[&str]) -> Result<Vec<String>> {
    let path = project_root.join("mise.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse mise.toml")?;

    let mut removed = vec![];
    if let Some(tools) = doc.get_mut("tools").and_then(|t| t.as_table_mut()) {
        for key in keys {
            if tools.remove(key).is_some() {
                removed.push((*key).to_string());
            }
        }
    }

    if !removed.is_empty() {
        std::fs::write(&path, doc.to_string()).context("failed to write mise.toml")?;
    }
    Ok(removed)
}

pub(super) fn apply_changes(
    path: &Path,
    current_content: &str,
    to_add: &[(String, Option<String>)],
    to_remove: &[String],
    to_upgrade: &[(String, String)],
) -> Result<()> {
    apply_changes_with(
        path,
        current_content,
        to_add,
        to_remove,
        to_upgrade,
        run_mise_use,
    )
}

fn apply_changes_with(
    path: &Path,
    current_content: &str,
    to_add: &[(String, Option<String>)],
    to_remove: &[String],
    to_upgrade: &[(String, String)],
    mut runner: impl FnMut(&Path, &str, &str),
) -> Result<()> {
    let project_root = path.parent().unwrap_or(path);

    let mut pinned_via_mise: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (key, _) in to_add {
        if pin_tool_via_mise_with(project_root, key, "latest", &mut runner) {
            pinned_via_mise.insert(key.clone());
        } else {
            eprintln!("  warning: could not pin {key} via mise — writing \"latest\"");
        }
    }

    let current_content: String = if pinned_via_mise.is_empty() {
        current_content.to_string()
    } else {
        std::fs::read_to_string(path).unwrap_or_else(|_| current_content.to_string())
    };
    let mut doc: toml_edit::DocumentMut = current_content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    if !doc.contains_key("tools") {
        doc.insert("tools", toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let tools = doc["tools"]
        .as_table_mut()
        .context("[tools] is not a table")?;

    for key in to_remove {
        tools.remove(key.as_str());
    }

    for (key, components) in to_add {
        let already_pinned = pinned_via_mise.contains(key.as_str());
        match components {
            Some(comps) => {
                let existing_version = if already_pinned {
                    tool_version(tools, key).unwrap_or_else(|| "latest".to_string())
                } else {
                    "latest".to_string()
                };
                insert_tool_with_components(tools, key, &existing_version, comps);
            }
            None => {
                if !already_pinned {
                    tools.insert(key.as_str(), toml_edit::value("latest"));
                }
            }
        }
    }

    for (key, components) in to_upgrade {
        let existing_version = tool_version(tools, key).unwrap_or_else(|| "latest".to_string());
        insert_tool_with_components(tools, key, &existing_version, components);
    }

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

fn tool_version(tools: &toml_edit::Table, key: &str) -> Option<String> {
    tools
        .get(key)
        .and_then(|item| item.as_value())
        .and_then(|v| match v {
            toml_edit::Value::String(s) => Some(s.value().to_string()),
            toml_edit::Value::InlineTable(tbl) => tbl
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            _ => None,
        })
}

fn insert_tool_with_components(
    tools: &mut toml_edit::Table,
    key: &str,
    version: &str,
    components: &str,
) {
    let mut tbl = toml_edit::InlineTable::new();
    tbl.insert("version", toml_edit::Value::from(version));
    tbl.insert("components", toml_edit::Value::from(components));
    tools.insert(
        key,
        toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
    );
}

/// Sorts `[tools]` entries and inserts the `# Linters` header when they are not
/// already in canonical form. Returns `true` if the file was rewritten.
fn normalize_tools_section_impl(path: &Path, verbose: bool) -> Result<bool> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    let mut doc: toml_edit::DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(_) => return Ok(false),
    };
    let Some(tools) = doc.get_mut("tools").and_then(|i| i.as_table_mut()) else {
        return Ok(false);
    };
    sort_and_group_tools(tools, &content);
    let new_content = doc.to_string();
    if new_content == content {
        return Ok(false);
    }
    std::fs::write(path, new_content)?;
    if verbose {
        println!("  normalized [tools] in {}", path.display());
    }
    Ok(true)
}

pub(crate) fn tools_section_needs_normalization(path: &Path) -> Result<bool> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
    };
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(tools) = doc.get_mut("tools").and_then(|i| i.as_table_mut()) else {
        return Ok(false);
    };
    sort_and_group_tools(tools, &content);
    Ok(doc.to_string() != content)
}

pub(crate) fn normalize_tools_section(path: &Path) -> Result<bool> {
    normalize_tools_section_impl(path, true)
}

/// Sorts `[tools]` entries alphabetically and inserts a `# Linters` comment
/// before the first linter entry. Runtime, SDK, and unrelated project tools stay
/// above the header; known linter keys go below.
fn sort_and_group_tools(tools: &mut toml_edit::Table, original: &str) {
    let mut entries: Vec<(String, toml_edit::Item)> = tools
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    if entries.is_empty() {
        return;
    }
    let linter_keys = crate::registry::linter_keys();
    let (mut linters, mut runtimes): (Vec<_>, Vec<_>) = entries
        .drain(..)
        .partition(|(k, _)| linter_keys.contains(k.as_str()));
    runtimes.sort_by(|a, b| a.0.cmp(&b.0));
    linters.sort_by(|a, b| a.0.cmp(&b.0));
    let preserved_prefixes: std::collections::HashMap<String, String> = tools
        .iter()
        .filter_map(|(k, _)| {
            let prefix = tools
                .key(k)
                .and_then(|key| key.leaf_decor().prefix())
                .and_then(|prefix| {
                    prefix
                        .as_str()
                        .or_else(|| prefix.span().and_then(|span| original.get(span)))
                })?;
            let is_header = prefix == "\n# Linters\n";
            (!is_header && !prefix.is_empty()).then(|| (k.to_string(), prefix.to_string()))
        })
        .collect();

    let keys: Vec<String> = tools.iter().map(|(k, _)| k.to_string()).collect();
    for k in keys {
        tools.remove(&k);
    }
    for (k, v) in runtimes {
        tools.insert(&k, v);
    }
    let first_linter_key = linters.first().map(|(k, _)| k.clone());
    for (k, v) in linters {
        tools.insert(&k, v);
    }
    for k in tools.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>() {
        if let Some(mut key_mut) = tools.key_mut(&k) {
            if let Some(prefix) = preserved_prefixes.get(&k) {
                key_mut.leaf_decor_mut().set_prefix(prefix);
                continue;
            }
            let existing_prefix = key_mut.leaf_decor().prefix().and_then(|prefix| {
                prefix
                    .as_str()
                    .or_else(|| prefix.span().and_then(|span| original.get(span)))
            });
            if existing_prefix == Some("\n# Linters\n") {
                key_mut.leaf_decor_mut().set_prefix("");
            }
        }
    }
    if let Some(k) = first_linter_key
        && let Some(mut key_mut) = tools.key_mut(&k)
    {
        key_mut.leaf_decor_mut().set_prefix("\n# Linters\n");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_changes_with, ensure_flint_self_pin, ensure_flint_self_pin_with, ensure_node_for_npm,
        ensure_node_for_npm_with, needs_node_for_npm, pin_tool_via_mise_with, remove_tool_keys,
        replace_obsolete_keys,
    };

    #[test]
    fn needs_node_when_npm_key_without_node() {
        let content = "[tools]\n\"npm:renovate\" = \"43.129.0\"\n";
        assert!(needs_node_for_npm(content));
    }

    #[test]
    fn no_node_needed_when_node_already_present() {
        let content = "[tools]\nnode = \"20\"\n\"npm:renovate\" = \"43.129.0\"\n";
        assert!(!needs_node_for_npm(content));
    }

    #[test]
    fn noop_without_npm_tools() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        let original = "[tools]\nshellcheck = \"v0.11.0\"\n";
        std::fs::write(&path, original).unwrap();

        let added = ensure_node_for_npm(dir.path()).unwrap();

        assert!(!added);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn pin_tool_via_mise_detects_successful_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\n").unwrap();

        let pinned = pin_tool_via_mise_with(dir.path(), "node", "lts", |project_root, key, _| {
            std::fs::write(
                project_root.join("mise.toml"),
                format!("[tools]\n{key} = \"24.0.0\"\n"),
            )
            .unwrap();
        });

        assert!(pinned);
    }

    #[test]
    fn ensure_node_for_npm_uses_resolved_pin_from_mise_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\n\"npm:renovate\" = \"43.129.0\"\n").unwrap();

        let added = ensure_node_for_npm_with(dir.path(), |project_root, key, version| {
            assert_eq!(key, "node");
            assert_eq!(version, "lts");
            std::fs::write(
                project_root.join("mise.toml"),
                "[tools]\nnode = \"24.0.0\"\n\"npm:renovate\" = \"43.129.0\"\n",
            )
            .unwrap();
        })
        .unwrap();

        assert!(added);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("node = \"24.0.0\""));
        assert!(result.contains("\"npm:renovate\" = \"43.129.0\""));
    }

    #[test]
    fn flint_self_pin_reverts_cargo_git_pin_to_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(
            &path,
            "[tools]\n\"cargo:https://github.com/grafana/flint\" = \"rev:deadbeef\"\n\n[env]\nFLINT_CONFIG_DIR = \".github/config\"\n",
        )
        .unwrap();

        let changed = ensure_flint_self_pin(dir.path(), None).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();

        assert!(changed);
        assert!(result.contains("\"github:grafana/flint\""));
        assert!(!result.contains("cargo:https://github.com/grafana/flint"));
        assert!(result.contains("FLINT_CONFIG_DIR = \".github/config\""));
    }

    #[test]
    fn flint_self_pin_writes_cargo_git_pin_for_rev() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(
            &path,
            "[tools]\n\"github:grafana/flint\" = \"0.20.5\"\n\n[env]\nFLINT_CONFIG_DIR = \".github/config\"\n",
        )
        .unwrap();

        let changed = ensure_flint_self_pin(dir.path(), Some("rev:deadbeef")).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();

        assert!(changed);
        assert!(result.contains("\"cargo:https://github.com/grafana/flint\" = \"rev:deadbeef\""));
        assert!(!result.contains("\"github:grafana/flint\""));
        assert!(result.contains("FLINT_CONFIG_DIR = \".github/config\""));
    }

    #[test]
    fn flint_self_pin_prefers_resolved_mise_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\nrust = \"1.95.0\"\n").unwrap();

        let changed = ensure_flint_self_pin_with(dir.path(), None, |project_root, key, version| {
            assert_eq!(key, "github:grafana/flint");
            assert_eq!(version, env!("CARGO_PKG_VERSION"));
            let version = env!("CARGO_PKG_VERSION");
            std::fs::write(
                project_root.join("mise.toml"),
                format!("[tools]\nrust = \"1.95.0\"\n\"github:grafana/flint\" = \"{version}\"\n"),
            )
            .unwrap();
        })
        .unwrap();

        assert!(changed);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains(&format!(
            "\"github:grafana/flint\" = \"{}\"",
            env!("CARGO_PKG_VERSION")
        )));
        assert!(!result.contains(&format!("version = \"{}\"", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn replaces_old_key_preserving_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\n\"github:mvdan/sh\" = \"v3.13.1\"\n").unwrap();

        let replaced = replace_obsolete_keys(dir.path(), &[("github:mvdan/sh", "shfmt")]).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();

        assert_eq!(
            replaced,
            vec![("github:mvdan/sh".to_string(), "shfmt".to_string())]
        );
        assert!(result.contains("shfmt"), "new key written: {result}");
        assert!(
            !result.contains("\"github:mvdan/sh\""),
            "old key removed: {result}"
        );
        assert!(result.contains("v3.13.1"), "version preserved: {result}");
    }

    #[test]
    fn removes_requested_tool_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(
            &path,
            "[tools]\n\"npm:prettier\" = \"3.6.2\"\n\"npm:markdownlint-cli2\" = \"0.18.1\"\n",
        )
        .unwrap();

        let removed = remove_tool_keys(dir.path(), &["npm:prettier"]).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();

        assert_eq!(removed, vec!["npm:prettier".to_string()]);
        assert!(!result.contains("npm:prettier"));
        assert!(result.contains("npm:markdownlint-cli2"));
    }

    #[test]
    fn apply_changes_preserves_resolved_versions_from_mise_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        let current = "[tools]\n";

        apply_changes_with(
            &path,
            current,
            &[("rust".to_string(), Some("clippy,rustfmt".to_string()))],
            &[],
            &[],
            |project_root, key, version| {
                assert_eq!(key, "rust");
                assert_eq!(version, "latest");
                std::fs::write(
                    project_root.join("mise.toml"),
                    "[tools]\nrust = \"1.95.0\"\n",
                )
                .unwrap();
            },
        )
        .unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("version = \"1.95.0\""));
        assert!(result.contains("components = \"clippy,rustfmt\""));
    }
}
