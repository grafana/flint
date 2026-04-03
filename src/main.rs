mod config;
mod files;
mod linters;
mod registry;
mod runner;

use anyhow::Result;
use clap::{Parser, Subcommand};
use runner::{CheckResult, RunOptions};
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "flint", about = "flint — fast lint")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<SubCommand>,

    /// Auto-fix issues instead of checking
    #[arg(long)]
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

    let config_dir = std::env::var("FLINT_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| project_root.clone());

    std::env::set_current_dir(&project_root)?;

    let registry = registry::builtin();

    if let Some(SubCommand::List) = cli.command {
        let mise_tools = registry::read_mise_tools(&project_root);
        print_list(&registry, &mise_tools);
        return Ok(());
    }

    let cfg = config::load(&config_dir)?;

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

    // Discover which checks are declared in the consuming repo's mise.toml, and apply
    // --fast filter. mise guarantees declared tools are on PATH, so no PATH check needed.
    let mise_tools = registry::read_mise_tools(&project_root);
    let active: Vec<&registry::Check> = checks
        .into_iter()
        .filter(|c| registry::check_active(c, &mise_tools))
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
            RunOptions {
                fix: false,
                verbose: false,
                short: true,
            },
            &project_root,
            &cfg,
            &config_dir,
        )
        .await?;

        let (fixable, reviewable): (Vec<CheckResult>, Vec<CheckResult>) = check_results
            .into_iter()
            .filter(|r| !r.ok)
            .partition(|r| is_fixable(&r.name, &active));

        let mut fixed = vec![];
        let mut fix_failed = vec![];
        if !fixable.is_empty() {
            let fixable_names: Vec<&str> = fixable.iter().map(|r| r.name.as_str()).collect();
            let to_fix: Vec<&registry::Check> = active
                .iter()
                .filter(|c| fixable_names.contains(&c.name))
                .copied()
                .collect();
            let fix_results = runner::run(
                &to_fix,
                &file_list,
                RunOptions {
                    fix: true,
                    verbose: false,
                    short: true,
                },
                &project_root,
                &cfg,
                &config_dir,
            )
            .await?;
            for r in fix_results {
                if r.ok {
                    fixed.push(r.name);
                } else {
                    fix_failed.push(r.name);
                }
            }
        }

        // Emit linter output for checks that need manual review so the caller
        // has the failure details without a second flint invocation.
        for r in &reviewable {
            eprintln!("[{}]", r.name);
            if !r.stdout.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&r.stdout));
            }
            if !r.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&r.stderr));
            }
        }

        let remaining: Vec<&str> = reviewable
            .iter()
            .map(|r| r.name.as_str())
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
        RunOptions {
            fix: cli.fix,
            verbose: cli.verbose,
            short: cli.short,
        },
        &project_root,
        &cfg,
        &config_dir,
    )
    .await?;

    let failed: Vec<&str> = results
        .iter()
        .filter(|r| !r.ok)
        .map(|r| r.name.as_str())
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
                .partition(|name| is_fixable(name, &active));
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
                    "💡 Try `mise run lint:fix` to auto-fix lint issues, then re-run `mise run lint` to verify."
                );
            }
        }
        std::process::exit(1);
    }

    Ok(())
}

fn is_fixable(name: &str, active: &[&registry::Check]) -> bool {
    active.iter().any(|c| c.name == name && c.has_fix())
}

fn print_list(registry: &[registry::Check], mise_tools: &HashMap<String, String>) {
    // Column widths.
    let name_w = registry
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let bin_w = registry
        .iter()
        .map(|c| c.bin_name.len())
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
        let status = if registry::check_active(check, mise_tools) {
            "active"
        } else if mise_tools.contains_key(check.bin_name) {
            "wrong version"
        } else {
            "missing"
        };
        let speed = if check.slow { "slow" } else { "fast" };
        println!(
            "{:<name_w$}  {:<bin_w$}  {:<9}  {:<4}  {}",
            check.name,
            check.bin_name,
            status,
            speed,
            check.patterns.join(" "),
            name_w = name_w,
            bin_w = bin_w,
        );
    }
}
