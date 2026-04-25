use anyhow::Result;
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::registry::{Category, Check, EditorconfigLineLengthPolicy, builtin};

mod config_files;
mod detection;
pub(crate) mod generation;
mod migrations;
mod scaffold;
mod ui;

use config_files::{
    disable_editorconfig_line_length_for_patterns, generate_biome_config, generate_editorconfig,
    generate_flint_toml, generate_rumdl_config, generate_rustfmt_config, generate_taplo_config,
    generate_yamllint_config,
};
use detection::{
    build_linter_groups, detect_obsolete_keys, detect_present_patterns, parse_tool_keys,
};
use generation::{
    apply_changes, detect_base_branch, ensure_flint_self_pin, ensure_node_for_npm, flint_preset,
    get_existing_config_dir, has_slow_selected, normalize_tools_section, patch_renovate_extends,
    prompt_config_dir, remove_tool_keys, remove_v1_tasks,
};
use migrations::{active_editorconfig_line_length_sections, apply_repo_migrations};
use scaffold::{apply_env_and_tasks, generate_lint_workflow, maybe_install_hook};
use ui::{interactive_select_linters, select_categories_arrow};

const DEFAULT_LINE_LENGTH: u16 = 120;

/// Linter profile — shorthand for `--profile` CLI flag; maps to a category set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Primary language linters only (ruff, cargo-clippy, golangci-lint, …).
    Lang,
    /// Lang + supplementary checks + fast general tools (shellcheck, rumdl, codespell, …).
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

fn selected_editorconfig_line_length_sections(
    groups: &[LinterGroup<'_>],
) -> Vec<(&'static [&'static str], &'static str)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for group in groups {
        for (check, selected) in group.checks.iter().zip(&group.check_selected) {
            if !selected {
                continue;
            }
            let EditorconfigLineLengthPolicy::DisableForPatterns { patterns, comment } =
                check.editorconfig_line_length_policy
            else {
                continue;
            };
            let key = patterns.join(",");
            if seen.insert(key) {
                out.push((patterns, comment));
            }
        }
    }
    out
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

    // If init will generate `.github/workflows/lint.yml`, treat both the workflow-
    // specific patterns and generic YAML patterns as present so actionlint and
    // yaml-lint get selected in the same run. Without this, init would be
    // non-idempotent: the second run would see the newly-generated workflow and
    // add extra linters then.
    if !project_root.join(".github/workflows/lint.yml").exists() {
        present_patterns.insert(".github/workflows/*.yml".to_string());
        present_patterns.insert(".github/workflows/*.yaml".to_string());
        present_patterns.insert("*.yml".to_string());
        present_patterns.insert("*.yaml".to_string());
    }

    // Step 1: determine which categories set the initial pre-selection.
    let mut line_length = DEFAULT_LINE_LENGTH;
    let default_categories: HashSet<Category> = if let Some(profile) = profile_arg {
        profile_to_categories(profile)
    } else if yes {
        profile_to_categories(Profile::Default)
    } else {
        let mut cat_items = default_category_items();
        if !select_categories_arrow(&mut cat_items, &mut line_length)? {
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
    let unsupported_keys: Vec<&str> = crate::registry::UNSUPPORTED_KEYS
        .iter()
        .filter_map(|(old_key, _)| current_tool_keys.contains(*old_key).then_some(*old_key))
        .collect();

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

    // Detect obsolete tool keys (e.g. github:mvdan/sh → shfmt).
    // These are removed regardless of the interactive selection — keeping them serves no purpose.
    let obsolete = detect_obsolete_keys(&current_tool_keys);
    for (old_key, replacement) in &obsolete {
        println!("  removing obsolete linter {old_key} (replaced by {replacement})");
    }
    for old_key in &unsupported_keys {
        println!("  removing unsupported legacy linter {old_key}");
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
    for old_key in &unsupported_keys {
        final_remove.push((*old_key).to_string());
    }

    let has_slow = has_slow_selected(&groups);
    let has_renovate = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "renovate-deps")
    });
    let has_rumdl = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "rumdl")
    });
    let has_yaml_lint = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "yaml-lint")
    });
    let has_taplo = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "taplo")
    });
    let has_biome = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && (c.name == "biome" || c.name == "biome-format"))
    });
    let has_cargo_fmt = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "cargo-fmt")
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
    let rumdl_generated = if has_rumdl {
        generate_rumdl_config(project_root, &config_dir_path, line_length)?
    } else {
        false
    };
    let editorconfig_line_length_sections = selected_editorconfig_line_length_sections(&groups);
    let delegated_patterns = editorconfig_line_length_sections
        .iter()
        .map(|(patterns, _)| *patterns)
        .collect::<Vec<_>>();
    let migration_summary =
        apply_repo_migrations(project_root, &config_dir_path, &delegated_patterns)?;
    migration_summary.print_messages();
    let editorconfig_generated = generate_editorconfig(project_root, line_length)?;
    let editorconfig_line_length_disabled = disable_editorconfig_line_length_for_patterns(
        project_root,
        &editorconfig_line_length_sections,
    )?;
    if !editorconfig_line_length_disabled.is_empty() {
        println!(
            "  patched <REPO>/.editorconfig — disable max_line_length for {}",
            editorconfig_line_length_disabled.join(", ")
        );
    }
    let yamllint_generated = if has_yaml_lint {
        generate_yamllint_config(&config_dir_path, line_length)?
    } else {
        false
    };
    let taplo_generated = if has_taplo {
        generate_taplo_config(&config_dir_path, line_length)?
    } else {
        false
    };
    let rustfmt_generated = if has_cargo_fmt {
        generate_rustfmt_config(&config_dir_path, line_length)?
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
        && migration_summary.is_noop()
        && !flint_pinned
        && !tools_normalized
        && v1.removed_tasks.is_empty()
        && !v1.removed_renovate_env
        && !meta_changed
        && !toml_generated
        && !workflow_generated
        && !rumdl_generated
        && !editorconfig_generated
        && editorconfig_line_length_disabled.is_empty()
        && !yamllint_generated
        && !taplo_generated
        && !rustfmt_generated
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

pub fn update(project_root: &Path, config_dir: &Path) -> Result<()> {
    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);
    let delegated_sections = active_editorconfig_line_length_sections(&current_tool_keys);
    let delegated_patterns = delegated_sections
        .iter()
        .map(|(patterns, _)| *patterns)
        .collect::<Vec<_>>();
    let migration_summary = apply_repo_migrations(project_root, config_dir, &delegated_patterns)?;
    let tools_normalized = normalize_tools_section(&mise_path)?;

    if migration_summary.is_noop() && !tools_normalized {
        println!("flint: repo lint migration is up to date");
        return Ok(());
    }

    migration_summary.print_messages();

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
/// Preference order: `mise_tool_name` → `bin_name`.
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
    use config_files::generate_flint_toml;
    use detection::entry_components_differ;
    use generation::{
        apply_changes, get_existing_config_dir, has_slow_selected, normalize_tools_section,
    };
    use scaffold::{apply_env_and_tasks, generate_lint_workflow};

    #[test]
    fn detect_obsolete_keys_finds_known_stale_key() {
        use detection::detect_obsolete_keys;
        let mut keys = HashSet::new();
        keys.insert("github:mvdan/sh".to_string());
        keys.insert("shellcheck".to_string());
        let found = detect_obsolete_keys(&keys);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "github:mvdan/sh");
        assert_eq!(found[0].1, "shfmt");
    }

    #[test]
    fn detect_obsolete_keys_ignores_current_keys() {
        use detection::detect_obsolete_keys;
        let mut keys = HashSet::new();
        keys.insert("rumdl".to_string());
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
actionlint = "1.7.0"
rumdl = "0.1.0"
rust = { version = "1.95.0", components = "clippy,rustfmt" }
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let changed = normalize_tools_section(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        let header_pos = result.find("# Linters").expect("header present");
        let biome_pos = result.find("biome =").unwrap_or(usize::MAX);
        let rust_pos = result.find("rust =").expect("rust present");
        let actionlint_pos = result.find("actionlint =").expect("actionlint present");
        let lychee_pos = result.find("lychee =").expect("lychee present");
        let rumdl_pos = result.find("rumdl =").expect("rumdl present");
        assert!(rust_pos < header_pos, "toolchains above header");
        assert!(actionlint_pos > header_pos, "linters below header");
        assert!(
            actionlint_pos < lychee_pos
                && lychee_pos < rumdl_pos
                && (biome_pos == usize::MAX || rumdl_pos < biome_pos),
            "linters sorted alphabetically"
        );

        // Idempotent: second call returns false and leaves content unchanged.
        let changed_again = normalize_tools_section(tmp.path()).unwrap();
        assert!(!changed_again);
        let result_again = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(result, result_again);
    }

    #[test]
    fn normalize_tools_section_moves_node_above_linters_header() {
        let content = r#"[tools]
rust = { version = "1.95.0", components = "clippy,rustfmt" }

# Linters
bats = "1.13.0"
java = "temurin-25.0.2+10.0.LTS"
node = "24.15.0"
"npm:renovate" = "43.0.0"
shellcheck = "0.11.0"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let changed = normalize_tools_section(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        let bats_pos = result.find("bats =").expect("bats present");
        let java_pos = result.find("java =").expect("java present");
        let node_pos = result.find("node =").expect("node present");
        let header_pos = result.find("# Linters").expect("header present");
        let renovate_pos = result.find("\"npm:renovate\"").expect("renovate present");
        assert!(
            bats_pos < header_pos
                && java_pos < header_pos
                && node_pos < header_pos
                && header_pos < renovate_pos,
            "non-linter tools must stay above linter header:\n{result}"
        );
        assert_eq!(result.matches("# Linters").count(), 1, "single header");
    }

    #[test]
    fn normalize_tools_section_preserves_unrelated_tool_comments() {
        let content = r#"[tools]
# Runtime comment
node = "24.15.0"

# Linters
shellcheck = "0.11.0"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        normalize_tools_section(tmp.path()).unwrap();
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(result.contains("# Runtime comment"));
        assert!(result.contains("# Linters"));
        assert_eq!(result.matches("# Linters").count(), 1);
    }

    #[test]
    fn normalize_tools_section_keeps_unknown_tools_above_linters_header() {
        let content = r#"[tools]

# Linters
custom-tool = "1.0.0"
java = "temurin-25.0.3+9.0.LTS"
node = "24.15.0"
protoc = "34.1"
shellcheck = "0.11.0"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let changed = normalize_tools_section(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        let custom_pos = result.find("custom-tool =").expect("custom tool present");
        let java_pos = result.find("java =").expect("java present");
        let node_pos = result.find("node =").expect("node present");
        let protoc_pos = result.find("protoc =").expect("protoc present");
        let header_pos = result.find("# Linters").expect("header present");
        let shellcheck_pos = result.find("shellcheck =").expect("shellcheck present");
        assert!(
            custom_pos < header_pos
                && java_pos < header_pos
                && node_pos < header_pos
                && protoc_pos < header_pos
                && header_pos < shellcheck_pos,
            "only explicitly managed linter keys belong below the header:\n{result}"
        );
        assert_eq!(result.matches("# Linters").count(), 1, "single header");
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
rumdl = "0.1.0"
rust = { version = "1.0", components = "clippy" }
"#;
        let keys = parse_tool_keys(content);
        assert!(keys.contains("shellcheck"));
        assert!(keys.contains("rumdl"));
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
    fn generate_rumdl_config_writes_file() {
        use config_files::generate_rumdl_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        let written = generate_rumdl_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
        assert!(content.contains("line-length = 120"));
        assert!(content.contains("code-blocks = false"));
        assert!(!content.contains("[global]"));
    }

    #[test]
    fn generate_rumdl_config_skips_when_target_exists() {
        use config_files::generate_rumdl_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join(".rumdl.toml"), "existing").unwrap();
        let written = generate_rumdl_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_rumdl_config_replaces_legacy_json() {
        use config_files::generate_rumdl_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::write(tmp.path().join(".markdownlint.json"), r#"{"MD013":false}"#).unwrap();
        let written = generate_rumdl_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        assert!(!tmp.path().join(".markdownlint.json").exists());
        let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
        assert!(content.contains("[MD013]"));
    }

    #[test]
    fn remove_legacy_lint_files_removes_v1_artifacts() {
        use config_files::remove_legacy_lint_files;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(tmp.path().join(".prettierignore"), "docs/themes/**\n").unwrap();
        std::fs::write(tmp.path().join(".gitleaksignore"), "secret\n").unwrap();
        std::fs::write(config_dir.join("super-linter.env"), "LOG_LEVEL=ERROR\n").unwrap();

        let removed = remove_legacy_lint_files(tmp.path(), &config_dir).unwrap();
        assert_eq!(removed.len(), 3);
        assert!(!tmp.path().join(".prettierignore").exists());
        assert!(!tmp.path().join(".gitleaksignore").exists());
        assert!(!config_dir.join("super-linter.env").exists());
    }

    #[test]
    fn remove_stale_markdownlint_line_length_directives_strips_md013_only() {
        use config_files::remove_stale_markdownlint_line_length_directives;
        let tmp = tempfile::TempDir::new().unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        std::fs::write(
            tmp.path().join("README.md"),
            "# Title\n\n<!-- markdownlint-disable MD013 -->\nlong line\n<!-- markdownlint-enable MD013 -->\n<!-- markdownlint-disable MD033 -->\nhtml\n<!-- markdownlint-enable MD033 -->\n",
        )
        .unwrap();
        std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        let changed = remove_stale_markdownlint_line_length_directives(tmp.path()).unwrap();
        assert_eq!(changed, vec!["README.md".to_string()]);
        let updated = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
        assert!(!updated.contains("markdownlint-disable MD013"));
        assert!(!updated.contains("markdownlint-enable MD013"));
        assert!(updated.contains("markdownlint-disable MD033"));
        assert!(updated.contains("markdownlint-enable MD033"));
    }

    #[test]
    fn remove_stale_editorconfig_checker_directives_strips_delegated_markdown_comments() {
        use config_files::remove_stale_editorconfig_checker_directives;
        let tmp = tempfile::TempDir::new().unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        std::fs::write(
            tmp.path().join("README.md"),
            "# Title\n\n<!-- editorconfig-checker-disable -->\n- [Link](https://example.com) <!-- editorconfig-checker-disable-line -->\n<!-- editorconfig-checker-enable -->\n",
        )
        .unwrap();
        std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        let changed =
            remove_stale_editorconfig_checker_directives(tmp.path(), &[&["*.md"]]).unwrap();
        assert_eq!(changed, vec!["README.md".to_string()]);
        let updated = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
        assert!(!updated.contains("editorconfig-checker-disable"));
        assert!(!updated.contains("editorconfig-checker-enable"));
        assert!(updated.contains("- [Link](https://example.com)"));
    }

    #[test]
    fn generate_editorconfig_writes_file() {
        use config_files::generate_editorconfig;
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
        assert!(content.contains("max_line_length = 120"));
        assert!(content.contains("insert_final_newline = true"));
    }

    #[test]
    fn generate_editorconfig_patches_existing_global_section() {
        use config_files::generate_editorconfig;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".editorconfig"),
            "root = true\n\n[*]\nindent_size = 2\n\n[*.rs]\nindent_size = 4\n",
        )
        .unwrap();
        let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
        assert!(content.contains("[*]\nindent_size = 2\nmax_line_length = 120\n"));
        assert!(content.contains("[*.rs]\nindent_size = 4\n"));
    }

    #[test]
    fn generate_editorconfig_skips_existing_line_length() {
        use config_files::generate_editorconfig;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".editorconfig"),
            "root = true\n\n[*]\nmax_line_length = 100\n",
        )
        .unwrap();
        let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
        assert!(content.contains("max_line_length = 100"));
        assert!(!content.contains("max_line_length = 120"));
    }

    #[test]
    fn disable_editorconfig_line_length_for_patterns_updates_editorconfig() {
        use config_files::disable_editorconfig_line_length_for_patterns;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".editorconfig"),
            "root = true\n\n[*]\nmax_line_length = 120\n",
        )
        .unwrap();
        let changed = disable_editorconfig_line_length_for_patterns(
            tmp.path(),
            &[(&["*.md"], "Markdown line length is handled by rumdl")],
        )
        .unwrap();
        assert_eq!(changed, vec!["[*.md]".to_string()]);
        let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
        assert!(content.contains("[*.md]"));
        assert!(content.contains("# Markdown line length is handled by rumdl"));
        assert!(content.contains("max_line_length = off"));
    }

    #[test]
    fn disable_editorconfig_line_length_for_patterns_is_idempotent() {
        use config_files::disable_editorconfig_line_length_for_patterns;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".editorconfig"),
            "root = true\n\n[*]\nmax_line_length = 120\n\n[*.md]\n# Markdown line length is handled by rumdl\nmax_line_length = off\n",
        )
        .unwrap();
        let changed = disable_editorconfig_line_length_for_patterns(
            tmp.path(),
            &[(&["*.md"], "Markdown line length is handled by rumdl")],
        )
        .unwrap();
        assert!(changed.is_empty());
    }

    #[test]
    fn generate_yamllint_config_writes_file() {
        use config_files::generate_yamllint_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        let written = generate_yamllint_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(config_dir.join(".yamllint.yml")).unwrap();
        assert!(content.contains("extends: relaxed"));
        assert!(content.contains("document-start: disable"));
        assert!(content.contains("line-length:"));
        assert!(content.contains("max: 120"));
        assert!(content.contains("indentation:"));
        assert!(content.contains("spaces: 2"));
    }

    #[test]
    fn generate_taplo_config_writes_file() {
        use config_files::generate_taplo_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        let written = generate_taplo_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(config_dir.join(".taplo.toml")).unwrap();
        assert!(content.contains("[formatting]"));
        assert!(content.contains("column_width = 120"));
        assert!(content.contains("indent_string = \"  \""));
    }

    #[test]
    fn generate_taplo_config_skips_existing_supported_file() {
        use config_files::generate_taplo_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join(".taplo.toml"), "existing").unwrap();
        let written = generate_taplo_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(config_dir.join(".taplo.toml")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_taplo_config_skips_existing_legacy_name() {
        use config_files::generate_taplo_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("taplo.toml"), "existing").unwrap();
        let written = generate_taplo_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(!written);
        assert!(!config_dir.join(".taplo.toml").exists());
    }

    #[test]
    fn generate_rustfmt_config_writes_file() {
        use config_files::generate_rustfmt_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        let written = generate_rustfmt_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(config_dir.join("rustfmt.toml")).unwrap();
        assert_eq!(content, "max_width = 120\n");
    }

    #[test]
    fn generate_rustfmt_config_skips_existing_file() {
        use config_files::generate_rustfmt_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("rustfmt.toml"), "existing").unwrap();
        let written = generate_rustfmt_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(config_dir.join("rustfmt.toml")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_biome_config_writes_file() {
        use config_files::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
        assert!(content.contains("\"indentStyle\": \"space\""));
        assert!(content.contains("\"indentWidth\": 2"));
    }

    #[test]
    fn generate_biome_config_skips_existing_jsonc() {
        use config_files::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("biome.jsonc"), "existing").unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
        assert_eq!(content, "existing");
    }

    #[test]
    fn generate_biome_config_migrates_legacy_supported_json_name() {
        use config_files::generate_biome_config;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("biome.json"), "existing").unwrap();
        let written = generate_biome_config(tmp.path()).unwrap();
        assert!(written);
        assert!(!tmp.path().join("biome.json").exists());
        let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
        assert_eq!(content, "existing");
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
        assert!(!content.contains("--fast-only"));
    }

    #[test]
    fn apply_env_and_tasks_does_not_add_pre_commit_task_when_slow() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        apply_env_and_tasks(tmp.path(), ".", true, &[]).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!content.contains("--fast-only"));
        assert!(!content.contains("lint:pre-commit"));
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
