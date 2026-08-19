use super::snapshot::canonical_manager_name;

/// Reject deterministic GitHub Action digest lookup failures while leaving
/// transient lookup failures inconclusive.
pub(crate) fn validate_lookup_action_warnings(
    log_bytes: &[u8],
    exclude_managers: &[String],
) -> anyhow::Result<()> {
    const DETERMINISTIC: &str = "Could not determine new digest for update";

    if exclude_managers
        .iter()
        .map(|manager| canonical_manager_name(manager))
        .any(|manager| manager == "github-actions")
    {
        return Ok(());
    }

    for line in std::str::from_utf8(log_bytes)?.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !entry
            .get("msg")
            .and_then(|value| value.as_str())
            .is_some_and(|msg| msg == "packageFiles with updates")
        {
            continue;
        }
        let Some(package_files) = entry
            .get("packageFiles")
            .or_else(|| entry.get("config"))
            .and_then(|value| value.get("github-actions"))
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for package_file in package_files {
            let Some(deps) = package_file.get("deps").and_then(|value| value.as_array()) else {
                continue;
            };
            for dep in deps {
                let Some(warnings) = dep.get("warnings").and_then(|value| value.as_array()) else {
                    continue;
                };
                for warning in warnings {
                    let Some(message) = warning.get("message").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    if !message.starts_with(DETERMINISTIC) {
                        continue;
                    }
                    let dep_name = dep
                        .get("depName")
                        .or_else(|| dep.get("packageName"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown dependency");
                    anyhow::bail!(
                        "Renovate reported an invalid GitHub Action ref for {dep_name}: {message}"
                    );
                }
            }
        }
    }

    Ok(())
}
