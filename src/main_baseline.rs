//! Baseline expansion and linter configuration discovery for `flint run`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::{files, registry};

pub(crate) fn baseline_check_names(
    active: &[&registry::Check],
    file_list: &files::FileList,
    project_root: &Path,
    config_dir: &Path,
    current_tools: &HashMap<String, String>,
) -> HashSet<String> {
    if file_list.full {
        return HashSet::new();
    }
    let Some(merge_base) = file_list.merge_base.as_deref() else {
        return HashSet::new();
    };

    let changed = changed_rel_paths(file_list, project_root);
    let previous_tools = registry::read_mise_tools_at_ref(project_root, merge_base);
    if registry::flint_version_changed(&previous_tools, current_tools)
        || registry::full_baseline_runtime_changed(active, &previous_tools, current_tools)
    {
        return active.iter().map(|check| check.name.to_string()).collect();
    }

    let flint_config = config_rel_path(project_root, config_dir, "flint.toml");
    let flint_config_changed = changed.contains(&flint_config);
    let flint_toml =
        flint_config_changed.then(|| flint_toml_change(project_root, config_dir, merge_base));

    active
        .iter()
        .filter(|check| {
            let newly_active = !registry::check_active(check, &previous_tools);
            let tool_version_changed =
                registry::tool_version_changed(check, &previous_tools, current_tools);
            let runtime_version_changed =
                registry::runtime_version_changed(check, &previous_tools, current_tools);
            let flint_toml_requires_baseline = flint_toml.as_ref().is_some_and(|change| {
                change.settings_changed
                    || (check.kind.is_native() && change.check_changed(check.name))
            });
            let baseline_config_changed = check.baseline_config.as_ref().is_some_and(|config| {
                changed.contains(&config_file_rel_path(project_root, config_dir, config))
            });
            let baseline_trigger_changed = check.baseline_triggers.iter().any(|config| {
                changed.contains(&config_file_rel_path(project_root, config_dir, config))
            });

            newly_active
                || tool_version_changed
                || runtime_version_changed
                || flint_toml_requires_baseline
                || baseline_config_changed
                || baseline_trigger_changed
        })
        .map(|check| check.name.to_string())
        .collect()
}

pub(crate) fn unsupported_config(
    check: &registry::Check,
    project_root: &Path,
    config_dir: &Path,
) -> Option<String> {
    let baseline_path = check
        .baseline_config
        .as_ref()
        .map(|config| config_file_abs_path(project_root, config_dir, config));

    check
        .unsupported_configs
        .iter()
        .find(|config| {
            let path = config_file_abs_path(project_root, config_dir, config);
            let overlaps_baseline = baseline_path
                .as_ref()
                .is_some_and(|baseline| *baseline == path);
            (!overlaps_baseline || !check.allow_baseline_overlap_in_unsupported_configs)
                && config_present(project_root, config_dir, config)
        })
        .map(|config| config_file_rel_path(project_root, config_dir, config))
}

pub(crate) struct FlintTomlChange {
    pub(crate) current: toml::Value,
    pub(crate) previous: toml::Value,
    pub(crate) settings_changed: bool,
}

impl FlintTomlChange {
    pub(crate) fn check_changed(&self, name: &str) -> bool {
        self.check_config(&self.current, name) != self.check_config(&self.previous, name)
    }

    fn check_config<'a>(&self, value: &'a toml::Value, name: &str) -> Option<&'a toml::Value> {
        let underscore_alias = name.replace('-', "_");
        toml_section(value, &["checks", name])
            .or_else(|| toml_section(value, &["checks", &underscore_alias]))
    }
}

pub(crate) fn flint_toml_change(
    project_root: &Path,
    config_dir: &Path,
    merge_base: &str,
) -> FlintTomlChange {
    let rel = config_rel_path(project_root, config_dir, "flint.toml");
    let current_path = project_root.join(&rel);
    let current = read_toml_file(&current_path);
    let previous = read_toml_at_ref(project_root, merge_base, &rel);
    let settings_changed =
        toml_section(&current, &["settings"]) != toml_section(&previous, &["settings"]);
    FlintTomlChange {
        current,
        previous,
        settings_changed,
    }
}

pub(crate) fn read_toml_file(path: &Path) -> toml::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()))
}

pub(crate) fn config_present(
    project_root: &Path,
    config_dir: &Path,
    config: &registry::ConfigFile,
) -> bool {
    let path = config_file_abs_path(project_root, config_dir, config);
    match config.presence {
        registry::ConfigMatch::Exists => path.exists(),
        registry::ConfigMatch::TomlSection(section) => {
            toml_section(&read_toml_file(&path), section).is_some()
        }
        registry::ConfigMatch::IniSection(section) => ini_section_exists(&path, section),
    }
}

pub(crate) fn ini_section_exists(path: &Path, section: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .is_some_and(|name| name.trim() == section)
    })
}

pub(crate) fn read_toml_at_ref(project_root: &Path, git_ref: &str, rel_path: &str) -> toml::Value {
    let spec = format!("{git_ref}:{rel_path}");
    std::process::Command::new("git")
        .args(["show", &spec])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()))
}

pub(crate) fn toml_section<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub(crate) fn changed_rel_paths(
    file_list: &files::FileList,
    project_root: &Path,
) -> HashSet<String> {
    if !file_list.changed_paths.is_empty() {
        return file_list.changed_paths.iter().cloned().collect();
    }

    file_list
        .files
        .iter()
        .filter_map(|path| path.strip_prefix(project_root).ok())
        .map(normalize_path)
        .collect()
}

pub(crate) fn config_rel_path(project_root: &Path, config_dir: &Path, file: &str) -> String {
    let path = if config_dir.is_absolute() {
        config_dir.join(file)
    } else {
        project_root.join(config_dir).join(file)
    };
    path.strip_prefix(project_root)
        .map(normalize_path)
        .unwrap_or_else(|_| normalize_path(&PathBuf::from(file)))
}

pub(crate) fn config_file_abs_path(
    project_root: &Path,
    config_dir: &Path,
    config: &registry::ConfigFile,
) -> PathBuf {
    match config.base {
        registry::ConfigBase::ProjectRoot => project_root.join(config.path),
        registry::ConfigBase::ConfigDir => {
            if config_dir.is_absolute() {
                config_dir.join(config.path)
            } else {
                project_root.join(config_dir).join(config.path)
            }
        }
    }
}

pub(crate) fn config_file_rel_path(
    project_root: &Path,
    config_dir: &Path,
    config: &registry::ConfigFile,
) -> String {
    let path = config_file_abs_path(project_root, config_dir, config);
    path.strip_prefix(project_root)
        .map(normalize_path)
        .unwrap_or_else(|_| normalize_path(&PathBuf::from(config.path)))
}

pub(crate) fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
