use std::path::Path;
use std::path::PathBuf;

use crate::init::generation::{normalize_tools_section, tools_section_needs_normalization};
use crate::init::write_setup_migration_version;
use crate::linters::LinterOutput;
use crate::registry::{
    CheckTypeDef, NativeCheckDef, NativePrepareContext, NativeRunContext, NativeRunFuture,
    PreparedNativeCheck,
};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native(
    "flint-setup",
    NativeCheckDef::new(prepare).with_fix().setup(),
);

#[derive(Debug)]
struct PreparedFlintSetup {
    name: String,
    config_dir: PathBuf,
    setup_migration_version: u32,
    tracked_files: Vec<PathBuf>,
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    Some(Box::new(PreparedFlintSetup {
        name: ctx.name.to_string(),
        config_dir: ctx.config_dir.to_path_buf(),
        setup_migration_version: ctx.cfg.settings.setup_migration_version,
        tracked_files: vec![
            ctx.project_root.join("mise.toml"),
            ctx.config_dir.join("flint.toml"),
        ],
    }))
}

impl PreparedNativeCheck for PreparedFlintSetup {
    fn name(&self) -> &str {
        &self.name
    }

    fn tracked_files(&self) -> &[PathBuf] {
        &self.tracked_files
    }

    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture {
        Box::pin(async move {
            crate::linters::flint_setup::run(
                ctx.fix,
                &ctx.project_root,
                &self.config_dir,
                self.setup_migration_version,
            )
            .await
        })
    }
}

pub async fn run(
    fix: bool,
    project_root: &Path,
    config_dir: &Path,
    setup_migration_version: u32,
) -> LinterOutput {
    let path = project_root.join("mise.toml");
    let flint_toml = config_dir.join("flint.toml");
    let mut errors = vec![];
    let mut versioned_migrations_pending = false;

    if flint_toml.exists() && setup_migration_version > crate::setup::LATEST_SUPPORTED_SETUP_VERSION
    {
        errors.push(format!(
            "flint.toml setup_migration_version is {setup_migration_version}, but this flint only supports {}.",
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION
        ));
    } else if setup_migration_version < crate::setup::LATEST_SUPPORTED_SETUP_VERSION {
        match crate::init::detect_setup_migrations(
            project_root,
            config_dir,
            setup_migration_version,
        ) {
            Ok(true) => {
                versioned_migrations_pending = true;
                errors.push(format!(
                    "Flint setup migrations after version {setup_migration_version} apply to this repo."
                ));
            }
            Ok(false) => {}
            Err(e) => return LinterOutput::err(format!("flint: flint-setup: {e}\n")),
        }
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

    if flint_toml.exists() && setup_migration_version > crate::setup::LATEST_SUPPORTED_SETUP_VERSION
    {
        return LinterOutput::err(format!(
            "ERROR: {}\nUpgrade flint before changing this repo setup.\n",
            errors.join("\nERROR: ")
        ));
    }

    let migrations_applied = match crate::init::apply_setup_migrations(project_root, config_dir) {
        Ok(applied) => applied,
        Err(e) => return LinterOutput::err(format!("flint: flint-setup: {e}\n")),
    };
    if let Err(e) = normalize_tools_section(&path) {
        return LinterOutput::err(format!("flint: flint-setup: {e}\n"));
    }
    if migrations_applied
        && versioned_migrations_pending
        && flint_toml.exists()
        && let Err(e) = write_setup_migration_version(
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
    async fn missing_setup_migration_version_without_drift_passes() {
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

        assert!(out.ok);
    }

    #[tokio::test]
    async fn current_setup_migration_version_ignores_broad_cleanup_drift() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\n\n# Linters\nrumdl = \"0.1.78\"\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("flint.toml"),
            "[settings]\nsetup_migration_version = 2\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("README.md"),
            "<!-- markdownlint-disable MD013 -->\n# Title\n",
        )
        .unwrap();

        let out = run(
            false,
            tmp.path(),
            tmp.path(),
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION,
        )
        .await;

        assert!(out.ok, "{}", String::from_utf8_lossy(&out.stderr));
    }

    #[tokio::test]
    async fn fix_mode_does_not_apply_broad_repo_cleanup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let readme = "<!-- markdownlint-disable MD013 -->\n# Title\n";
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\nrumdl = \"0.1.78\"\nnode = \"24.0.0\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("README.md"), readme).unwrap();

        let out = run(
            true,
            tmp.path(),
            tmp.path(),
            crate::setup::LATEST_SUPPORTED_SETUP_VERSION,
        )
        .await;
        let content = std::fs::read_to_string(tmp.path().join("mise.toml")).unwrap();
        let readme_after = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();

        assert!(out.ok, "{}", String::from_utf8_lossy(&out.stderr));
        assert!(content.contains("# Linters"));
        assert_eq!(readme_after, readme);
    }

    #[tokio::test]
    async fn fix_mode_writes_current_setup_migration_version_when_migration_applies() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\n\"npm:markdownlint-cli2\" = \"0.18.1\"\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("flint.toml"),
            "[settings]\nsetup_migration_version = 1\n",
        )
        .unwrap();

        let out = run(
            true,
            tmp.path(),
            tmp.path(),
            crate::setup::V2_BASELINE_SETUP_VERSION,
        )
        .await;
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();

        assert!(out.ok);
        assert!(content.contains("setup_migration_version = 2"));
    }
}
