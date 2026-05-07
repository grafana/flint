use anyhow::Context;
use std::path::{Path, PathBuf};

const FLINT_PATCH_DIR: &str = "flint/renovate-local-dry-run";
const FLINT_LOADER_FILE: &str = "loader.mjs";
const FLINT_REGISTER_FILE: &str = "register.mjs";
pub(crate) const RENOVATE_LOCAL_PATCH_SENTINEL: &str = r#"dryRun: _params.dryRun ?? "lookup","#;
pub(crate) const RENOVATE_LOCAL_PATCH_REGEX: &str = r#"^(?<indent>\s*)dryRun:\s*"lookup",\s*$"#;

pub(crate) fn configure_extract_workaround_env(
    env: &mut Vec<(String, String)>,
    dry_run: &str,
) -> anyhow::Result<()> {
    if dry_run != "extract" {
        return Ok(());
    }

    let register_path = ensure_loader_files(&std::env::temp_dir().join(FLINT_PATCH_DIR))?;
    append_node_import(env, &register_path);
    Ok(())
}

fn ensure_loader_files(dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let loader_path = dir.join(FLINT_LOADER_FILE);
    let register_path = dir.join(FLINT_REGISTER_FILE);
    std::fs::write(&loader_path, loader_source())
        .with_context(|| format!("failed to write {}", loader_path.display()))?;
    std::fs::write(&register_path, register_source(&loader_path))
        .with_context(|| format!("failed to write {}", register_path.display()))?;

    Ok(register_path)
}

fn append_node_import(env: &mut Vec<(String, String)>, register_path: &Path) {
    let import_opt = format!("--import={}", file_url(register_path));
    let Some((_, node_options)) = env.iter_mut().find(|(key, _)| key == "NODE_OPTIONS") else {
        env.push(("NODE_OPTIONS".into(), import_opt));
        return;
    };

    if node_options.split_whitespace().any(|opt| opt == import_opt) {
        return;
    }

    if node_options.is_empty() {
        *node_options = import_opt;
    } else {
        node_options.push(' ');
        node_options.push_str(&import_opt);
    }
}

fn loader_source() -> String {
    format!(
        "const targetSuffix = \"/dist/modules/platform/local/index.js\";\n\
const patchSentinel = {};\n\
const patchRegex = /{}/m;\n\
\n\
export async function load(url, context, defaultLoad) {{\n\
  const result = await defaultLoad(url, context, defaultLoad);\n\
  if (!url.endsWith(targetSuffix)) return result;\n\
\n\
  const source = typeof result.source === \"string\"\n\
    ? result.source\n\
    : Buffer.from(result.source).toString(\"utf8\");\n\
  if (source.includes(patchSentinel)) return result;\n\
\n\
  const patched = source.replace(patchRegex, '$<indent>dryRun: _params.dryRun ?? \"lookup\",');\n\
  return {{ ...result, source: patched }};\n\
}}\n",
        serde_json::to_string(RENOVATE_LOCAL_PATCH_SENTINEL).unwrap(),
        RENOVATE_LOCAL_PATCH_REGEX,
    )
}

fn file_url(path: &Path) -> String {
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let raw = absolute.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !raw.starts_with('/') {
        out.push('/');
    }
    for b in raw.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn register_source(loader_path: &Path) -> String {
    format!(
        "import {{ register }} from 'node:module';\n\
import {{ pathToFileURL }} from 'node:url';\n\
\n\
register(pathToFileURL({}).href, import.meta.url);\n",
        serde_json::to_string(&loader_path.display().to_string()).unwrap(),
    )
}
