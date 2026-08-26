//! Presentation for the `flint linters` command.

use crate::registry::{FixBehavior, LinterConfig};
use crate::{config, registry};
use std::collections::HashMap;

struct LinterStatusContext<'a> {
    cfg: &'a config::Config,
}

impl registry::StatusContext for LinterStatusContext<'_> {
    fn config(&self) -> &config::Config {
        self.cfg
    }
}

pub(crate) fn print_json(
    registry: &[registry::Check],
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
) {
    let entries: Vec<serde_json::Value> = registry
        .iter()
        .map(|check| linter_json_for(check, mise_tools, cfg, registry::binary_on_path))
        .collect();
    println!("{}", serde_json::to_string_pretty(&entries).unwrap());
}

pub(crate) fn linter_json_for<F>(
    check: &registry::Check,
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
    binary_on_path: F,
) -> serde_json::Value
where
    F: Fn(&str) -> bool,
{
    let status = linter_status(check, mise_tools, cfg, binary_on_path);
    let declared_version = registry::declared_tool_version(check, mise_tools);
    linter_json(check, status, declared_version)
}

pub(crate) fn linter_json(
    check: &registry::Check,
    status: &str,
    declared_version: Option<&str>,
) -> serde_json::Value {
    let scope = check.kind.scope_name();
    let patterns: Vec<&str> = check.patterns.to_vec();
    let config_file = check
        .linter_config
        .as_ref()
        .map(LinterConfig::canonical_location);
    let baseline_config = check
        .baseline_config
        .map(|config| config_file_location(&config));
    let baseline_triggers: Vec<String> = check
        .baseline_triggers
        .iter()
        .map(config_file_location)
        .collect();
    let fix_behavior = check.has_fix().then(|| match check.fix_behavior() {
        FixBehavior::Definitive => "definitive",
        FixBehavior::PartialNeedsVerify => "partial-needs-verify",
    });
    serde_json::json!({
        "name": check.name,
        "description": check.desc,
        "binary": if check.uses_binary() { check.bin_name } else { "(built-in)" },
        "install_key": check.install_key(),
        "status": status,
        "declared_version": declared_version,
        "patterns": patterns,
        "fix": check.has_fix(),
        "fix_behavior": fix_behavior,
        "run_policy": run_policy_label(check),
        "slow": check.category == registry::Category::Slow,
        "category": category_label(check.category),
        "scope": scope,
        "config_file": config_file,
        "config_doc_url": check.config_doc_url,
        "project_url": check.project_url,
        "formatter": check.is_formatter,
        "defers_to_formatters": check.defers_to_formatters,
        "baseline_config": baseline_config,
        "baseline_triggers": baseline_triggers,
        "fix_after": check.fix_after,
    })
}

fn category_label(category: registry::Category) -> &'static str {
    match category {
        registry::Category::Lang => "lang",
        registry::Category::Style => "style",
        registry::Category::Default => "default",
        registry::Category::Slow => "slow",
    }
}

fn config_file_location(config: &registry::ConfigFile) -> String {
    match config.base {
        registry::ConfigBase::ProjectRoot => config.path.to_string(),
        registry::ConfigBase::ConfigDir => format!("FLINT_CONFIG_DIR/{}", config.path),
    }
}

fn run_policy_label(check: &registry::Check) -> &'static str {
    if check.adaptive_relevance.is_some() {
        "adaptive"
    } else if check.category == registry::Category::Slow {
        "slow"
    } else {
        "fast"
    }
}

pub(crate) fn print_table(
    registry: &[registry::Check],
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
) {
    print!(
        "{}",
        render_linters_table(registry, mise_tools, cfg, registry::binary_on_path)
    );
}

pub(crate) fn render_linters_table<F>(
    registry: &[registry::Check],
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
    binary_on_path: F,
) -> String
where
    F: Fn(&str) -> bool,
{
    use std::fmt::Write;

    // Column widths.
    let name_w = registry
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let bin_w = registry
        .iter()
        .map(display_binary)
        .map(str::len)
        .max()
        .unwrap_or(6)
        .max(6);
    let desc_w = registry
        .iter()
        .map(|c| c.desc.len())
        .max()
        .unwrap_or(11)
        .max(11);

    let mut out = String::new();
    writeln!(
        out,
        "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {:<desc_w$}  PATTERNS",
        "NAME",
        "BINARY",
        "STATUS",
        "SPEED",
        "FIX",
        "DESCRIPTION",
        name_w = name_w,
        bin_w = bin_w,
        desc_w = desc_w,
    )
    .unwrap();
    writeln!(out, "{}", "-".repeat(name_w + bin_w + desc_w + 46)).unwrap();

    for check in registry {
        let status = linter_status(check, mise_tools, cfg, &binary_on_path);
        let speed = run_policy_label(check);
        let fix = if check.has_fix() { "yes" } else { "no" };
        let patterns_str = check.patterns.join(" ");
        let binary = display_binary(check);
        if patterns_str.is_empty() {
            writeln!(
                out,
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {}",
                check.name,
                binary,
                status,
                speed,
                fix,
                check.desc,
                name_w = name_w,
                bin_w = bin_w,
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {:<desc_w$}  {}",
                check.name,
                binary,
                status,
                speed,
                fix,
                check.desc,
                patterns_str,
                name_w = name_w,
                bin_w = bin_w,
                desc_w = desc_w,
            )
            .unwrap();
        }
    }

    out
}

pub(crate) fn linter_status<F>(
    check: &registry::Check,
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
    binary_on_path: F,
) -> &'static str
where
    F: Fn(&str) -> bool,
{
    if registry::check_active(check, mise_tools) {
        if !check.uses_binary() || binary_on_path(check.bin_name) {
            let status_ctx = LinterStatusContext { cfg };
            check
                .status_hook
                .and_then(|hook| hook(&status_ctx))
                .unwrap_or("active")
        } else {
            "no binary"
        }
    } else if registry::declared_tool_version(check, mise_tools).is_some() {
        "wrong version"
    } else {
        "missing"
    }
}

pub(crate) fn display_binary(check: &registry::Check) -> &'static str {
    if check.uses_binary() {
        check.bin_name
    } else {
        "(built-in)"
    }
}
