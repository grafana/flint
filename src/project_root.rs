use std::path::{Path, PathBuf};

pub(crate) fn detect() -> PathBuf {
    let cwd = std::env::current_dir().expect("cannot determine working directory");
    find(&cwd).unwrap_or(cwd)
}

fn find(start: &Path) -> Option<PathBuf> {
    let mut first_mise = None;

    for dir in start.ancestors() {
        let path = dir.join("mise.toml");
        if !path.is_file() {
            continue;
        }

        if first_mise.is_none() {
            first_mise = Some(dir.to_path_buf());
        }

        if mise_toml_declares_flint(&path) {
            return Some(dir.to_path_buf());
        }
    }

    first_mise
}

fn mise_toml_declares_flint(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&content) else {
        return false;
    };
    let Some(tools) = value.get("tools").and_then(toml::Value::as_table) else {
        return false;
    };

    tools
        .keys()
        .any(|key| crate::registry::is_flint_tool_key(key))
}

#[cfg(test)]
mod tests {
    use super::find;

    #[test]
    fn prefers_parent_mise_toml_that_declares_flint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let nested = root.join("apps/service/src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("mise.toml"), "[tools]\npython = '3.12'\n").unwrap();
        std::fs::write(
            root.join("apps/service/mise.toml"),
            "[tools]\n\"aqua:grafana/flint\" = '0.22.7'\n",
        )
        .unwrap();

        let found = find(&nested);

        assert_eq!(found, Some(root.join("apps/service")));
    }

    #[test]
    fn falls_back_to_first_parent_mise_toml_without_flint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let nested = root.join("apps/service/src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("mise.toml"), "[tools]\npython = '3.12'\n").unwrap();
        std::fs::write(
            root.join("apps/service/mise.toml"),
            "[tools]\nnode = '24'\n",
        )
        .unwrap();

        let found = find(&nested);

        assert_eq!(found, Some(root.join("apps/service")));
    }

    #[test]
    fn recognizes_cargo_flint_key_from_any_github_owner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let nested = root.join("apps/service/src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("mise.toml"), "[tools]\npython = '3.12'\n").unwrap();
        std::fs::write(
            root.join("apps/service/mise.toml"),
            "[tools]\n\"cargo:https://github.com/trask/flint\" = { version = \"branch:fix-lychee-windows-arg-limit\", crate = \"flint\", bin = \"flint\" }\n",
        )
        .unwrap();

        let found = find(&nested);

        assert_eq!(found, Some(root.join("apps/service")));
    }
}
