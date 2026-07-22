use anyhow::{Result, bail};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::registry::{Category, Check, InitHookContext, WorkflowSetup, builtin};

mod config_files;
mod detection;
pub(crate) mod generation;
mod migrations;
mod mise_tools;
mod scaffold;
mod ui;

use config_files::{
    disable_editorconfig_line_length_for_patterns, generate_editorconfig, generate_flint_toml,
};
use detection::{
    build_explicit_linter_groups, build_linter_groups, detect_obsolete_keys,
    detect_present_patterns, parse_tool_keys,
};
use generation::{
    apply_changes, detect_base_branch, ensure_flint_self_pin, ensure_flint_self_pin_additive,
    ensure_node_for_npm, get_existing_config_dir, has_slow_selected, normalize_tools_section,
    prompt_config_dir, remove_tool_keys,
};
use migrations::{
    apply_repo_migrations, selected_editorconfig_cleanup_sections,
    selected_editorconfig_line_length_sections,
};
pub(crate) use migrations::{apply_setup_migrations, detect_setup_migrations};
use scaffold::{
    apply_env_and_tasks, ensure_agent_linting_guidance, generate_lint_workflow, maybe_install_hook,
};
use ui::{interactive_select_linters, select_categories_arrow};

const DEFAULT_LINE_LENGTH: u16 = 120;

fn interactive_note(yes: bool, message: &str) {
    if !yes {
        println!("{message}");
    }
}

/// Linter profile — shorthand for `--profile` CLI flag; maps to a category set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Primary language linters only (ruff, cargo-clippy, golangci-lint, …).
    Lang,
    /// Lang + supplementary checks + fast general tools (shellcheck, rumdl, typos, …).
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

fn resolve_only_checks<'a>(registry: &'a [Check], names: &[String]) -> Result<Vec<&'a Check>> {
    let requested: HashSet<&str> = names.iter().map(String::as_str).collect();
    let unknown: Vec<&str> = names
        .iter()
        .filter_map(|name| {
            (!registry.iter().any(|check| check.name == name)).then_some(name.as_str())
        })
        .collect();
    if !unknown.is_empty() {
        let noun = if unknown.len() == 1 {
            "check"
        } else {
            "checks"
        };
        bail!(
            "flint: unknown {noun}: {}",
            unknown
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Follow registry order rather than CLI order. This keeps shared tool
    // components and setup hooks deterministic, and naturally de-duplicates
    // repeated names.
    Ok(registry
        .iter()
        .filter(|check| requested.contains(check.name))
        .collect())
}

fn merge_components(current: Option<&str>, required: &str) -> String {
    let mut components: Vec<&str> = current
        .into_iter()
        .flat_map(|components| components.split(','))
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .collect();

    for required_component in required.split(',').map(str::trim) {
        if !required_component.is_empty() && !components.contains(&required_component) {
            components.push(required_component);
        }
    }
    components.join(",")
}

type ToolAdd = (String, Option<String>);
type ToolUpgrade = (String, String);

fn explicit_tool_changes(groups: &[LinterGroup<'_>]) -> (Vec<ToolAdd>, Vec<ToolUpgrade>) {
    let mut to_add = vec![];
    let mut to_upgrade = vec![];

    for group in groups {
        let Some(required) = group.selected_components() else {
            if !group.installed {
                to_add.push((group.key.to_string(), None));
            }
            continue;
        };

        if !group.installed {
            to_add.push((group.key.to_string(), Some(required)));
            continue;
        }

        let merged = merge_components(group.current_components.as_deref(), &required);
        if group.current_components.as_deref() != Some(merged.as_str()) {
            to_upgrade.push((group.key.to_string(), merged));
        }
    }

    (to_add, to_upgrade)
}

fn selected_checks<'a>(groups: &'a [LinterGroup<'a>]) -> Vec<&'a Check> {
    groups
        .iter()
        .flat_map(|group| {
            group
                .checks
                .iter()
                .zip(&group.check_selected)
                .filter_map(|(check, selected)| selected.then_some(*check))
        })
        .collect()
}

struct CheckTypeInitHookContext<'a> {
    project_root: &'a Path,
    config_dir: &'a Path,
    line_length: u16,
    flint_toml_generated: bool,
}

impl InitHookContext for CheckTypeInitHookContext<'_> {
    fn project_root(&self) -> &Path {
        self.project_root
    }

    fn config_dir(&self) -> &Path {
        self.config_dir
    }

    fn line_length(&self) -> u16 {
        self.line_length
    }

    fn flint_toml_generated(&self) -> bool {
        self.flint_toml_generated
    }
}

fn apply_check_type_init_hooks(
    checks: &[&Check],
    project_root: &Path,
    config_dir: &Path,
    line_length: u16,
    flint_toml_generated: bool,
) -> Result<bool> {
    let context = CheckTypeInitHookContext {
        project_root,
        config_dir,
        line_length,
        flint_toml_generated,
    };
    let mut changed = false;
    let mut initialized_check_types = HashSet::new();
    for check in checks {
        if let Some(check_type) = check.check_type
            && let Some(hook) = check_type.init_hook()
            && initialized_check_types.insert(check_type.name())
        {
            changed |= hook(&context)?;
        }
    }
    Ok(changed)
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
            label: "general — general tools (typos, ec, lychee, …)",
        },
        CategoryItem {
            selected: false,
            category: Category::Slow,
            label: "slow    — slow linters (renovate-deps)",
        },
    ]
}

pub fn run(
    project_root: &Path,
    profile_arg: Option<Profile>,
    yes: bool,
    flint_rev: Option<&str>,
) -> Result<()> {
    run_with_only(project_root, profile_arg, &[], yes, flint_rev)
}

pub fn run_with_only(
    project_root: &Path,
    profile_arg: Option<Profile>,
    only_names: &[String],
    yes: bool,
    flint_rev: Option<&str>,
) -> Result<()> {
    let registry = builtin();
    let only_checks = resolve_only_checks(&registry, only_names)?;
    let only_mode = !only_names.is_empty();
    if only_mode {
        let builtins: Vec<&str> = only_checks
            .iter()
            .filter(|check| install_key(check).is_none())
            .map(|check| check.name)
            .collect();
        if !builtins.is_empty() {
            bail!(
                "flint: --only cannot install built-in or config-only checks: {}",
                builtins.join(", ")
            );
        }
    }

    println!(
        "Tip: flint init detects languages from tracked files (`git ls-files`). \
Add and stage your source files before running init so the detection is accurate."
    );
    println!();

    let mut present_patterns = if only_mode {
        HashSet::new()
    } else {
        detect_present_patterns(project_root, &registry)?
    };

    // If init will generate `.github/workflows/lint.yml`, treat both the workflow-
    // specific patterns and generic YAML patterns as present so actionlint and
    // ryl get selected in the same run. Without this, init would be
    // non-idempotent: the second run would see the newly-generated workflow and
    // add extra linters then.
    if !only_mode && !project_root.join(".github/workflows/lint.yml").exists() {
        present_patterns.insert(".github/workflows/*.yml".to_string());
        present_patterns.insert(".github/workflows/*.yaml".to_string());
        present_patterns.insert("*.yml".to_string());
        present_patterns.insert("*.yaml".to_string());
    }

    // Step 1: determine which categories set the initial pre-selection.
    let mut line_length = DEFAULT_LINE_LENGTH;
    let default_categories: HashSet<Category> = if only_mode {
        HashSet::new()
    } else if let Some(profile) = profile_arg {
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
    let unsupported_keys: Vec<&str> = crate::registry::unsupported_keys()
        .into_iter()
        .filter_map(|(old_key, _)| {
            (!only_mode && current_tool_keys.contains(old_key)).then_some(old_key)
        })
        .collect();

    // Step 2: build one group per install key, covering all checks whose files are
    // present in the repo or which are already installed.
    let mut groups = if only_mode {
        build_explicit_linter_groups(&only_checks, &current_tool_keys, &current_content)
    } else {
        build_linter_groups(
            &registry,
            &present_patterns,
            &current_tool_keys,
            &current_content,
            &default_categories,
        )
    };

    if groups.is_empty() && !only_mode {
        println!("No applicable linters found for this project.");
        return Ok(());
    }

    // Step 3: interactive linter table (skipped with --yes).
    if !only_mode && !yes && !interactive_select_linters(&mut groups)? {
        println!("Aborted.");
        return Ok(());
    }

    // Detect obsolete tool keys (e.g. github:mvdan/sh → shfmt).
    // These are removed regardless of the interactive selection — keeping them serves no purpose.
    let obsolete = if only_mode {
        vec![]
    } else {
        detect_obsolete_keys(&current_tool_keys)
    };
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

    if only_mode {
        let (add, upgrade) = explicit_tool_changes(&groups);
        final_add = add;
        final_upgrade = upgrade;
    } else {
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
    }

    // Always remove obsolete tool keys (detected before the interactive selection).
    for (old_key, _) in &obsolete {
        final_remove.push(old_key.to_string());
    }
    for old_key in &unsupported_keys {
        final_remove.push((*old_key).to_string());
    }

    let has_slow = has_slow_selected(&groups);
    let selected_checks = selected_checks(&groups);

    // Prompt for the flint config dir (skipped if already set in mise.toml or --yes).
    let existing_config_dir = get_existing_config_dir(&current_content);
    if existing_config_dir.is_some() {
        interactive_note(
            yes,
            "Using existing FLINT_CONFIG_DIR from mise.toml; skipping config-dir prompt.",
        );
    }
    let config_dir_rel = prompt_config_dir(existing_config_dir.as_deref(), yes)?;

    let tools_changed =
        !final_add.is_empty() || !final_remove.is_empty() || !final_upgrade.is_empty();
    if tools_changed {
        interactive_note(yes, "\nApplying mise/tooling changes...");
        apply_changes(
            &mise_path,
            &current_content,
            &final_add,
            &final_remove,
            &final_upgrade,
        )?;
    }
    let flint_pinned = if only_mode {
        ensure_flint_self_pin_additive(project_root, flint_rev)?
    } else {
        ensure_flint_self_pin(project_root, flint_rev)?
    };
    if flint_pinned {
        println!("  pinned flint itself — reproducible lint runs across contributors");
    }
    let meta_changed = apply_env_and_tasks(&mise_path, &config_dir_rel, has_slow)?;
    let node_added = ensure_node_for_npm(project_root)?;
    if node_added {
        println!("  added node (LTS) — required by npm: backend tools");
    }
    let tools_normalized = normalize_tools_section(&mise_path)?;

    interactive_note(yes, "\nScaffolding flint config and workflow...");
    let base_branch = detect_base_branch(project_root);
    let config_dir_path = project_root.join(&config_dir_rel);
    let toml_generated = generate_flint_toml(&config_dir_path, &base_branch)?;
    let needs_rust_components = selected_checks
        .iter()
        .any(|check| check.workflow_setup == Some(WorkflowSetup::RustComponents));
    let workflow_generated =
        generate_lint_workflow(project_root, &base_branch, needs_rust_components)?;
    let check_type_init_changed = apply_check_type_init_hooks(
        &selected_checks,
        project_root,
        &config_dir_path,
        line_length,
        toml_generated,
    )?;
    let editorconfig_line_length_sections =
        selected_editorconfig_line_length_sections(&selected_checks);
    let editorconfig_cleanup_sections = selected_editorconfig_cleanup_sections(&selected_checks);
    let migration_summary = if only_mode {
        migrations::RepoMigrationSummary::noop()
    } else {
        apply_repo_migrations(
            project_root,
            &config_dir_path,
            &editorconfig_cleanup_sections,
        )?
    };
    migration_summary.print_messages();
    let editorconfig_generated = generate_editorconfig(project_root, line_length)?;
    let editorconfig_line_length_disabled = disable_editorconfig_line_length_for_patterns(
        project_root,
        &editorconfig_line_length_sections,
    )?;
    let agent_guidance_changed = if only_mode {
        false
    } else {
        ensure_agent_linting_guidance(project_root)?
    };
    if !editorconfig_line_length_disabled.is_empty() {
        println!(
            "  patched <REPO>/.editorconfig — disable max_line_length for {}",
            editorconfig_line_length_disabled.join(", ")
        );
    }
    if !tools_changed
        && migration_summary.is_noop()
        && !flint_pinned
        && !meta_changed
        && !node_added
        && !tools_normalized
        && !toml_generated
        && !workflow_generated
        && !check_type_init_changed
        && !editorconfig_generated
        && !agent_guidance_changed
        && editorconfig_line_length_disabled.is_empty()
    {
        println!("No changes to apply.");
        return Ok(());
    }

    interactive_note(yes, "\nFinishing setup...");
    maybe_install_hook(project_root, yes)?;

    println!("Done. Run `mise install` to install the new tools.");
    Ok(())
}

/// Returns the canonical mise.toml tool key to write when installing this check
/// via `flint init`, or `None` if no mise entry is needed (built-in or
/// unconditionally active checks).
///
/// Preference order: `mise_tool_name` → `bin_name`.
pub fn install_key(check: &Check) -> Option<&'static str> {
    check.install_key()
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
mod tests;
