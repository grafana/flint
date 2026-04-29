use anyhow::{Context, Result};
use std::path::Path;

/// Ensures `flint.toml` has the Renovate check config that init owns.
/// Returns `true` when the file was changed.
pub(super) fn ensure_renovate_deps_config(
    toml_path: &Path,
    exclude_managers: Option<&[String]>,
) -> Result<bool> {
    let content = std::fs::read_to_string(toml_path)
        .with_context(|| format!("failed to read {}", toml_path.display()))?;
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse flint.toml")?;
    let Some(checks) = doc.get("checks").and_then(|item| item.as_table()) else {
        return append_renovate_deps_config(toml_path, &content, exclude_managers);
    };
    let Some(table_key) = ["renovate-deps", "renovate_deps"]
        .into_iter()
        .find(|key| checks.contains_key(key))
    else {
        return append_renovate_deps_config(toml_path, &content, exclude_managers);
    };

    let managers = exclude_managers.unwrap_or_default();
    if managers.is_empty() {
        return Ok(false);
    }
    let renovate = doc
        .get_mut("checks")
        .and_then(|item| item.as_table_mut())
        .and_then(|checks| checks.get_mut(table_key))
        .and_then(|item| item.as_table_mut())
        .with_context(|| {
            format!(
                "[checks.{table_key}] is not a table in {}",
                toml_path.display()
            )
        })?;
    if renovate.contains_key("exclude_managers") {
        return Ok(false);
    }
    renovate.insert("exclude_managers", toml_edit::value(string_array(managers)));
    std::fs::write(toml_path, doc.to_string())
        .with_context(|| format!("failed to write {}", toml_path.display()))?;
    println!(
        "  patched {} — added checks.renovate-deps.exclude_managers",
        toml_path.display()
    );
    Ok(true)
}

fn append_renovate_deps_config(
    toml_path: &Path,
    content: &str,
    exclude_managers: Option<&[String]>,
) -> Result<bool> {
    let mut next = String::from(content);
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n[checks.renovate-deps]\n");
    match exclude_managers {
        Some(managers) if !managers.is_empty() => {
            next.push_str(&format!("exclude_managers = {}\n", string_array(managers)));
        }
        _ => next.push_str("# exclude_managers = []\n"),
    }
    std::fs::write(toml_path, next)
        .with_context(|| format!("failed to write {}", toml_path.display()))?;
    println!(
        "  patched {} — added checks.renovate-deps",
        toml_path.display()
    );
    Ok(true)
}

fn string_array(values: &[String]) -> toml_edit::Array {
    let mut array = toml_edit::Array::default();
    for value in values {
        array.push(value.as_str());
    }
    array
}

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

#[cfg(test)]
mod tests {
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
