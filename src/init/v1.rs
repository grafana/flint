use anyhow::Result;
use std::path::Path;

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

#[cfg(test)]
mod tests {
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
