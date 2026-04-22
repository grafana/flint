use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use super::LinterGroup;
use super::detection::parse_tool_keys;

/// Returns the renovate preset entry to inject, e.g. `github>grafana/flint#v0.9.2`.
/// Pre-release suffixes are stripped so dev builds produce a valid tag reference.
pub(super) fn flint_preset() -> String {
    let ver = env!("CARGO_PKG_VERSION");
    let ver = ver.split('-').next().unwrap_or(ver);
    format!("github>grafana/flint#v{ver}")
}

/// Adds the flint renovate preset to the `extends` array in a renovate config file.
/// Works for both JSON and JSON5. If an unpinned or differently-pinned flint entry
/// already exists, it is replaced in-place rather than duplicated.
/// Returns `true` if the file was changed.
pub(super) fn patch_renovate_extends(path: &Path) -> Result<bool> {
    let entry = flint_preset();
    let content = std::fs::read_to_string(path)?;

    if content.contains(&entry) {
        return Ok(false);
    }

    // If an existing flint entry (any pin) is present, replace it in-place.
    const FLINT_ENTRY_PREFIX: &str = "\"github>grafana/flint";
    let new_content = if let Some(pos) = content.find(FLINT_ENTRY_PREFIX) {
        let after_open = pos + 1; // skip leading "
        let close = content[after_open..]
            .find('"')
            .context("unclosed quote in existing flint preset entry")?;
        let end = after_open + close + 1; // position after closing "
        format!("{}\"{}\"{}", &content[..pos], entry, &content[end..])
    } else {
        add_to_extends(&content, &entry)
            .with_context(|| format!("failed to patch extends in {}", path.display()))?
    };

    std::fs::write(path, new_content)?;
    Ok(true)
}

/// Text-based insertion of `entry` into the `extends` array.
/// Works for both JSON (`"extends": [`) and JSON5 (`extends: [`).
fn add_to_extends(content: &str, entry: &str) -> Result<String> {
    let re = regex::Regex::new(r#"(?:"extends"|extends)\s*:\s*\["#).unwrap();

    if let Some(m) = re.find(content) {
        let bracket_pos = m.end() - 1; // index of '['
        let inside_start = bracket_pos + 1;

        let close_offset = content[inside_start..]
            .find(']')
            .context("extends array has no closing ]")?;
        let close_pos = inside_start + close_offset;
        let inside = &content[inside_start..close_pos];

        if inside.contains('\n') {
            // Multiline: detect indent from first non-empty line, insert at top
            let indent = inside
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| " ".repeat(l.len() - l.trim_start().len()))
                .unwrap_or_else(|| "  ".to_string());
            Ok(format!(
                "{}\n{}\"{}\"{}{}",
                &content[..inside_start],
                indent,
                entry,
                ",",
                &content[inside_start..]
            ))
        } else {
            // Single-line (empty or not): prepend entry
            let sep = if inside.trim().is_empty() { "" } else { ", " };
            Ok(format!(
                "{}\"{}\"{}{}",
                &content[..inside_start],
                entry,
                sep,
                &content[inside_start..]
            ))
        }
    } else {
        // No extends key — add after the opening {
        let open = content
            .find('{')
            .context("no opening { in renovate config")?;
        let (before, after) = content.split_at(open + 1);
        Ok(format!(
            "{}\n  \"extends\": [\"{}\"],{}",
            before, entry, after
        ))
    }
}

/// Runs `mise use --pin <key>@<version>` in the project directory to add a tool
/// with a pinned version (mise resolves `latest`/`lts` to a concrete version at
/// write time). Returns `true` if the key was written to the config (checked by
/// re-reading the file), ignoring non-zero exit codes that arise from post-write
/// steps like shim rebuilds failing in restricted environments.
fn pin_tool_via_mise(project_root: &Path, key: &str, version: &str) -> bool {
    let mise_path = project_root.join("mise.toml");
    let before = std::fs::read_to_string(&mise_path).unwrap_or_default();

    let _ = Command::new("mise")
        .args(["use", "--pin", &format!("{key}@{version}")])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // Success = the key is now present in the config (regardless of exit code).
    let after = std::fs::read_to_string(&mise_path).unwrap_or_default();
    after != before && parse_tool_keys(&after).contains(key)
}

/// True when `[tools]` contains at least one `npm:*` key but no `node` entry.
/// The npm backend needs a Node.js runtime; without an explicit pin, mise falls
/// back to system node — may be absent, wrong version, or drift across machines.
fn needs_node_for_npm(content: &str) -> bool {
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
    let mise_path = project_root.join("mise.toml");
    let content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    if !needs_node_for_npm(&content) {
        return Ok(false);
    }
    if pin_tool_via_mise(project_root, "node", "lts") {
        return Ok(true);
    }
    // Fallback: write a soft pin so mise.toml still declares the prereq.
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
/// suffixes are stripped to match [`flint_preset`].
///
/// Returns `true` if a flint entry was added.
pub(crate) fn ensure_flint_self_pin(project_root: &Path) -> Result<bool> {
    const KEY: &str = "github:grafana/flint";
    let mise_path = project_root.join("mise.toml");
    let content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    if let Ok(doc) = content.parse::<toml_edit::DocumentMut>()
        && let Some(tools) = doc.get("tools").and_then(|t| t.as_table())
        && tools.contains_key(KEY)
    {
        return Ok(false);
    }
    let ver = env!("CARGO_PKG_VERSION");
    let ver = ver.split('-').next().unwrap_or(ver);
    if pin_tool_via_mise(project_root, KEY, ver) {
        return Ok(true);
    }
    let mut doc: toml_edit::DocumentMut = if content.is_empty() {
        "[tools]\n".parse().unwrap()
    } else {
        content.parse().context("failed to parse mise.toml")?
    };
    let tools = doc["tools"]
        .as_table_mut()
        .context("[tools] is not a table")?;
    tools.insert(KEY, toml_edit::value(ver));
    std::fs::write(&mise_path, doc.to_string())?;
    Ok(true)
}

/// Replaces obsolete tool keys in mise.toml with their modern equivalents,
/// preserving the existing version value. Returns the list of replacements made
/// as `(old_key, new_key)` pairs. No-ops if the file doesn't exist or has no
/// obsolete keys.
pub fn replace_obsolete_keys(
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

pub(super) fn apply_changes(
    path: &Path,
    current_content: &str,
    to_add: &[(String, Option<String>)],
    to_remove: &[String],
    to_upgrade: &[(String, String)],
) -> Result<()> {
    let project_root = path.parent().unwrap_or(path);

    // Pin new tools via `mise use --pin`. For tools where mise succeeds the
    // file is already updated; we still open the file below to handle removals,
    // upgrades, and component additions.
    let mut pinned_via_mise: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (key, _) in to_add {
        if pin_tool_via_mise(project_root, key, "latest") {
            pinned_via_mise.insert(key.clone());
        } else {
            eprintln!("  warning: could not pin {key} via mise — writing \"latest\"");
        }
    }

    // Re-read the file only if mise actually modified it.
    let current_content: String = if pinned_via_mise.is_empty() {
        current_content.to_string()
    } else {
        std::fs::read_to_string(path).unwrap_or_else(|_| current_content.to_string())
    };
    let mut doc: toml_edit::DocumentMut = current_content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    // Ensure [tools] table exists.
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
                // If mise already wrote a plain-string version, upgrade to inline
                // table to attach the components field.
                let existing_version = if already_pinned {
                    tools
                        .get(key.as_str())
                        .and_then(|i| i.as_value())
                        .and_then(|v| match v {
                            toml_edit::Value::String(s) => Some(s.value().to_string()),
                            toml_edit::Value::InlineTable(t) => t
                                .get("version")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            _ => None,
                        })
                        .unwrap_or_else(|| "latest".to_string())
                } else {
                    "latest".to_string()
                };
                let mut tbl = toml_edit::InlineTable::new();
                tbl.insert("version", toml_edit::Value::from(existing_version.as_str()));
                tbl.insert("components", toml_edit::Value::from(comps.as_str()));
                tools.insert(
                    key.as_str(),
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
                );
            }
            None => {
                if !already_pinned {
                    tools.insert(key.as_str(), toml_edit::value("latest"));
                }
                // Already pinned by mise — leave the entry as-is.
            }
        }
    }

    // Upgrade existing entries: preserve the current version, update components.
    for (key, components) in to_upgrade {
        let existing_version = tools
            .get(key.as_str())
            .and_then(|item| item.as_value())
            .and_then(|v| match v {
                toml_edit::Value::String(s) => Some(s.value().to_string()),
                toml_edit::Value::InlineTable(tbl) => tbl
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| "latest".to_string());

        let mut tbl = toml_edit::InlineTable::new();
        tbl.insert("version", toml_edit::Value::from(existing_version.as_str()));
        tbl.insert("components", toml_edit::Value::from(components.as_str()));
        tools.insert(
            key.as_str(),
            toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
        );
    }

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

/// Sorts `[tools]` entries and inserts the `# Linters` header when they are not
/// already in canonical form. Returns `true` if the file was rewritten.
pub(super) fn normalize_tools_section(path: &Path) -> Result<bool> {
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
    sort_and_group_tools(tools);
    let new_content = doc.to_string();
    if new_content == content {
        return Ok(false);
    }
    std::fs::write(path, new_content)?;
    println!("  normalized [tools] in {}", path.display());
    Ok(true)
}

/// Sorts `[tools]` entries alphabetically and inserts a `# Linters` comment
/// before the first linter entry. Toolchain keys (derived from registry checks
/// marked `.toolchain()`, plus `node`) stay above the header; every other key
/// goes below.
fn sort_and_group_tools(tools: &mut toml_edit::Table) {
    let mut entries: Vec<(String, toml_edit::Item)> = tools
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    if entries.is_empty() {
        return;
    }
    let toolchains = crate::registry::toolchain_keys();
    let (mut runtimes, mut linters): (Vec<_>, Vec<_>) = entries
        .drain(..)
        .partition(|(k, _)| toolchains.contains(k.as_str()));
    runtimes.sort_by(|a, b| a.0.cmp(&b.0));
    linters.sort_by(|a, b| a.0.cmp(&b.0));

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
    if let Some(k) = first_linter_key
        && let Some(mut key_mut) = tools.key_mut(&k)
    {
        key_mut.leaf_decor_mut().set_prefix("\n# Linters\n");
    }
}

const FLINT_V1_URL_PREFIX: &str = "https://raw.githubusercontent.com/grafana/flint/";

pub(super) struct V1Removal {
    /// Task keys that were removed from `[tasks]`.
    pub removed_tasks: Vec<String>,
    /// Whether `RENOVATE_TRACKED_DEPS_EXCLUDE` was removed from `[env]`.
    pub removed_renovate_env: bool,
    /// The manager list from `RENOVATE_TRACKED_DEPS_EXCLUDE`, split on commas, if it was present.
    pub renovate_exclude_managers: Option<Vec<String>>,
}

/// Removes v1 HTTP task entries (tasks whose `file` value starts with the
/// flint raw-GitHub URL) and, when a renovate-deps v1 task is present,
/// also removes `RENOVATE_TRACKED_DEPS_EXCLUDE` from `[env]`.
///
/// Returns details about what was removed. Writes the file only when changed.
pub(super) fn remove_v1_tasks(path: &Path) -> Result<V1Removal> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    let mut removed_tasks: Vec<String> = Vec::new();
    let mut has_v1_renovate = false;

    if let Some(tasks) = doc.get_mut("tasks").and_then(|t| t.as_table_mut()) {
        let keys_to_remove: Vec<String> = tasks
            .iter()
            .filter_map(|(key, item)| {
                let file_val = item
                    .as_table()
                    .and_then(|t| t.get("file"))
                    .and_then(|v| v.as_str())?;
                if file_val.starts_with(FLINT_V1_URL_PREFIX) {
                    Some(key.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            // Check if it's a renovate-deps task before removing.
            if let Some(file_val) = tasks
                .get(&key)
                .and_then(|i| i.as_table())
                .and_then(|t| t.get("file"))
                .and_then(|v| v.as_str())
                && file_val.contains("renovate-deps")
            {
                has_v1_renovate = true;
            }
            tasks.remove(&key);
            removed_tasks.push(key);
        }
    }

    let mut removed_renovate_env = false;
    let mut renovate_exclude_managers: Option<Vec<String>> = None;
    if has_v1_renovate
        && let Some(env) = doc.get_mut("env").and_then(|t| t.as_table_mut())
        && let Some(val) = env
            .get("RENOVATE_TRACKED_DEPS_EXCLUDE")
            .and_then(|v| v.as_str())
    {
        renovate_exclude_managers = Some(
            val.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        );
        env.remove("RENOVATE_TRACKED_DEPS_EXCLUDE");
        removed_renovate_env = true;
    }

    if !removed_tasks.is_empty() || removed_renovate_env {
        std::fs::write(path, doc.to_string())?;
    }

    removed_tasks.sort();
    Ok(V1Removal {
        removed_tasks,
        removed_renovate_env,
        renovate_exclude_managers,
    })
}

/// Returns true if any currently-selected check has `Category::Slow`.
pub(super) fn has_slow_selected(groups: &[LinterGroup]) -> bool {
    use crate::registry::Category;
    groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.category == Category::Slow)
    })
}

/// Reads the default branch for `origin` from git, falling back to `"main"`.
pub(super) fn detect_base_branch(project_root: &Path) -> String {
    Command::new("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().strip_prefix("origin/").map(str::to_string))
        .unwrap_or_else(|| "main".to_string())
}

/// Reads `FLINT_CONFIG_DIR` from the `[env]` section of a mise.toml string, if present.
pub(super) fn get_existing_config_dir(content: &str) -> Option<String> {
    let doc: toml_edit::DocumentMut = content.parse().ok()?;
    doc.get("env")?
        .as_table()?
        .get("FLINT_CONFIG_DIR")?
        .as_str()
        .map(str::to_string)
}

/// Asks where `flint.toml` should live. Skips the prompt when `--yes` or when
/// `FLINT_CONFIG_DIR` is already set in the current mise.toml.
///
/// Returns a path relative to the project root (e.g. `".github/config"`).
pub(super) fn prompt_config_dir(existing: Option<&str>, yes: bool) -> Result<String> {
    if let Some(dir) = existing {
        return Ok(dir.to_string());
    }
    if yes {
        return Ok(".github/config".to_string());
    }

    const CHOICES: &[&str] = &[".github/config", ".github", ".", "other…"];
    println!("Where should flint.toml live?\n");
    for (i, choice) in CHOICES.iter().enumerate() {
        println!("  {}) {}", i + 1, choice);
    }
    print!("\nChoice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim();

    let idx: usize = if input.is_empty() {
        0
    } else {
        input.parse::<usize>().unwrap_or(1).saturating_sub(1)
    };

    if idx == CHOICES.len() - 1 {
        print!("Config dir path: ");
        io::stdout().flush()?;
        let mut path = String::new();
        io::stdin().lock().read_line(&mut path)?;
        Ok(path.trim().to_string())
    } else {
        Ok(CHOICES[idx.min(CHOICES.len() - 2)].to_string())
    }
}

/// Writes a skeleton `flint.toml` in `config_dir`. Creates the directory if needed.
/// Returns `true` if the file was written, `false` if it already existed.
///
/// `exclude_managers`: when `Some`, populates `exclude_managers` in `[checks.renovate-deps]`
/// with the given list (migrated from `RENOVATE_TRACKED_DEPS_EXCLUDE`). When `None` and
/// `has_renovate` is true, writes a commented-out placeholder instead.
pub(super) fn generate_flint_toml(
    config_dir: &Path,
    base_branch: &str,
    has_renovate: bool,
    exclude_managers: Option<&[String]>,
) -> Result<bool> {
    let toml_path = config_dir.join("flint.toml");
    if toml_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(config_dir)?;
    let mut content = String::from("[settings]\n");
    if base_branch != "main" {
        content.push_str(&format!("base_branch = \"{base_branch}\"\n"));
    }
    content.push_str("# exclude = \"CHANGELOG\\\\.md\"\n");
    content.push_str("# exclude_paths = []\n");
    if has_renovate {
        content.push_str("\n[checks.renovate-deps]\n");
        match exclude_managers {
            Some(managers) if !managers.is_empty() => {
                let list = managers
                    .iter()
                    .map(|m| format!("\"{m}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                content.push_str(&format!("exclude_managers = [{list}]\n"));
            }
            _ => content.push_str("# exclude_managers = []\n"),
        }
    }
    std::fs::write(&toml_path, &content)?;
    println!("  wrote {}", toml_path.display());
    Ok(true)
}

/// Generates `.rumdl.toml` in the project root when rumdl is being set up.
/// Returns `true` if the file was written (or an older markdownlint variant was replaced).
pub(super) fn generate_rumdl_config(project_root: &Path) -> Result<bool> {
    const LEGACY_CONFIG_NAMES: &[&str] = &[
        ".markdownlint.json",
        ".markdownlint.jsonc",
        ".markdownlint.yaml",
        ".markdownlint.yml",
        ".markdownlint-cli2.jsonc",
        ".markdownlint-cli2.yaml",
        ".markdownlint-cli2.yml",
        ".markdownlint-cli2.cjs",
        ".markdownlint-cli2.mjs",
    ];
    let target = project_root.join(".rumdl.toml");
    if target.exists() {
        return Ok(false);
    }
    for name in LEGACY_CONFIG_NAMES {
        let legacy = project_root.join(name);
        if legacy.exists() {
            std::fs::remove_file(&legacy)?;
            println!("  removed {} (replaced by .rumdl.toml)", legacy.display());
        }
    }
    let content = concat!(
        "[global]\n",
        "line-length = 120\n\n",
        "[MD013]\n",
        "enabled = true\n",
        "line-length = 120\n",
        "code-blocks = false\n",
        "tables = false\n",
    );
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates `.yamllint.yml` in the project root when yaml-lint is being set up.
pub(super) fn generate_yamllint_config(project_root: &Path) -> Result<bool> {
    let target = project_root.join(".yamllint.yml");
    if target.exists() {
        return Ok(false);
    }
    let content = concat!(
        "extends: relaxed\n",
        "\n",
        "rules:\n",
        "  document-start: disable\n",
        "  line-length:\n",
        "    max: 120\n",
        "  indentation:\n",
        "    spaces: 2\n",
    );
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates `biome.json` in the project root when biome is being set up and no
/// existing biome config is present.
///
/// Flint writes explicit space indentation to avoid Biome's default tab
/// formatting surprising consumers during rollout.
pub(super) fn generate_biome_config(project_root: &Path) -> Result<bool> {
    const EXISTING_CONFIG_NAMES: &[&str] = &["biome.json", "biome.jsonc"];
    if EXISTING_CONFIG_NAMES
        .iter()
        .map(|name| project_root.join(name))
        .any(|path| path.exists())
    {
        return Ok(false);
    }

    let target = project_root.join("biome.json");
    let content = concat!(
        "{\n",
        "  \"formatter\": {\n",
        "    \"indentStyle\": \"space\",\n",
        "    \"indentWidth\": 2\n",
        "  }\n",
        "}\n"
    );
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates `.github/workflows/lint.yml` if it does not already exist.
/// Returns `true` if the file was written.
pub(super) fn generate_lint_workflow(
    project_root: &Path,
    base_branch: &str,
    has_rust: bool,
) -> Result<bool> {
    let workflows_dir = project_root.join(".github/workflows");
    let workflow_path = workflows_dir.join("lint.yml");
    if workflow_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&workflows_dir)?;
    let push_comment = if has_rust {
        " # warms the Rust cache so PR branches get a cache hit"
    } else {
        ""
    };
    let rust_steps = if has_rust {
        "\n      - uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1\n\n      - name: Install Rust lint components\n        run: rustup component add clippy rustfmt\n"
    } else {
        ""
    };
    let content = format!(
        r#"name: Lint

on:
  push:
    branches: [{base_branch}]{push_comment}
  pull_request:
    branches: [{base_branch}]

permissions:
  contents: read

jobs:
  lint:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
        with:
          persist-credentials: false
          fetch-depth: 0

      - name: Setup mise
        uses: jdx/mise-action@1648a7812b9aeae629881980618f079932869151 # v4.0.1
        with:
          version: v2026.4.18
          sha256: 6ae2d5f0f23a2f2149bc5d9bf264fe0922a1da843f1903e453516c462b23cc1f
{rust_steps}
      - name: Lint
        env:
          GITHUB_TOKEN: ${{{{ github.token }}}}
          GITHUB_HEAD_SHA: ${{{{ github.event.pull_request.head.sha }}}}
        run: mise run lint
"#
    );
    std::fs::write(&workflow_path, content)?;
    println!("  wrote {}", workflow_path.display());
    Ok(true)
}

/// Adds a `[tasks.<name>]` entry only when it is not already present.
/// Returns `true` if an entry was added.
fn add_task_if_absent(
    tasks: &mut toml_edit::Table,
    name: &str,
    description: &str,
    run: &str,
) -> bool {
    if tasks.contains_key(name) {
        return false;
    }
    write_task(tasks, name, description, run);
    true
}

/// Unconditionally writes a `[tasks.<name>]` entry (adds or replaces).
fn write_task(tasks: &mut toml_edit::Table, name: &str, description: &str, run: &str) {
    let mut t = toml_edit::Table::new();
    t.insert("description", toml_edit::value(description));
    t.insert("run", toml_edit::value(run));
    tasks.insert(name, toml_edit::Item::Table(t));
}

/// Returns `true` when the named task has a `depends` array where at least one
/// entry is in `removed_tasks`. Used to detect tasks made stale by v1 removal.
fn task_has_removed_dep(tasks: &toml_edit::Table, name: &str, removed: &[String]) -> bool {
    let Some(item) = tasks.get(name) else {
        return false;
    };
    let Some(task) = item.as_table() else {
        return false;
    };
    let Some(depends) = task.get("depends").and_then(|v| v.as_array()) else {
        return false;
    };
    depends.iter().any(|v| {
        v.as_str()
            .map(|s| removed.iter().any(|r| r == s))
            .unwrap_or(false)
    })
}

/// Adds `[env] FLINT_CONFIG_DIR` and the standard `lint*` tasks to `mise.toml`,
/// skipping any that are already present.
///
/// When `removed_v1_tasks` is non-empty, standard tasks whose `depends` reference
/// any of those removed tasks are replaced (they became stale after v1 removal).
///
/// Returns `true` if the file was changed.
pub(super) fn apply_env_and_tasks(
    mise_path: &Path,
    config_dir_rel: &str,
    has_slow: bool,
    removed_v1_tasks: &[String],
) -> Result<bool> {
    let content = std::fs::read_to_string(mise_path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());
    let mut changed = false;

    // [env] — add FLINT_CONFIG_DIR if absent
    {
        if !doc.contains_key("env") {
            doc.insert("env", toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let env = doc["env"].as_table_mut().context("[env] is not a table")?;
        if !env.contains_key("FLINT_CONFIG_DIR") {
            env.insert("FLINT_CONFIG_DIR", toml_edit::value(config_dir_rel));
            changed = true;
        }
    }

    // [tasks] — add lint / lint:fix / (lint:pre-commit)
    {
        if !doc.contains_key("tasks") {
            let mut tasks_table = toml_edit::Table::new();
            tasks_table.set_implicit(true);
            doc.insert("tasks", toml_edit::Item::Table(tasks_table));
        }
        let tasks = doc["tasks"]
            .as_table_mut()
            .context("[tasks] is not a table")?;

        // Replace the lint task when it was made stale by v1 removal (its depends
        // referenced removed tasks and would now fail). Otherwise add if absent.
        let lint_stale = task_has_removed_dep(tasks, "lint", removed_v1_tasks);
        if lint_stale {
            write_task(tasks, "lint", "Run all lints", "flint run");
            changed = true;
        } else {
            changed |= add_task_if_absent(tasks, "lint", "Run all lints", "flint run");
        }

        changed |= add_task_if_absent(tasks, "lint:fix", "Auto-fix lint issues", "flint run --fix");
        if has_slow {
            changed |= add_task_if_absent(
                tasks,
                "lint:pre-commit",
                "Fast auto-fix lint (skips slow checks) — for pre-commit/pre-push hooks",
                "flint run --fix --fast-only",
            );
        }
    }

    if changed {
        std::fs::write(mise_path, doc.to_string())?;
    }
    Ok(changed)
}

/// Offers to install the git pre-commit hook via `flint hook install`.
/// Prompts the user unless `yes` is true. Silently skips if the hook is already installed.
pub(super) fn maybe_install_hook(project_root: &Path, yes: bool) -> Result<()> {
    let hook_path = project_root.join(".git/hooks/pre-commit");
    if hook_path.exists() {
        return Ok(());
    }

    let install = if yes {
        true
    } else {
        print!(
            "Install pre-commit hook (runs `flint run --fix --fast-only` before each commit)? [Y/n] "
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    };

    if install {
        crate::hook::install(project_root)?;
    }
    Ok(())
}

#[cfg(test)]
mod node_prereq_tests {
    use super::{ensure_node_for_npm, needs_node_for_npm};

    // Full end-to-end ensure_node_for_npm coverage (npm-present, node-absent →
    // node added, file modified) lives in the e2e case `general/update-adds-node`.
    // That case drives the flint binary and exercises the real mise subprocess.

    #[test]
    fn needs_node_when_npm_key_without_node() {
        let content = "[tools]\n\"npm:renovate\" = \"43.129.0\"\n";
        assert!(needs_node_for_npm(content));
    }

    #[test]
    fn no_node_needed_when_no_npm_keys() {
        let content = "[tools]\nshellcheck = \"v0.11.0\"\n";
        assert!(!needs_node_for_npm(content));
    }

    #[test]
    fn no_node_needed_when_node_already_declared() {
        let content = "[tools]\nnode = \"20\"\n\"npm:renovate\" = \"43.129.0\"\n";
        assert!(!needs_node_for_npm(content));
    }

    #[test]
    fn no_node_needed_when_tools_section_missing() {
        assert!(!needs_node_for_npm(""));
        assert!(!needs_node_for_npm("[env]\nFOO = \"bar\"\n"));
    }

    #[test]
    fn noop_when_node_already_present() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        let original = "[tools]\nnode = \"20\"\n\"npm:renovate\" = \"43.129.0\"\n";
        std::fs::write(&path, original).unwrap();
        let added = ensure_node_for_npm(dir.path()).unwrap();
        assert!(!added);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
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
}

#[cfg(test)]
mod replace_obsolete_tests {
    use super::replace_obsolete_keys;

    #[test]
    fn replaces_old_key_preserving_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\n\"github:mvdan/sh\" = \"v3.13.1\"\n").unwrap();
        let replaced = replace_obsolete_keys(dir.path(), &[("github:mvdan/sh", "shfmt")]).unwrap();
        assert_eq!(
            replaced,
            vec![("github:mvdan/sh".to_string(), "shfmt".to_string())]
        );
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("shfmt"), "new key written: {result}");
        assert!(
            !result.contains("\"github:mvdan/sh\""),
            "old key removed: {result}"
        );
        assert!(result.contains("v3.13.1"), "version preserved: {result}");
    }

    #[test]
    fn noop_when_no_obsolete_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mise.toml");
        std::fs::write(&path, "[tools]\nshfmt = \"v3.13.1\"\n").unwrap();
        let replaced = replace_obsolete_keys(dir.path(), &[("github:mvdan/sh", "shfmt")]).unwrap();
        assert!(replaced.is_empty());
    }
}

#[cfg(test)]
mod v1_removal_tests {
    use super::remove_v1_tasks;

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), content).unwrap();
        f
    }

    #[test]
    fn removes_v1_http_tasks() {
        let content = r#"
[tools]
lychee = "latest"

[tasks."lint:links"]
description = "Check for broken links"
file = "https://raw.githubusercontent.com/grafana/flint/abc123/tasks/lint/links.sh"

[tasks."lint:renovate-deps"]
description = "Check renovate deps"
file = "https://raw.githubusercontent.com/grafana/flint/abc123/tasks/lint/renovate-deps.py"

[tasks.build]
description = "Build the project"
run = "cargo build"
"#;
        let tmp = write_tmp(content);
        let result = remove_v1_tasks(tmp.path()).unwrap();
        assert_eq!(result.removed_tasks, ["lint:links", "lint:renovate-deps"]);
        assert!(!result.removed_renovate_env);
        let after = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!after.contains("lint:links"));
        assert!(!after.contains("lint:renovate-deps"));
        assert!(after.contains("[tasks.build]"), "non-v1 tasks preserved");
    }

    #[test]
    fn removes_renovate_env_when_v1_renovate_task_present() {
        let content = r#"
[env]
RENOVATE_TRACKED_DEPS_EXCLUDE = "github-actions, github-runners"

[tasks."lint:renovate-deps"]
description = "Check renovate deps"
file = "https://raw.githubusercontent.com/grafana/flint/abc123/tasks/lint/renovate-deps.py"
"#;
        let tmp = write_tmp(content);
        let result = remove_v1_tasks(tmp.path()).unwrap();
        assert_eq!(result.removed_tasks, ["lint:renovate-deps"]);
        assert!(result.removed_renovate_env);
        assert_eq!(
            result.renovate_exclude_managers,
            Some(vec![
                "github-actions".to_string(),
                "github-runners".to_string()
            ])
        );
        let after = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!after.contains("RENOVATE_TRACKED_DEPS_EXCLUDE"));
    }

    #[test]
    fn does_not_remove_renovate_env_without_v1_renovate_task() {
        let content = r#"
[env]
RENOVATE_TRACKED_DEPS_EXCLUDE = "github-actions"

[tasks."lint:links"]
description = "Check links"
file = "https://raw.githubusercontent.com/grafana/flint/abc123/tasks/lint/links.sh"
"#;
        let tmp = write_tmp(content);
        let result = remove_v1_tasks(tmp.path()).unwrap();
        assert_eq!(result.removed_tasks, ["lint:links"]);
        assert!(!result.removed_renovate_env);
        let after = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(
            after.contains("RENOVATE_TRACKED_DEPS_EXCLUDE"),
            "env var preserved when no renovate task"
        );
    }

    #[test]
    fn no_op_when_no_v1_tasks() {
        let content = "[tools]\nlychee = \"latest\"\n";
        let tmp = write_tmp(content);
        let original_mtime = std::fs::metadata(tmp.path()).unwrap().modified().unwrap();
        let result = remove_v1_tasks(tmp.path()).unwrap();
        assert!(result.removed_tasks.is_empty());
        assert!(!result.removed_renovate_env);
        // File should not have been written.
        let new_mtime = std::fs::metadata(tmp.path()).unwrap().modified().unwrap();
        assert_eq!(
            original_mtime, new_mtime,
            "file unchanged when nothing to remove"
        );
    }

    #[test]
    fn ignores_non_flint_http_tasks() {
        let content = r#"
[tasks."lint:something"]
file = "https://raw.githubusercontent.com/some-other-org/some-repo/abc123/task.sh"
"#;
        let tmp = write_tmp(content);
        let result = remove_v1_tasks(tmp.path()).unwrap();
        assert!(result.removed_tasks.is_empty());
    }
}

#[cfg(test)]
mod extends_tests {
    use super::{add_to_extends, patch_renovate_extends};

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), content).unwrap();
        f
    }

    #[test]
    fn replaces_unpinned_flint_entry_in_place() {
        let input = r#"{ extends: ["config:recommended", "github>grafana/flint"] }"#;
        let tmp = write_tmp(input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(
            result.contains("github>grafana/flint#v"),
            "pinned entry written: {result}"
        );
        // Only one flint entry — no duplicate
        assert_eq!(
            result.matches("grafana/flint").count(),
            1,
            "no duplicate: {result}"
        );
        assert!(
            !result.contains("\"github>grafana/flint\""),
            "unpinned removed: {result}"
        );
    }

    #[test]
    fn replaces_differently_pinned_flint_entry() {
        let input = r#"{ extends: ["config:recommended", "github>grafana/flint#v0.5.0"] }"#;
        let tmp = write_tmp(input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!result.contains("v0.5.0"), "old pin removed: {result}");
        assert_eq!(
            result.matches("grafana/flint").count(),
            1,
            "no duplicate: {result}"
        );
    }

    #[test]
    fn no_op_when_already_pinned_to_current_version() {
        let entry = super::flint_preset();
        let input = format!(r#"{{ extends: ["config:recommended", "{entry}"] }}"#);
        let tmp = write_tmp(&input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(!changed);
    }

    #[test]
    fn adds_to_single_line_extends() {
        let input = r#"{ "extends": ["config:recommended"], "other": 1 }"#;
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#"["github>grafana/flint#v0.9.2", "config:recommended"]"#));
    }

    #[test]
    fn adds_to_json5_unquoted_key() {
        let input = "{\n  extends: [\"config:recommended\"],\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#""github>grafana/flint#v0.9.2", "config:recommended""#));
    }

    #[test]
    fn adds_to_multiline_extends() {
        let input = "{\n  extends: [\n    \"config:recommended\",\n    \"other\"\n  ]\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains("\"github>grafana/flint#v0.9.2\","));
        // Entry should appear before existing entries
        let flint_pos = result.find("grafana/flint").unwrap();
        let existing_pos = result.find("config:recommended").unwrap();
        assert!(flint_pos < existing_pos);
    }

    #[test]
    fn adds_extends_when_absent() {
        let input = "{\n  \"branchPrefix\": \"renovate/\"\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains("\"extends\""));
        assert!(result.contains("github>grafana/flint#v0.9.2"));
    }

    #[test]
    fn adds_to_empty_extends_array() {
        let input = r#"{ "extends": [] }"#;
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#"["github>grafana/flint#v0.9.2"]"#));
    }
}
