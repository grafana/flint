use std::collections::HashMap;
use std::path::Path;

use super::types::Check;

/// Reads `[tools]` from the consuming repo's mise.toml and returns a map of
/// tool name → declared version string.
///
/// Also registers normalized aliases for backend-prefixed tools so that checks
/// can match by their bare package/binary name. For example:
/// - `"cargo:yaml-lint"` → also registers `"yaml-lint"`
/// - `"github:google/google-java-format"` → also registers `"google-java-format"`
///
/// The original key is always preserved; aliases only fill in missing entries.
pub fn read_mise_tools(project_root: &Path) -> HashMap<String, String> {
    let path = project_root.join("mise.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    read_mise_tools_from_str(&content)
}

pub fn read_mise_tools_at_ref(project_root: &Path, git_ref: &str) -> HashMap<String, String> {
    let spec = format!("{git_ref}:mise.toml");
    let out = match std::process::Command::new("git")
        .args(["show", &spec])
        .current_dir(project_root)
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return HashMap::new(),
    };
    let content = String::from_utf8_lossy(&out.stdout);
    read_mise_tools_from_str(&content)
}

fn read_mise_tools_from_str(content: &str) -> HashMap<String, String> {
    let value: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut tools = HashMap::new();
    if let Some(table) = value.get("tools").and_then(|v| v.as_table()) {
        for (name, val) in table {
            let version = match val {
                toml::Value::String(s) => Some(s.clone()),
                toml::Value::Table(t) => {
                    t.get("version").and_then(|v| v.as_str()).map(String::from)
                }
                _ => None,
            };
            if let Some(v) = version {
                tools.insert(name.clone(), v);
            }
        }
    }
    // Add normalized aliases: strip the backend prefix (e.g. "cargo:", "pipx:", "github:")
    // and take the last path component (e.g. "@biomejs/biome" → "biome").
    // Aliases never override an explicitly declared entry.
    let aliases: Vec<(String, String)> = tools
        .iter()
        .filter_map(|(k, v)| {
            let (_, rest) = k.split_once(':')?;
            let base = rest.rsplit('/').next().unwrap_or(rest);
            Some((base.to_string(), v.clone()))
        })
        .collect();
    for (alias, version) in aliases {
        tools.entry(alias).or_insert(version);
    }
    tools
}

/// Returns true if the check's tool is declared in mise.toml and its version
/// satisfies the check's version_range (if any).
pub fn check_active(check: &Check, mise_tools: &HashMap<String, String>) -> bool {
    if check.activate_unconditionally {
        return true;
    }
    let Some(declared) = declared_tool_version(check, mise_tools) else {
        return false;
    };
    let Some(range_str) = check.version_range else {
        return true;
    };
    let Ok(req) = semver::VersionReq::parse(range_str) else {
        return false;
    };
    coerce_version(declared).is_some_and(|v| req.matches(&v))
}

pub fn tool_version_changed(
    check: &Check,
    previous_tools: &HashMap<String, String>,
    current_tools: &HashMap<String, String>,
) -> bool {
    let previous = declared_tool_version(check, previous_tools);
    let current = declared_tool_version(check, current_tools);
    previous.is_some() && current.is_some() && previous != current
}

fn declared_tool_version<'a>(
    check: &Check,
    mise_tools: &'a HashMap<String, String>,
) -> Option<&'a str> {
    if check.activate_unconditionally {
        return None;
    }
    let lookup_key = check.mise_tool_name.unwrap_or(check.bin_name);
    // When mise_tool_name is set (e.g. "cargo:yaml-lint"), also accept
    // the bare bin_name ("yaml-lint") so repos using either form work.
    mise_tools
        .get(lookup_key)
        .or_else(|| check.mise_tool_name.and(mise_tools.get(check.bin_name)))
        .map(String::as_str)
}

/// Parses a version string, padding with `.0` components if needed to satisfy
/// semver's three-part requirement (e.g. `"20"` → `20.0.0`, `"3.12"` → `3.12.0`).
fn coerce_version(s: &str) -> Option<semver::Version> {
    semver::Version::parse(s).ok().or_else(|| {
        let parts = s.split('.').count();
        match parts {
            1 => semver::Version::parse(&format!("{s}.0.0")).ok(),
            2 => semver::Version::parse(&format!("{s}.0")).ok(),
            _ => None,
        }
    })
}
