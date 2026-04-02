mod config;
mod files;
mod links;
mod registry;
mod renovate_deps;
mod runner;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "flint", about = "mise-native lint orchestrator")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<SubCommand>,

    /// Auto-fix issues instead of checking
    #[arg(long, env = "AUTOFIX")]
    fix: bool,

    /// Lint all files instead of only changed files
    #[arg(long)]
    full: bool,

    /// Skip slow checks
    #[arg(long)]
    fast: bool,

    /// Show all linter output, not just failures
    #[arg(long)]
    verbose: bool,

    /// Compact summary output — no per-check noise (human) or read-only AI review
    #[arg(long, env = "FLINT_SHORT")]
    short: bool,

    /// Autonomous mode: fix what's fixable, report what still needs review.
    /// Exits 0 if everything passed or was fixed. Intended for pre-push hooks
    /// and agentic pipelines that have write access.
    #[arg(long)]
    auto: bool,

    /// Compare changed files from this ref (default: merge base with base branch)
    #[arg(long)]
    from_ref: Option<String>,

    /// Compare changed files to this ref (default: HEAD)
    #[arg(long)]
    to_ref: Option<String>,

    /// Linters to run (default: all discovered)
    linters: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    /// List all available checks with their status
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_root = std::env::var("MISE_PROJECT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("cannot determine working directory"));

    std::env::set_current_dir(&project_root)?;

    let registry = registry::builtin();

    if let Some(SubCommand::List) = cli.command {
        print_list(&registry);
        return Ok(());
    }

    let cfg = config::load(&project_root)?;

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

    // Discover which checks have their tool available in PATH, and apply --fast filter.
    let active: Vec<&registry::Check> = checks
        .into_iter()
        .filter(|c| which::which(c.bin()).is_ok())
        .filter(|c| !cli.fast || !c.slow)
        .collect();

    let file_list = files::changed(
        &project_root,
        &cfg,
        cli.full,
        cli.from_ref.as_deref(),
        cli.to_ref.as_deref(),
    )?;

    if cli.auto {
        // Run checks, fix what's fixable, report outcome.
        // Exits 0 if everything passed or was fixed; 1 if anything still needs review.
        let check_results = runner::run(
            &active,
            &file_list,
            false,
            false,
            true, // suppress per-check output
            &project_root,
            &cfg,
        )
        .await?;

        let (fixable_names, reviewable): (Vec<&str>, Vec<&str>) = check_results
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(name, _)| name.as_str())
            .partition(|name| active.iter().any(|c| c.name == *name && c.has_fix()));

        let mut fixed = vec![];
        let mut fix_failed = vec![];
        if !fixable_names.is_empty() {
            let to_fix: Vec<&registry::Check> = active
                .iter()
                .filter(|c| fixable_names.contains(&c.name))
                .copied()
                .collect();
            let fix_results = runner::run(
                &to_fix,
                &file_list,
                true,
                false,
                true, // suppress per-check output
                &project_root,
                &cfg,
            )
            .await?;
            for (name, ok) in fix_results {
                if ok {
                    fixed.push(name);
                } else {
                    fix_failed.push(name);
                }
            }
        }

        let remaining: Vec<&str> = reviewable
            .iter()
            .copied()
            .chain(fix_failed.iter().map(String::as_str))
            .collect();

        let mut segments = vec![];
        if !fixed.is_empty() {
            // Exit 1 even when fixes were applied: in a pre-push context the
            // fixed files are uncommitted. The caller must commit them first.
            segments.push(format!(
                "fixed: {} — commit before pushing",
                fixed.join(", ")
            ));
        }
        if !remaining.is_empty() {
            segments.push(format!("review: {}", remaining.join(", ")));
        }
        if !segments.is_empty() {
            eprintln!("flint: {}", segments.join(" | "));
            std::process::exit(1);
        }
        return Ok(());
    }

    let results = runner::run(
        &active,
        &file_list,
        cli.fix,
        cli.verbose,
        cli.short,
        &project_root,
        &cfg,
    )
    .await?;

    let failed: Vec<&str> = results
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| name.as_str())
        .collect();

    if !failed.is_empty() {
        let n = failed.len();
        let noun = if n == 1 { "check" } else { "checks" };
        if cli.short {
            // Partition by fixability. Emit the exact command for fixable checks
            // so AI callers can act without a reasoning step.
            let (fixable, reviewable): (Vec<&str>, Vec<&str>) = failed
                .iter()
                .copied()
                .partition(|name| active.iter().any(|c| c.name == *name && c.has_fix()));
            let mut segments = vec![];
            if !fixable.is_empty() {
                segments.push(format!("flint --fix {}", fixable.join(" ")));
            }
            if !reviewable.is_empty() {
                segments.push(format!("review: {}", reviewable.join(", ")));
            }
            eprintln!("flint: {n} {noun} failed — {}", segments.join(" | "));
        } else {
            eprintln!(
                "\nflint: {n} {noun} failed ({names})",
                names = failed.join(", ")
            );
            if !cli.fix {
                eprintln!(
                    "💡 Try `mise run fix` to auto-fix lint issues, then re-run `mise run lint` to verify."
                );
            }
        }
        std::process::exit(1);
    }

    Ok(())
}

fn print_list(registry: &[registry::Check]) {
    // Column widths.
    let name_w = registry
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let bin_w = registry
        .iter()
        .map(|c| c.bin().len())
        .max()
        .unwrap_or(6)
        .max(6);

    println!(
        "{:<name_w$}  {:<bin_w$}  {:<9}  {:<4}  PATTERNS",
        "NAME",
        "BINARY",
        "STATUS",
        "SPEED",
        name_w = name_w,
        bin_w = bin_w,
    );
    println!("{}", "-".repeat(name_w + bin_w + 35));

    for check in registry {
        let status = if which::which(check.bin()).is_ok() {
            "installed"
        } else {
            "missing"
        };
        let speed = if check.slow { "slow" } else { "fast" };
        println!(
            "{:<name_w$}  {:<bin_w$}  {:<9}  {:<4}  {}",
            check.name,
            check.bin(),
            status,
            speed,
            check.patterns,
            name_w = name_w,
            bin_w = bin_w,
        );
    }
}
