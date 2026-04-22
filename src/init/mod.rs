use anyhow::Result;
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::registry::{Category, Check, builtin};

mod detection;
pub(crate) mod generation;
mod ui;

use detection::{
    build_linter_groups, detect_obsolete_keys, detect_present_patterns, parse_tool_keys,
};
use generation::{
    apply_changes, apply_env_and_tasks, detect_base_branch, ensure_flint_self_pin,
    ensure_node_for_npm, flint_preset, generate_biome_config, generate_flint_toml,
    generate_lint_workflow, generate_markdownlint_config, get_existing_config_dir,
    has_slow_selected, maybe_install_hook, normalize_tools_section, patch_renovate_extends,
    prompt_config_dir, remove_v1_tasks,
};
use ui::{interactive_select_linters, select_categories_arrow};

/// Linter profile — shorthand for `--profile` CLI flag; maps to a category set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Primary language linters only (ruff, cargo-clippy, golangci-lint, …).
    Lang,
    /// Lang + supplementary checks + fast general tools (shellcheck, prettier, codespell, …).
    Default,
    /// Default + slow linters (renovate-deps).
    Comprehensive,
}

fn profile_to_categories(profile: Profile) -> HashSet<Category> {
    match profile {
        Profile::Lang => [Category::Lang].into(),
        Profile::Default => [Category::Lang, Category::Style, Category::Default].into(),
        Profile::Comprehensive => [
            Category::Lang,
            Category::Style,
            Category::Default,
            Category::Slow,
        ]
        .into(),
    }
}

/// Desired tools for a profile: maps each mise tool key to its optional components string.
#[cfg(test)]
type DesiredTools = HashMap<String, Option<String>>;

// One entry per install key — groups all checks sharing that key.
struct LinterGroup<'a> {
    key: &'static str,
    checks: Vec<&'a Check>,    // sorted by name
    check_selected: Vec<bool>, // parallel to checks
    installed: bool,
    current_components: Option<String>,
}

impl LinterGroup<'_> {
    fn any_selected(&self) -> bool {
        self.check_selected.iter().any(|&s| s)
    }

    /// Components string to write for the currently selected checks, e.g. `"clippy,rustfmt"`.
    /// Returns `None` when no selected check carries a component requirement.
    fn selected_components(&self) -> Option<String> {
        let comps: Vec<&'static str> = self
            .checks
            .iter()
            .zip(&self.check_selected)
            .filter_map(|(c, &sel)| if sel { c.components() } else { None })
            .collect();
        if comps.is_empty() {
            None
        } else {
            Some(comps.join(","))
        }
    }

    fn action(&self) -> &'static str {
        if self.any_selected() {
            if !self.installed {
                "add"
            } else if self.selected_components() != self.current_components {
                "upgrade"
            } else {
                "keep"
            }
        } else if self.installed {
            "remove"
        } else {
            ""
        }
    }
}

// --- Category selection (step 1) ---

struct CategoryItem {
    selected: bool,
    category: Category,
    label: &'static str,
}

fn default_category_items() -> Vec<CategoryItem> {
    vec![
        CategoryItem {
            selected: true,
            category: Category::Lang,
            label: "lang    — primary language linters (ruff, cargo-clippy, golangci-lint, …)",
        },
        CategoryItem {
            selected: true,
            category: Category::Style,
            label: "style   — supplementary checks (shellcheck, actionlint, hadolint, …)",
        },
        CategoryItem {
            selected: true,
            category: Category::Default,
            label: "general — general tools (codespell, ec, lychee, …)",
        },
        CategoryItem {
            selected: false,
            category: Category::Slow,
            label: "slow    — slow linters (renovate-deps)",
        },
    ]
}

pub fn run(project_root: &Path, profile_arg: Option<Profile>, yes: bool) -> Result<()> {
    println!(
        "Tip: flint init detects languages from tracked files (`git ls-files`). \
Add and stage your source files before running init so the detection is accurate."
    );
    println!();

    let registry = builtin();
    let mut present_patterns = detect_present_patterns(project_root, &registry)?;

    // If init will generate `.github/workflows/lint.yml`, treat the workflow
    // patterns as present so actionlint gets selected in the same run.
    // Without this, init would be non-idempotent: the second run would see the
    // newly-generated workflow and add actionlint then.
    if !project_root.join(".github/workflows/lint.yml").exists() {
        present_patterns.insert(".github/workflows/*.yml".to_string());
        present_patterns.insert(".github/workflows/*.yaml".to_string());
    }

    // Step 1: determine which categories set the initial pre-selection.
    let default_categories: HashSet<Category> = if let Some(profile) = profile_arg {
        profile_to_categories(profile)
    } else if yes {
        profile_to_categories(Profile::Default)
    } else {
        let mut cat_items = default_category_items();
        if !select_categories_arrow(&mut cat_items)? {
            println!("Aborted.");
            return Ok(());
        }
        cat_items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.category)
            .collect()
    };

    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);
    let known_keys: HashSet<&str> = registry.iter().filter_map(install_key).collect();

    // Step 2: build one group per install key, covering all checks whose files are
    // present in the repo or which are already installed.
    let mut groups = build_linter_groups(
        &registry,
        &present_patterns,
        &current_tool_keys,
        &current_content,
        &default_categories,
    );

    if groups.is_empty() {
        println!("No applicable linters found for this project.");
        return Ok(());
    }

    // Step 3: interactive linter table (skipped with --yes).
    if !yes && !interactive_select_linters(&mut groups)? {
        println!("Aborted.");
        return Ok(());
    }

    // Detect obsolete tool keys (e.g. npm:markdownlint-cli → npm:markdownlint-cli2).
    // These are removed regardless of the interactive selection — keeping them serves no purpose.
    let obsolete = detect_obsolete_keys(&current_tool_keys);
    for (old_key, replacement) in &obsolete {
        println!("  removing obsolete linter {old_key} (replaced by {replacement})");
    }

    // Derive changes from final selection state.
    let mut final_add: Vec<(String, Option<String>)> = Vec::new();
    let mut final_remove: Vec<String> = Vec::new();
    let mut final_upgrade: Vec<(String, String)> = Vec::new();

    for group in &groups {
        if group.any_selected() {
            if !group.installed {
                final_add.push((group.key.to_string(), group.selected_components()));
            } else {
                let target = group.selected_components();
                if target != group.current_components {
                    // Upgrade: components changed (added, removed, or reordered).
                    // If the target has no components (e.g. all component-bearing checks
                    // deselected), treat as a plain-version install via add+remove.
                    if let Some(comps) = target {
                        final_upgrade.push((group.key.to_string(), comps));
                    }
                }
            }
        } else if group.installed && known_keys.contains(group.key) {
            final_remove.push(group.key.to_string());
        }
    }

    // Always remove obsolete tool keys (detected before the interactive selection).
    for (old_key, _) in &obsolete {
        final_remove.push(old_key.to_string());
    }

    let has_slow = has_slow_selected(&groups);
    let has_renovate = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "renovate-deps")
    });
    let has_markdownlint = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "markdownlint-cli2")
    });
    let has_editorconfig_checker = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "editorconfig-checker")
    });
    let has_biome = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && (c.name == "biome" || c.name == "biome-format"))
    });

    // Prompt for the flint config dir (skipped if already set in mise.toml or --yes).
    let existing_config_dir = get_existing_config_dir(&current_content);
    let config_dir_rel = prompt_config_dir(existing_config_dir.as_deref(), yes)?;

    let tools_changed =
        !final_add.is_empty() || !final_remove.is_empty() || !final_upgrade.is_empty();
    if tools_changed {
        apply_changes(
            &mise_path,
            &current_content,
            &final_add,
            &final_remove,
            &final_upgrade,
        )?;
    }
    let node_added = ensure_node_for_npm(project_root)?;
    if node_added {
        println!("  added node (LTS) — required by npm: backend tools");
    }
    let flint_pinned = ensure_flint_self_pin(project_root)?;
    if flint_pinned {
        println!("  pinned flint itself — reproducible lint runs across contributors");
    }
    let tools_normalized = normalize_tools_section(&mise_path)?;

    let v1 = remove_v1_tasks(&mise_path)?;
    for key in &v1.removed_tasks {
        println!("  removing v1 task {key}");
    }
    if v1.removed_renovate_env {
        println!("  removing RENOVATE_TRACKED_DEPS_EXCLUDE from [env] (use flint.toml instead)");
    }

    let meta_changed =
        apply_env_and_tasks(&mise_path, &config_dir_rel, has_slow, &v1.removed_tasks)?;

    let base_branch = detect_base_branch(project_root);
    let config_dir_path = project_root.join(&config_dir_rel);
    let toml_generated = generate_flint_toml(
        &config_dir_path,
        &base_branch,
        has_renovate,
        v1.renovate_exclude_managers.as_deref(),
    )?;
    let has_rust = final_add.iter().any(|(k, _)| k == "rust")
        || (current_tool_keys.contains("rust") && !final_remove.iter().any(|k| k == "rust"));
    let workflow_generated = generate_lint_workflow(project_root, &base_branch, has_rust)?;
    let markdownlint_generated = if has_markdownlint && has_editorconfig_checker {
        generate_markdownlint_config(project_root)?
    } else {
        false
    };
    let biome_generated = if has_biome {
        generate_biome_config(project_root)?
    } else {
        false
    };

    let renovate_patched = find_renovate_config(project_root)
        .map(|path| {
            let result = patch_renovate_extends(&path);
            if let Ok(true) = result {
                let rel = path.strip_prefix(project_root).unwrap_or(&path);
                println!("  patched {} — added {}", rel.display(), flint_preset());
            }
            result
        })
        .transpose()?
        .unwrap_or(false);

    if !tools_changed
        && !node_added
        && !flint_pinned
        && !tools_normalized
        && v1.removed_tasks.is_empty()
        && !v1.removed_renovate_env
        && !meta_changed
        && !toml_generated
        && !workflow_generated
        && !markdownlint_generated
        && !biome_generated
        && !renovate_patched
    {
        println!("No changes to apply.");
        return Ok(());
    }

    maybe_install_hook(project_root, yes)?;

    println!("Done. Run `mise install` to install the new tools.");
    Ok(())
}

fn find_renovate_config(project_root: &Path) -> Option<std::path::PathBuf> {
    crate::linters::renovate_deps::RENOVATE_CONFIG_PATTERNS
        .iter()
        .map(|p| project_root.join(p))
        .find(|p| p.exists())
}

/// Returns the canonical mise.toml tool key to write when installing this check
/// via `flint init`, or `None` if no mise entry is needed (built-in or
/// unconditionally active checks).
///
/// Preference order: `mise_install_key` → `mise_tool_name` → `bin_name`.
pub fn install_key(check: &Check) -> Option<&'static str> {
    if !check.uses_binary() || check.activate_unconditionally {
        return None;
    }
    Some(check.mise_tool_name.unwrap_or(check.bin_name))
}

/// Compute the map of `tool_key → optional_components` for the given category set,
/// filtered to file patterns present in the repo.
#[cfg(test)]
fn compute_desired_tools(
    registry: &[Check],
    present_patterns: &HashSet<String>,
    categories: &HashSet<Category>,
) -> DesiredTools {
    use detection::files_present;

    // Collect per-key component lists so multiple checks sharing a key are merged.
    let mut by_key: HashMap<String, Vec<&'static str>> = HashMap::new();
    for check in registry {
        let key = match install_key(check) {
            Some(k) => k,
            None => continue,
        };
        if !files_present(check, present_patterns) {
            continue;
        }
        if categories.contains(&check.category) {
            let entry = by_key.entry(key.to_string()).or_default();
            if let Some(comp) = check.components()
                && !entry.contains(&comp)
            {
                entry.push(comp);
            }
        }
    }
    by_key
        .into_iter()
        .map(|(k, comps)| {
            let merged = if comps.is_empty() {
                None
            } else {
                Some(comps.join(","))
            };
            (k, merged)
        })
        .collect()
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use detection::entry_components_differ;
    use generation::{
        apply_changes, apply_env_and_tasks, generate_flint_toml, generate_lint_workflow,
        get_existing_config_dir, has_slow_selected, normalize_tools_section,
    };

    #[test]
    fn detect_obsolete_keys_finds_known_stale_key() {
        use detection::detect_obsolete_keys;
        let mut keys = HashSet::new();
        keys.insert("npm:markdownlint-cli".to_string());
        keys.insert("shellcheck".to_string());
        let found = detect_obsolete_keys(&keys);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "npm:markdownlint-cli");
        assert_eq!(found[0].1, "npm:markdownlint-cli2");
    }

    #[test]
    fn detect_obsolete_keys_ignores_current_keys() {
        use detection::detect_obsolete_keys;
        let mut keys = HashSet::new();
        keys.insert("npm:markdownlint-cli2".to_string());
        keys.insert("shellcheck".to_string());
        let found = detect_obsolete_keys(&keys);
        assert!(found.is_empty());
    }

    #[test]
    fn all_registry_checks_have_install_key_or_none() {
        // Every check that uses a binary and isn't unconditional must have a resolvable key.
        for check in builtin() {
            if check.uses_binary() && !check.activate_unconditionally {
                let key = install_key(&check);
                assert!(
                    key.is_some(),
                    "check '{}' is missing an install key",
                    check.name
                );
            }
        }
    }

    #[test]
    fn entry_components_differ_string_value() {
        let content = "[tools]\nrust = \"1.80.0\"\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_without_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\" }\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_wrong_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy\" }\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_correct_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy,rustfmt\" }\n";
        assert!(!entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn normalize_tools_section_sorts_and_inserts_linters_header() {
        let content = r#"[tools]
lychee = "0.22.0"
node = "24.0.0"
actionlint = "1.7.0"
"npm:prettier" = "3.8.0"
rust = { version = "1.95.0", components = "clippy,rustfmt" }
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let changed = normalize_tools_section(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        let header_pos = result.find("# Linters").expect("header present");
        let node_pos = result.find("node =").expect("node present");
        let rust_pos = result.find("rust =").expect("rust present");
        let actionlint_pos = result.find("actionlint =").expect("actionlint present");
        let lychee_pos = result.find("lychee =").expect("lychee present");
        let prettier_pos = result.find("\"npm:prettier\"").expect("prettier present");
        // rust is the only true toolchain here; node lives with linters because
        // it's only pinned as a prereq for npm:* backend tools.
        assert!(rust_pos < header_pos, "toolchains above header");
        assert!(node_pos > header_pos, "node below header (linter prereq)");
        assert!(actionlint_pos > header_pos, "linters below header");
        assert!(
            actionlint_pos < lychee_pos && lychee_pos < node_pos && node_pos < prettier_pos,
            "linters sorted alphabetically"
        );

        // Idempotent: second call returns false and leaves content unchanged.
        let changed_again = normalize_tools_section(tmp.path()).unwrap();
        assert!(!changed_again);
        let result_again = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(result, result_again);
    }

    #[test]
    fn apply_changes_upgrade_preserves_version() {
        let content = "[tools]\nrust = \"1.80.0\"\n";
        let tmp = tempfile::NamedTempFile::new().unwrap();
        apply_changes(
            tmp.path(),
            content,
            &[],
            &[],
            &[("rust".to_string(), "clippy,rustfmt".to_string())],
        )
        .unwrap();
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(result.contains("version = \"1.80.0\""), "version preserved");
        assert!(
            result.contains("components = \"clippy,rustfmt\""),
            "components added"
        );
    }

    #[test]
    fn parse_tool_keys_reads_simple_toml() {
        let content = r#"
[tools]
shellcheck = "v0.11.0"
"npm:prettier" = "3.8.1"
rust = { version = "1.0", components = "clippy" }
"#;
        let keys = parse_tool_keys(content);
        assert!(keys.contains("shellcheck"));
        assert!(keys.contains("npm:prettier"));
        assert!(keys.contains("rust"));
        assert!(!keys.contains("nonexistent"));
    }

    #[test]
    fn compute_desired_tools_lang_profile() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert("*.sh".to_string());
        present.insert("*.bash".to_string());
        present.insert("*.rs".to_string());
        let categories = profile_to_categories(Profile::Lang);
        let tools = compute_desired_tools(&registry, &present, &categories);
        // Shell checks are supplementary (Style), not included in the lang profile.
        assert!(!tools.contains_key("shellcheck"));
        assert!(!tools.contains_key("shfmt"));
        // Primary language linters are included.
        assert!(tools.contains_key("rust"));
        // General tools are not lang-only.
        assert!(!tools.contains_key("pipx:codespell"));
    }

    #[test]
    fn rust_install_entry_has_components() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert("*.rs".to_string());
        let categories = profile_to_categories(Profile::Lang);
        let tools = compute_desired_tools(&registry, &present, &categories);
        // Both cargo-clippy and cargo-fmt share the "rust" key; their components are merged.
        assert_eq!(
            tools.get("rust"),
            Some(&Some("clippy,rustfmt".to_string())),
            "rust tool entry should carry merged components"
        );
    }

    #[test]
    fn compute_desired_tools_default_excludes_slow() {
        let registry = builtin();
        let present: HashSet<String> = HashSet::new();
        let categories = profile_to_categories(Profile::Default);
        let tools = compute_desired_tools(&registry, &present, &categories);
        // renovate-deps is slow — should be absent
        assert!(!tools.contains_key("npm:renovate"));
        // lychee is fast — should be present (empty patterns → always present)
        assert!(tools.contains_key("lychee"));
    }

    #[test]
    fn compute_desired_tools_comprehensive_includes_slow() {
        let registry = builtin();
        // Must include renovate config pattern so renovate-deps is considered present.
        let mut present: HashSet<String> = HashSet::new();
        present.insert(".github/renovate.json5".to_string());
        let categories = profile_to_categories(Profile::Comprehensive);
        let tools = compute_desired_tools(&registry, &present, &categories);
        assert!(tools.contains_key("lychee"));
        assert!(tools.contains_key("npm:renovate"));
    }

    #[test]
    fn renovate_deps_absent_without_renovate_config() {
        let registry = builtin();
        // No renovate config file in present patterns → renovate-deps should be excluded.
        let present: HashSet<String> = HashSet::new();
        let categories = profile_to_categories(Profile::Comprehensive);
        let tools = compute_desired_tools(&registry, &present, &categories);
        assert!(!tools.contains_key("npm:renovate"));
    }

    #[test]
    fn has_slow_selected_false_for_default_profile() {
        let registry = builtin();
        let present = HashSet::new();
        let categories = profile_to_categories(Profile::Default);
        let groups = build_linter_groups(&registry, &present, &HashSet::new(), "", &categories);
        assert!(!has_slow_selected(&groups));
    }

    #[test]
    fn get_existing_config_dir_reads_env_section() {
        let content = "[env]\nFLINT_CONFIG_DIR = \".github/config\"\n";
        assert_eq!(
            get_existing_config_dir(content),
            Some(".github/config".to_string())
        );
    }

    #[test]
    fn get_existing_config_dir_absent() {
        let content = "[tools]\nrust = \"latest\"\n";
        assert_eq!(get_existing_config_dir(content), None);
    }

    #[test]
    fn generate_markdownlint_config_writes_file() {
        use generation::generate_markdownlint_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_markdownlint_config(tmp.path()).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join(".markdownlint.yml")).unwrap();
        assert!(content.contains("MD013: false"));
        assert!(content.contains("editorconfig-checker"));
    }

    #[test]
    fn generate_markdownlint_config_skips_when_target_exists() {
        use generation::generate_markdownlint_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".markdownlint.yml"), "existing").unwrap();
        let written = generate_markdownlint_config(tmp.path()).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join(".markdownlint.yml")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_markdownlint_config_replaces_legacy_json() {
        use generation::generate_markdownlint_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".markdownlint.json"), r#"{"MD013":false}"#).unwrap();
        let written = generate_markdownlint_config(tmp.path()).unwrap();
        assert!(written);
        assert!(!tmp.path().join(".markdownlint.json").exists());
        let content = std::fs::read_to_string(tmp.path().join(".markdownlint.yml")).unwrap();
        assert!(content.contains("MD013: false"));
    }

    #[test]
    fn generate_biome_config_writes_file() {
        use generation::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join("biome.json")).unwrap();
        assert!(content.contains("\"indentStyle\": \"space\""));
        assert!(content.contains("\"indentWidth\": 2"));
    }

    #[test]
    fn generate_biome_config_skips_existing_json() {
        use generation::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("biome.json"), "existing").unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join("biome.json")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_biome_config_skips_existing_jsonc() {
        use generation::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("biome.jsonc"), "existing").unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(!written);
        assert!(!tmp.path().join("biome.json").exists());
    }

    #[test]
    fn generate_flint_toml_writes_skeleton() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("config");
        let written = generate_flint_toml(&dir, "main", false, None).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(dir.join("flint.toml")).unwrap();
        assert!(content.contains("[settings]"));
        assert!(content.contains("# exclude ="));
        assert!(content.contains("# exclude_paths ="));
        assert!(!content.contains("base_branch")); // "main" is the default, omitted
    }

    #[test]
    fn generate_flint_toml_non_main_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_flint_toml(tmp.path(), "master", false, None).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert!(content.contains("base_branch = \"master\""));
    }

    #[test]
    fn generate_flint_toml_with_renovate_placeholder() {
        let tmp = tempfile::TempDir::new().unwrap();
        generate_flint_toml(tmp.path(), "main", true, None).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert!(content.contains("[checks.renovate-deps]"));
        assert!(content.contains("# exclude_managers ="));
    }

    #[test]
    fn generate_flint_toml_with_renovate_managers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let managers = vec!["github-actions".to_string(), "cargo".to_string()];
        generate_flint_toml(tmp.path(), "main", true, Some(&managers)).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert!(content.contains("[checks.renovate-deps]"));
        assert!(
            content.contains("exclude_managers = [\"github-actions\", \"cargo\"]"),
            "managers written uncommented: {content}"
        );
        assert!(!content.contains("# exclude_managers"));
    }

    #[test]
    fn generate_flint_toml_skips_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "existing content").unwrap();
        let written = generate_flint_toml(tmp.path(), "main", false, None).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn generate_lint_workflow_writes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_lint_workflow(tmp.path(), "main", false).unwrap();
        assert!(written);
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert!(content.contains("branches: [main]"));
        assert!(content.contains("mise run lint"));
        assert!(content.contains("fetch-depth: 0"));
        assert!(content.contains("persist-credentials: false"));
        assert!(content.contains("mise-action"));
        assert!(content.contains("github.token"));
        assert!(!content.contains("rust-cache"));
        assert!(!content.contains("rustup component"));
    }

    #[test]
    fn generate_lint_workflow_non_main_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        generate_lint_workflow(tmp.path(), "master", false).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert!(content.contains("branches: [master]"));
    }

    #[test]
    fn generate_lint_workflow_skips_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        std::fs::write(
            tmp.path().join(".github/workflows/lint.yml"),
            "existing content",
        )
        .unwrap();
        let written = generate_lint_workflow(tmp.path(), "main", false).unwrap();
        assert!(!written);
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn generate_lint_workflow_with_rust() {
        let tmp = tempfile::TempDir::new().unwrap();
        generate_lint_workflow(tmp.path(), "main", true).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert!(content.contains("Swatinem/rust-cache"));
        assert!(content.contains("rustup component add clippy rustfmt"));
        assert!(content.contains("warms the Rust cache"));
    }

    #[test]
    fn apply_env_and_tasks_adds_sections() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[tools]\nrust = \"latest\"\n").unwrap();
        let changed = apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
        assert!(changed);
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("FLINT_CONFIG_DIR = \".github/config\""));
        assert!(content.contains("flint run"));
        assert!(content.contains("flint run --fix"));
        assert!(!content.contains("--fast-only")); // no slow linters
    }

    #[test]
    fn apply_env_and_tasks_adds_pre_commit_task_when_slow() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        apply_env_and_tasks(tmp.path(), ".", true, &[]).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("--fast-only"));
        assert!(content.contains("lint:pre-commit"));
    }

    #[test]
    fn apply_env_and_tasks_idempotent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
        let after_first = std::fs::read_to_string(tmp.path()).unwrap();
        let changed = apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
        assert!(!changed);
        let after_second = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(after_first, after_second);
    }

    #[test]
    fn apply_env_and_tasks_replaces_stale_lint_task() {
        let content = r#"
[tasks."lint"]
description = "Run all lints"
depends = ["lint:fast", "lint:renovate-deps"]
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let removed = vec!["lint:renovate-deps".to_string()];
        apply_env_and_tasks(tmp.path(), ".github/config", false, &removed).unwrap();
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(
            result.contains("run = \"flint run\""),
            "stale lint task replaced: {result}"
        );
        assert!(
            !result.contains("depends"),
            "old depends array removed: {result}"
        );
    }
}
