use anyhow::Context;
use std::path::Path;

use super::rules::{self, ExtractVersionMismatch};
use super::snapshot::Snapshot;

pub(crate) fn patch_semver_equivalent_mise_values(
    project_root: &Path,
    snapshot: &Snapshot,
    mismatches: &[ExtractVersionMismatch],
) -> anyhow::Result<bool> {
    let mut changed = false;

    for mismatch in mismatches {
        let Some(extracted_value) = mismatch.extracted_value.as_deref() else {
            continue;
        };
        if !rules::equivalent_version_shapes(extracted_value, &mismatch.current_value)
            || extracted_value == mismatch.current_value
        {
            continue;
        }

        for (file, managers) in &snapshot.files {
            let Some(deps) = managers.get("mise") else {
                continue;
            };
            if !deps.contains(&mismatch.dep_name) {
                continue;
            }
            changed |= patch_mise_tool_value(
                &project_root.join(file),
                &mismatch.dep_name,
                &mismatch.current_value,
                extracted_value,
            )?;
        }
    }

    Ok(changed)
}

fn patch_mise_tool_value(
    path: &Path,
    dep_name: &str,
    current_value: &str,
    preferred_value: &str,
) -> anyhow::Result<bool> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(tools) = doc.get_mut("tools").and_then(|item| item.as_table_mut()) else {
        return Ok(false);
    };
    let Some(item) = tools.get_mut(dep_name) else {
        return Ok(false);
    };

    let mut changed = false;
    match item.as_value_mut() {
        Some(toml_edit::Value::String(value)) if value.value() == current_value => {
            *item = toml_edit::value(preferred_value);
            changed = true;
        }
        Some(toml_edit::Value::InlineTable(table))
            if table.get("version").and_then(|value| value.as_str()) == Some(current_value) =>
        {
            table.insert("version", toml_edit::Value::from(preferred_value));
            changed = true;
        }
        _ => {}
    }

    if !changed {
        return Ok(false);
    }

    std::fs::write(path, doc.to_string())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}
