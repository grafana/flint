use std::path::Path;

use crate::init::generation::{normalize_tools_section, tools_section_needs_normalization};
use crate::init::write_setup_version;
use crate::linters::LinterOutput;

pub async fn run(
    fix: bool,
    project_root: &Path,
    config_dir: &Path,
    setup_version: u32,
) -> LinterOutput {
    let path = project_root.join("mise.toml");
    let flint_toml = config_dir.join("flint.toml");
    let mut errors = vec![];

    if flint_toml.exists() && setup_version > crate::setup::LATEST_SUPPORTED_SETUP_VERSION {
        errors.push(format!(
            "flint.toml setup_version is {setup_version}, but this flint only supports {}.",
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION
        ));
    } else if flint_toml.exists() && setup_version < crate::setup::LATEST_SUPPORTED_SETUP_VERSION {
        errors.push(format!(
            "flint.toml setup_version is {setup_version}, expected {}.",
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION
        ));
    }

    let mise_tools = crate::registry::read_mise_tools(project_root);
    if let Some((old, new)) = crate::registry::find_obsolete_key(&mise_tools) {
        errors.push(format!(
            "obsolete tool key in mise.toml: {old:?} (replaced by {new:?})."
        ));
    }
    if let Some((old, hint)) = crate::registry::find_unsupported_key(&mise_tools) {
        errors.push(format!(
            "unsupported legacy lint tool in mise.toml: {old:?}. Migration required: {hint}."
        ));
    }

    match tools_section_needs_normalization(&path) {
        Ok(true) => {
            errors.push("mise.toml [tools] entries are not in Flint's canonical order.".to_string())
        }
        Ok(false) => {}
        Err(e) => return LinterOutput::err(format!("flint: flint-setup: {e}\n")),
    }

    if errors.is_empty() {
        return LinterOutput {
            ok: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
    }

    if !fix {
        return LinterOutput::err(format!(
            "ERROR: {}\nRun `flint run --fix flint-setup` to apply Flint setup migrations.\n",
            errors.join("\nERROR: ")
        ));
    }

    if flint_toml.exists() && setup_version > crate::setup::LATEST_SUPPORTED_SETUP_VERSION {
        return LinterOutput::err(format!(
            "ERROR: {}\nUpgrade flint before changing this repo setup.\n",
            errors.join("\nERROR: ")
        ));
    }

    if let Err(e) = crate::init::apply_setup_migrations(project_root, config_dir, setup_version) {
        return LinterOutput::err(format!("flint: flint-setup: {e}\n"));
    }
    if let Err(e) = normalize_tools_section(&path) {
        return LinterOutput::err(format!("flint: flint-setup: {e}\n"));
    }
    if flint_toml.exists()
        && let Err(e) = write_setup_version(
            config_dir,
            "main",
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION,
        )
    {
        return LinterOutput::err(format!("flint: flint-setup: {e}\n"));
    }

    LinterOutput {
        ok: true,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn check_mode_reports_drift() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\nbiome = \"1\"\nnode = \"1\"\n",
        )
        .unwrap();

        let out = run(
            false,
            tmp.path(),
            tmp.path(),
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION,
        )
        .await;
        let content = std::fs::read_to_string(tmp.path().join("mise.toml")).unwrap();

        assert!(!out.ok);
        assert!(
            String::from_utf8(out.stderr)
                .unwrap()
                .contains("flint run --fix flint-setup")
        );
        assert!(content.contains("biome = \"1\""));
    }

    #[tokio::test]
    async fn fix_mode_normalizes_mise_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\nbiome = \"1\"\nnode = \"1\"\n",
        )
        .unwrap();

        let out = run(
            true,
            tmp.path(),
            tmp.path(),
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION,
        )
        .await;
        let content = std::fs::read_to_string(tmp.path().join("mise.toml")).unwrap();

        assert!(out.ok);
        assert!(content.contains("# Linters"));
        assert!(content.find("node =").unwrap() < content.find("# Linters").unwrap());
        assert!(content.find("# Linters").unwrap() < content.find("biome =").unwrap());
    }

    #[tokio::test]
    async fn missing_setup_version_baseline_reports_outdated() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mise.toml"), "[tools]\n").unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "[settings]\n").unwrap();

        let out = run(
            false,
            tmp.path(),
            tmp.path(),
            crate::setup::V2_BASELINE_SETUP_VERSION,
        )
        .await;

        assert!(!out.ok);
        assert!(
            String::from_utf8(out.stderr)
                .unwrap()
                .contains("setup_version is 1, expected 2")
        );
    }

    #[tokio::test]
    async fn fix_mode_writes_current_setup_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mise.toml"), "[tools]\n").unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "[settings]\n").unwrap();

        let out = run(
            true,
            tmp.path(),
            tmp.path(),
            crate::setup::V2_BASELINE_SETUP_VERSION,
        )
        .await;
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();

        assert!(out.ok);
        assert!(content.contains("setup_version = 2"));
    }
}
