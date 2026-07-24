use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::KubeLinterConfig;
use crate::linters::LinterOutput;
use crate::registry::{
    CheckTypeDef, NativeCheckDef, NativePrepareContext, NativeRunContext, NativeRunFuture,
    PreparedNativeCheck,
};
use serde::Deserialize;

/// Conventional directories are intentionally narrow. In particular, this
/// must not become a repository-wide `*.yaml` scan because Compose and other
/// application YAML files are not Kubernetes manifests.
const DEFAULT_MANIFEST_DIRECTORIES: &[&str] = &["k8s", "kubernetes", "manifests"];
const CONFIG_FILE_NAMES: &[&str] = &["kube-linter.yaml"];

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native(
    "kube-linter",
    NativeCheckDef::with_bin("kube-linter", prepare)
        .with_config_display("via `[checks.kube-linter]` in flint.toml"),
);

#[derive(Debug)]
struct PreparedKubeLinter {
    name: String,
    files: Vec<PathBuf>,
    config: Option<PathBuf>,
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    let cfg = &ctx.cfg.checks.kube_linter;
    let files = select_manifest_files(ctx.project_root, cfg);
    let config = find_config_file(ctx.config_dir, cfg);

    // Keep an empty prepared check so run() can report a successful no-op
    // instead of making an empty manifest set look like a missing check.
    Some(Box::new(PreparedKubeLinter {
        name: ctx.name.to_string(),
        files,
        config,
    }))
}

impl PreparedNativeCheck for PreparedKubeLinter {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture {
        Box::pin(async move { run(&ctx.project_root, &self.files, self.config.as_deref()).await })
    }
}

/// Select existing YAML files containing at least one Kubernetes-shaped YAML
/// document. Explicit paths take precedence over defaults; a directory is
/// traversed recursively, but only `.yaml`/`.yml` files are considered.
fn select_manifest_files(project_root: &Path, cfg: &KubeLinterConfig) -> Vec<PathBuf> {
    let roots: Vec<PathBuf> = if cfg.paths.is_empty() {
        DEFAULT_MANIFEST_DIRECTORIES
            .iter()
            .map(|path| project_root.join(path))
            .filter(|path| path.is_dir())
            .collect()
    } else {
        cfg.paths
            .iter()
            .map(|path| resolve_project_path(project_root, path))
            .collect()
    };

    let mut yaml_files = BTreeSet::new();
    for root in roots {
        collect_yaml_files(&root, &mut yaml_files);
    }

    yaml_files
        .into_iter()
        .filter(|path| path.is_file())
        .filter(|path| is_kubernetes_manifest(path))
        .collect()
}

fn resolve_project_path(project_root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn collect_yaml_files(path: &Path, files: &mut BTreeSet<PathBuf>) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };

    if metadata.file_type().is_symlink() || metadata.is_file() {
        if metadata.is_file() && is_yaml_path(path) {
            files.insert(path.to_path_buf());
        }
        return;
    }

    if !metadata.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_yaml_files(&entry.path(), files);
    }
}

fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "yaml" | "yml"))
}

fn is_kubernetes_manifest(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };

    serde_yaml_bw::Deserializer::from_str(&content).any(|document| {
        let Ok(value) = serde_yaml_bw::Value::deserialize(document) else {
            return false;
        };
        let serde_yaml_bw::Value::Mapping(mapping) = value else {
            return false;
        };
        has_mapping_key(&mapping, "apiVersion") && has_mapping_key(&mapping, "kind")
    })
}

fn has_mapping_key(mapping: &serde_yaml_bw::Mapping, expected: &str) -> bool {
    mapping.keys().any(|key| key.as_str() == Some(expected))
}

fn find_config_file(config_dir: &Path, cfg: &KubeLinterConfig) -> Option<PathBuf> {
    if let Some(config) = cfg.config.as_deref() {
        let configured = resolve_config_path(config_dir, config);
        if configured.is_file() {
            return Some(configured);
        }
    }

    CONFIG_FILE_NAMES
        .iter()
        .map(|name| config_dir.join(name))
        .find(|path| path.is_file())
}

fn resolve_config_path(config_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_dir.join(path)
    }
}

async fn run(project_root: &Path, files: &[PathBuf], config: Option<&Path>) -> LinterOutput {
    if files.is_empty() {
        return LinterOutput {
            ok: true,
            stdout: b"No Kubernetes manifests found; skipping kube-linter.\n".to_vec(),
            stderr: Vec::new(),
            setup_outcome: None,
        };
    }

    let mut argv = vec!["kube-linter".to_string(), "lint".to_string()];
    if let Some(config) = config {
        argv.push("--config".to_string());
        argv.push(path_for_command(project_root, config));
    }
    argv.extend(
        files
            .iter()
            .map(|path| path_for_command(project_root, path)),
    );

    let mut command = crate::linters::spawn_command(&argv, false);
    command
        .current_dir(project_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match command.output().await {
        Ok(output) => LinterOutput {
            ok: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
            setup_outcome: None,
        },
        Err(error) => LinterOutput {
            ok: false,
            stdout: Vec::new(),
            stderr: format!("flint: kube-linter: failed to spawn kube-linter: {error}\n")
                .into_bytes(),
            setup_outcome: None,
        },
    }
}

fn path_for_command(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn explicit_directories_select_kubernetes_documents_but_not_compose() {
        let project = tempfile::tempdir().unwrap();
        write(
            &project.path().join("manifests/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: app\n",
        );
        write(
            &project.path().join("manifests/compose.yml"),
            "services:\n  app:\n    image: example/app\n",
        );
        write(
            &project.path().join("manifests/values.yaml"),
            "replicaCount: 2\n",
        );

        let files = select_manifest_files(
            project.path(),
            &KubeLinterConfig {
                paths: vec!["manifests".into()],
                config: None,
            },
        );

        assert_eq!(
            files,
            vec![project.path().join("manifests/deployment.yaml")]
        );
    }

    #[test]
    fn multi_document_yaml_is_selected_when_any_document_is_kubernetes() {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("k8s/resources.yaml");
        write(
            &path,
            "---\nservices:\n  app:\n    image: example/app\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: app\n",
        );

        let files = select_manifest_files(
            project.path(),
            &KubeLinterConfig {
                paths: vec!["k8s/resources.yaml".into()],
                config: None,
            },
        );

        assert_eq!(files, vec![path]);
    }

    #[test]
    fn defaults_use_only_existing_conventional_directories() {
        let project = tempfile::tempdir().unwrap();
        write(
            &project.path().join("kubernetes/service.yml"),
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: app\n",
        );
        write(
            &project.path().join("unrelated/secret.yaml"),
            "apiVersion: v1\nkind: Secret\nmetadata:\n  name: secret\n",
        );

        let files = select_manifest_files(project.path(), &KubeLinterConfig::default());

        assert_eq!(files, vec![project.path().join("kubernetes/service.yml")]);
    }

    #[test]
    fn config_prefers_explicit_config_then_canonical_config_name() {
        let project = tempfile::tempdir().unwrap();
        let config_dir = project.path().join("config");
        write(&config_dir.join("custom.yaml"), "checks: {}\n");
        write(&config_dir.join("kube-linter.yaml"), "checks: {}\n");

        let explicit = find_config_file(
            &config_dir,
            &KubeLinterConfig {
                paths: Vec::new(),
                config: Some("custom.yaml".into()),
            },
        );
        assert_eq!(explicit, Some(config_dir.join("custom.yaml")));

        let conventional = find_config_file(&config_dir, &KubeLinterConfig::default());
        assert_eq!(conventional, Some(config_dir.join("kube-linter.yaml")));
    }

    #[test]
    fn missing_paths_and_config_are_a_noop() {
        let project = tempfile::tempdir().unwrap();
        let cfg = KubeLinterConfig {
            paths: vec!["does-not-exist".into()],
            config: Some("missing.yaml".into()),
        };

        assert!(select_manifest_files(project.path(), &cfg).is_empty());
        assert!(find_config_file(&project.path().join("config"), &cfg).is_none());
    }

    #[test]
    fn config_accepts_the_hyphenated_check_name_and_explicit_paths() {
        let config: crate::config::Config = toml::from_str(
            r#"
                [checks.kube-linter]
                paths = ["k8s", "manifests"]
            "#,
        )
        .unwrap();

        assert_eq!(
            config.checks.kube_linter.paths,
            vec!["k8s".to_string(), "manifests".to_string()]
        );
    }

    #[tokio::test]
    async fn no_selected_files_is_a_clean_noop() {
        let project = tempfile::tempdir().unwrap();
        let output = run(project.path(), &[], None).await;

        assert!(output.ok);
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8_lossy(&output.stdout).contains("skipping kube-linter"));
    }
}
