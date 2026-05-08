use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::Component;
use std::path::{Path, PathBuf};

use crate::registry::{CheckTypeDef, InitHookContext};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::with_init_hook("typos", init);

pub(crate) fn legacy_config_present(project_root: &Path) -> bool {
    project_root.join(".codespellrc").exists()
}

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    let migration = migrate_legacy_config(ctx.project_root(), ctx.config_dir())?;
    if !migration.changed() {
        return Ok(false);
    }

    migration.print_messages();
    Ok(true)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct MigrationResult {
    pub target_path: Option<PathBuf>,
    pub wrote_target: bool,
    pub removed_files: Vec<PathBuf>,
}

impl MigrationResult {
    pub(crate) fn changed(&self) -> bool {
        self.wrote_target || !self.removed_files.is_empty()
    }

    pub(crate) fn print_messages(&self) {
        if self.wrote_target
            && let Some(target) = &self.target_path
        {
            println!("  wrote {}", target.display());
        }
        for path in &self.removed_files {
            println!("  removed {} (replaced by _typos.toml)", path.display());
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct LegacyCodespellConfig {
    ignore_words_list: BTreeSet<String>,
    ignore_words_file: Option<PathBuf>,
}

pub(crate) fn migrate_legacy_config(
    project_root: &Path,
    config_dir: &Path,
) -> Result<MigrationResult> {
    let codespell_path = project_root.join(".codespellrc");
    if !codespell_path.exists() {
        return Ok(MigrationResult::default());
    }

    let legacy = parse_codespell_config(&codespell_path)?;
    let ignore_words_path =
        resolve_ignore_words_path(project_root, legacy.ignore_words_file.as_deref())?;
    let imported_words = load_ignore_words(ignore_words_path.as_deref())?;
    let mut all_words = legacy.ignore_words_list;
    all_words.extend(imported_words);

    let target = config_dir.join("_typos.toml");
    merge_typos_config(&target, &all_words)?;

    let mut removed_files = vec![];
    std::fs::remove_file(&codespell_path)
        .with_context(|| format!("failed to remove {}", codespell_path.display()))?;
    removed_files.push(codespell_path);
    if let Some(path) = ignore_words_path
        && path.exists()
    {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
        removed_files.push(path);
    }

    Ok(MigrationResult {
        target_path: Some(target),
        wrote_target: true,
        removed_files,
    })
}

fn parse_codespell_config(path: &Path) -> Result<LegacyCodespellConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut config = LegacyCodespellConfig::default();
    let mut in_codespell_section = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_codespell_section = line == "[codespell]";
            continue;
        }
        if !in_codespell_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "ignore-words-list" => {
                config.ignore_words_list.extend(parse_csv_list(value));
            }
            "ignore-words" if !value.is_empty() => {
                config.ignore_words_file = Some(PathBuf::from(value));
            }
            _ => {}
        }
    }

    Ok(config)
}

fn parse_csv_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn resolve_ignore_words_path(project_root: &Path, path: Option<&Path>) -> Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if path.is_absolute() {
        anyhow::bail!(
            "legacy .codespellrc ignore-words path must stay within the repository: {}",
            path.display()
        );
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            anyhow::bail!(
                "legacy .codespellrc ignore-words path must stay within the repository: {}",
                path.display()
            );
        }
    }
    Ok(Some(project_root.join(path)))
}

fn load_ignore_words(path: Option<&Path>) -> Result<BTreeSet<String>> {
    let Some(absolute) = path else {
        return Ok(BTreeSet::new());
    };
    if !absolute.exists() {
        return Ok(BTreeSet::new());
    }
    let content = std::fs::read_to_string(absolute)
        .with_context(|| format!("failed to read {}", absolute.display()))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

fn merge_typos_config(target: &Path, words: &BTreeSet<String>) -> Result<()> {
    let content = match std::fs::read_to_string(target) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", target.display()));
        }
    };
    let mut doc: toml_edit::DocumentMut = if content.is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        content
            .parse()
            .with_context(|| format!("failed to parse {}", target.display()))?
    };

    if !words.is_empty() {
        let default = ensure_table(doc.as_table_mut(), "default");
        let extend_words = ensure_table(default, "extend-words");
        for word in words {
            if !extend_words.contains_key(word) {
                extend_words.insert(word, toml_edit::value(word.as_str()));
            }
        }
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let rendered = normalize_rendered_typos_config(doc.to_string());
    std::fs::write(target, rendered)
        .with_context(|| format!("failed to write {}", target.display()))?;
    Ok(())
}

fn normalize_rendered_typos_config(rendered: String) -> String {
    rendered.replace(
        "[default]\n\n[default.extend-words]\n",
        "[default.extend-words]\n",
    )
}

fn ensure_table<'a>(parent: &'a mut toml_edit::Table, key: &str) -> &'a mut toml_edit::Table {
    if !parent.contains_key(key) {
        parent.insert(key, toml_edit::Item::Table(toml_edit::Table::new()));
    }
    parent
        .get_mut(key)
        .and_then(toml_edit::Item::as_table_mut)
        .expect("table just inserted or already present")
}

#[cfg(test)]
mod tests {
    use super::{
        load_ignore_words, merge_typos_config, migrate_legacy_config, parse_codespell_config,
        resolve_ignore_words_path,
    };
    use std::collections::BTreeSet;

    #[test]
    fn parse_codespell_config_reads_common_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".codespellrc");
        std::fs::write(
            &path,
            "[codespell]\nignore-words-list = ratatui, re-use\nignore-words = .codespellignore\nskip = .git,target\ncheck-hidden =\ninteractive = 1\n",
        )
        .unwrap();

        let parsed = parse_codespell_config(&path).unwrap();
        assert!(parsed.ignore_words_list.contains("ratatui"));
        assert!(parsed.ignore_words_list.contains("re-use"));
        assert_eq!(
            parsed.ignore_words_file.as_deref(),
            Some(std::path::Path::new(".codespellignore"))
        );
    }

    #[test]
    fn load_ignore_words_skips_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".codespellignore");
        std::fs::write(&path, "# comment\nratatui\n\nre-use\n").unwrap();

        let absolute =
            resolve_ignore_words_path(tmp.path(), Some(std::path::Path::new(".codespellignore")))
                .unwrap();
        let words = load_ignore_words(absolute.as_deref()).unwrap();
        assert!(words.contains("ratatui"));
        assert!(words.contains("re-use"));
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn migrate_legacy_config_writes_typos_config_and_removes_legacy_files() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".github/config");
        std::fs::write(
            tmp.path().join(".codespellrc"),
            "[codespell]\nignore-words-list = ratatui\nignore-words = .codespellignore\nskip = .git,target\ncheck-hidden =\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join(".codespellignore"), "flat\n").unwrap();

        let result = migrate_legacy_config(tmp.path(), &config_dir).unwrap();
        assert!(result.changed());
        assert!(!tmp.path().join(".codespellrc").exists());
        assert!(!tmp.path().join(".codespellignore").exists());

        let content = std::fs::read_to_string(config_dir.join("_typos.toml")).unwrap();
        assert!(content.contains("ratatui = \"ratatui\""));
        assert!(content.contains("flat = \"flat\""));
        assert!(!content.contains("extend-exclude"));
        assert!(!content.contains("ignore-hidden"));
        assert!(!content.contains("[default]\n\n[default.extend-words]"));
    }

    #[test]
    fn migrate_legacy_config_rejects_parent_dir_ignore_words_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".codespellrc"),
            "[codespell]\nignore-words = ../outside\n",
        )
        .unwrap();

        let err = migrate_legacy_config(tmp.path(), tmp.path()).unwrap_err();

        assert!(
            err.to_string().contains("must stay within the repository"),
            "{err:#}"
        );
        assert!(tmp.path().join(".codespellrc").exists());
        assert!(!tmp.path().join("_typos.toml").exists());
    }

    #[test]
    fn merge_typos_config_fails_on_unreadable_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("_typos.toml");
        std::fs::create_dir(&target).unwrap();

        let err = merge_typos_config(&target, &BTreeSet::new()).unwrap_err();

        assert!(err.to_string().contains("failed to read"));
    }
}
