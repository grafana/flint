#[path = "../readme_snippets.rs"]
mod readme_snippets;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use toml::Value;

fn main() -> Result<()> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = repo_root.join("README.md");
    let mise_path = repo_root.join("mise.toml");

    let mut readme = fs::read_to_string(&readme_path).context("read README.md")?;
    let mise = fs::read_to_string(&mise_path).context("read mise.toml")?;
    let mise: Value = toml::from_str(&mise).context("parse mise.toml")?;
    let tools = mise["tools"]
        .as_table()
        .context("mise.toml must contain [tools]")?;

    let install_block = format!(
        "[tools]\n\"github:grafana/flint\" = \"{}\"",
        env!("CARGO_PKG_VERSION")
    );
    replace_fenced_block(
        &mut readme,
        readme_snippets::INSTALL_MARKER,
        "toml",
        &install_block,
    )?;

    let quickstart_block = render_quickstart_tools(tools)?;
    replace_fenced_block(
        &mut readme,
        readme_snippets::QUICKSTART_MARKER,
        "toml",
        &quickstart_block,
    )?;

    fs::write(&readme_path, readme).context("write README.md")?;
    Ok(())
}

fn tool_versions(table: &toml::Table, keys: &[&str]) -> Result<BTreeMap<String, String>> {
    keys.iter()
        .map(|key| {
            let value = table
                .get(*key)
                .with_context(|| format!("missing tool key {key:?} in mise.toml"))?;
            let version = value
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| {
                    value
                        .as_table()
                        .and_then(|t| t.get("version"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .with_context(|| format!("tool key {key:?} must have a string version"))?;
            Ok(((*key).to_string(), version))
        })
        .collect()
}

fn render_quickstart_tools(table: &toml::Table) -> Result<String> {
    let versions = tool_versions(table, readme_snippets::QUICKSTART_KEYS)?;
    Ok(format!(
        "[tools]\n\
\"github:grafana/flint\" = \"{flint}\"\n\
\n\
# Add whichever linters apply to your repo:\n\
\"github:koalaman/shellcheck\" = \"{shellcheck}\"\n\
shfmt                   = \"{shfmt}\"\n\
actionlint              = \"{actionlint}\"\n\
",
        flint = env!("CARGO_PKG_VERSION"),
        shellcheck = versions["github:koalaman/shellcheck"],
        shfmt = versions["shfmt"],
        actionlint = versions["actionlint"],
    ))
}

fn replace_fenced_block(
    haystack: &mut String,
    marker: &str,
    lang: &str,
    replacement: &str,
) -> Result<()> {
    let marker_pos = haystack
        .find(marker)
        .with_context(|| format!("missing marker {marker:?}"))?;
    let after_marker = marker_pos + marker.len();
    let fence = format!("```{lang}\n");
    let rel_start = haystack[after_marker..]
        .find(&fence)
        .with_context(|| format!("missing {lang} fenced block after {marker:?}"))?;
    let block_start = after_marker + rel_start + fence.len();
    let rel_end = haystack[block_start..]
        .find("\n```")
        .with_context(|| format!("missing closing fence after {marker:?}"))?;
    let block_end = block_start + rel_end;
    if replacement.contains("```") {
        bail!("replacement block cannot contain code fences");
    }
    haystack.replace_range(block_start..block_end, replacement);
    Ok(())
}
