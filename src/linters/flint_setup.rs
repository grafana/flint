use std::path::Path;
use std::path::PathBuf;

use crate::init::generation::{needs_aube_for_renovate, needs_node_for_npm};
use crate::init::generation::{normalize_tools_section, tools_section_needs_normalization};
use crate::linters::LinterOutput;
use crate::registry::{
    CheckTypeDef, NativeCheckDef, NativePrepareContext, NativeRunContext, NativeRunFuture,
    PreparedNativeCheck, SetupOutcome,
};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native(
    "flint-setup",
    NativeCheckDef::new(prepare).with_fix().setup(),
);

#[derive(Debug)]
struct PreparedFlintSetup {
    name: String,
    config_dir: PathBuf,
    tracked_files: Vec<PathBuf>,
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    Some(Box::new(PreparedFlintSetup {
        name: ctx.name.to_string(),
        config_dir: ctx.config_dir.to_path_buf(),
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
            crate::linters::flint_setup::run(ctx.fix, &ctx.project_root, &self.config_dir).await
        })
    }
}

pub async fn run(fix: bool, project_root: &Path, config_dir: &Path) -> LinterOutput {
    let path = project_root.join("mise.toml");
    let mut errors = vec![];
    let mut setup_outcome = SetupOutcome::Clean;

    let mise_content = std::fs::read_to_string(&path).unwrap_or_default();
    let mise_tools = crate::registry::read_mise_tools(project_root);
    if let Some((old, new)) = crate::registry::find_obsolete_key(&mise_tools) {
        setup_outcome = setup_outcome.at_least(SetupOutcome::Blocking);
        errors.push(format!(
            "obsolete tool key in mise.toml: {old:?} (replaced by {new:?})."
        ));
    }
    if let Some((old, hint)) = crate::registry::find_unsupported_key(&mise_tools) {
        setup_outcome = setup_outcome.at_least(SetupOutcome::Blocking);
        errors.push(format!(
            "unsupported legacy lint tool in mise.toml: {old:?}. Migration required: {hint}."
        ));
    }
    if needs_node_for_npm(&mise_content) {
        setup_outcome = setup_outcome.at_least(SetupOutcome::Blocking);
        errors.push("mise.toml is missing `node` for npm: backend tools.".to_string());
    }
    if needs_aube_for_renovate(&mise_content) {
        setup_outcome = setup_outcome.at_least(SetupOutcome::Blocking);
        errors.push(
            "mise.toml has npm:renovate but is missing `aube` or `allow_builds = [\"re2\"]` — \
             run `flint run --fix flint-setup` to add them."
                .to_string(),
        );
    }

    match tools_section_needs_normalization(&path) {
        Ok(true) => {
            setup_outcome = setup_outcome.at_least(SetupOutcome::NonBlocking);
            errors.push("mise.toml [tools] entries are not in Flint's canonical order.".to_string())
        }
        Ok(false) => {}
        Err(e) => {
            return LinterOutput::setup_err(
                SetupOutcome::Fatal,
                format!("flint: flint-setup: {e}\n"),
            );
        }
    }

    let setup_migrations_pending = match crate::init::detect_setup_migrations(project_root) {
        Ok(pending) => pending,
        Err(e) => {
            return LinterOutput::setup_err(
                SetupOutcome::Fatal,
                format!("flint: flint-setup: {e}\n"),
            );
        }
    };
    if setup_migrations_pending {
        setup_outcome = setup_outcome.at_least(SetupOutcome::Blocking);
        errors.push("legacy Flint setup files or config need migration.".to_string());
    }

    if errors.is_empty() {
        return LinterOutput {
            ok: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
            setup_outcome: Some(SetupOutcome::Clean),
        };
    }

    if !fix {
        return LinterOutput {
            ok: false,
            stdout: Vec::new(),
            stderr: format!(
                "ERROR: {}\nRun `flint run --fix flint-setup` to apply Flint setup migrations.\n",
                errors.join("\nERROR: ")
            )
            .into_bytes(),
            setup_outcome: Some(setup_outcome),
        };
    }

    let _migrations_applied = match crate::init::apply_setup_migrations(project_root, config_dir) {
        Ok(applied) => applied,
        Err(e) => {
            return LinterOutput::setup_err(
                SetupOutcome::Fatal,
                format!("flint: flint-setup: {e}\n"),
            );
        }
    };
    if let Err(e) = normalize_tools_section(&path) {
        return LinterOutput::setup_err(SetupOutcome::Fatal, format!("flint: flint-setup: {e}\n"));
    }

    LinterOutput {
        ok: true,
        stdout: Vec::new(),
        stderr: Vec::new(),
        setup_outcome: Some(setup_outcome),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[tokio::test]
    async fn check_mode_reports_drift() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\nbiome = \"1\"\nnode = \"1\"\n",
        )
        .unwrap();

        let out = run(false, tmp.path(), tmp.path()).await;
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

        let out = run(true, tmp.path(), tmp.path()).await;
        let content = std::fs::read_to_string(tmp.path().join("mise.toml")).unwrap();

        assert!(out.ok);
        assert!(content.contains("# Linters"));
        assert!(content.find("node =").unwrap() < content.find("# Linters").unwrap());
        assert!(content.find("# Linters").unwrap() < content.find("biome =").unwrap());
    }

    #[tokio::test]
    async fn existing_flint_toml_without_setup_drift_passes() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mise.toml"), "[tools]\n").unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "[settings]\n").unwrap();

        let out = run(false, tmp.path(), tmp.path()).await;

        assert!(out.ok);
    }

    #[tokio::test]
    async fn setup_check_does_not_report_broad_repo_cleanup_drift() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\naube = \"1.0.0\"\nnode = \"24\"\n\n# Linters\n\"npm:renovate\" = { version = \"latest\", allow_builds = [\"re2\"] }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("README.md"),
            "<!-- markdownlint-disable MD013 -->\n# Title\n",
        )
        .unwrap();

        let out = run(false, tmp.path(), tmp.path()).await;

        assert!(out.ok, "{}", String::from_utf8_lossy(&out.stderr));
    }

    #[tokio::test]
    async fn fix_mode_applies_markdownlint_stack_cleanup() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tools]\nnode = \"24\"\n\n# Linters\nrumdl = \"0.1.78\"\n\"npm:markdownlint-cli2\" = \"0.18.1\"\n",
        )
        .unwrap();
        let readme = "<!-- markdownlint-disable MD013 -->\n# Title\n";
        std::fs::write(tmp.path().join("README.md"), readme).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .current_dir(tmp.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["add", "README.md", "mise.toml"])
                .current_dir(tmp.path())
                .status()
                .unwrap()
                .success()
        );

        let out = run(true, tmp.path(), tmp.path()).await;
        let content = std::fs::read_to_string(tmp.path().join("mise.toml")).unwrap();
        let readme_after = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();

        assert!(out.ok);
        assert!(!content.contains("npm:markdownlint-cli2"));
        assert_eq!(readme_after, "# Title\n");
    }
}
