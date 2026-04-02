mod config;
mod files;
mod registry;
mod runner;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "flint", about = "mise-native lint orchestrator")]
struct Cli {
    /// Auto-fix issues instead of checking
    #[arg(long, env = "AUTOFIX")]
    fix: bool,

    /// Lint all files instead of only changed files
    #[arg(long)]
    full: bool,

    /// Compare changed files from this ref (default: merge base with base branch)
    #[arg(long)]
    from_ref: Option<String>,

    /// Compare changed files to this ref (default: HEAD)
    #[arg(long)]
    to_ref: Option<String>,

    /// Linters to run (default: all discovered)
    linters: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_root = std::env::var("MISE_PROJECT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("cannot determine working directory"));

    std::env::set_current_dir(&project_root)?;

    let cfg = config::load(&project_root)?;

    let registry = registry::builtin();

    // Filter registry to requested linters (or all if none specified).
    let checks: Vec<&registry::Check> = if cli.linters.is_empty() {
        registry.iter().collect()
    } else {
        let mut out = vec![];
        for name in &cli.linters {
            match registry.iter().find(|c| c.name == name.as_str()) {
                Some(c) => out.push(c),
                None => {
                    eprintln!("flint: unknown linter: {name}");
                    std::process::exit(1);
                }
            }
        }
        out
    };

    // Discover which checks have their tool available in PATH.
    let active: Vec<&registry::Check> = checks
        .into_iter()
        .filter(|c| which::which(c.bin()).is_ok())
        .collect();

    let file_list = files::changed(
        &project_root,
        &cfg,
        cli.full,
        cli.from_ref.as_deref(),
        cli.to_ref.as_deref(),
    )?;

    let results = runner::run(&active, &file_list, cli.fix, &project_root).await?;

    let mut failed = false;
    for (name, ok) in &results {
        if !ok {
            eprintln!("flint: {name} failed");
            failed = true;
        }
    }

    if failed {
        if !cli.fix {
            eprintln!(
                "\n💡 Try `mise run fix` to auto-fix lint issues, then re-run `mise run lint` to verify."
            );
        }
        std::process::exit(1);
    }

    Ok(())
}
