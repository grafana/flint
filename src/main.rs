mod config;
mod files;
mod hook;
mod init;
mod linters;
mod registry;
mod runner;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use registry::{CheckKind, Scope};
use runner::{CheckResult, RunOptions};
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "flint", about = "flint — fast lint")]
#[command(subcommand_required = true, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    /// Lint the code.
    Run(RunArgs),
    /// List available linters and their status.
    Linters(LintersArgs),
    /// Set up linters in mise.toml for this project.
    Init(InitArgs),
    /// Manage git hooks.
    Hook(HookArgs),
    /// Display the flint version.
    Version,
}

#[derive(Args, Debug)]
struct HookArgs {
    #[command(subcommand)]
    command: HookCommand,
}

#[derive(Subcommand, Debug)]
enum HookCommand {
    /// Install a pre-commit hook that runs `flint run --fix --fast-only`.
    Install,
}

#[derive(Args, Debug)]
struct LintersArgs {
    /// Output as JSON instead of the human-readable table.
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct InitArgs {
    /// Profile to configure: lang, default, or comprehensive.
    #[arg(long, value_enum)]
    profile: Option<init::Profile>,

    /// Apply changes without prompting for confirmation.
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Fix what's fixable, report what still needs review.
    /// Exits 1 if anything was fixed (uncommitted) or needs review; 0 if already clean.
    #[arg(long, env = "FLINT_FIX")]
    fix: bool,

    /// Lint all files instead of only changed files.
    #[arg(long, env = "FLINT_FULL")]
    full: bool,

    /// Run only fast linters. Overridden by explicitly named linters.
    #[arg(long, env = "FLINT_FAST_ONLY")]
    fast_only: bool,

    /// Show all linter output, not just failures.
    #[arg(long, env = "FLINT_VERBOSE")]
    verbose: bool,

    /// Compact summary output — no per-check noise (human) or read-only AI review.
    #[arg(long, env = "FLINT_SHORT")]
    short: bool,

    /// Show only new issues created after git revision REV
    /// (default: merge base with base branch).
    #[arg(long, value_name = "REV", env = "FLINT_NEW_FROM_REV")]
    new_from_rev: Option<String>,

    /// Compare changed files to this ref (default: HEAD).
    #[arg(long, value_name = "REF", env = "FLINT_TO_REF")]
    to_ref: Option<String>,

    /// Show how long each linter took to run.
    #[arg(long, env = "FLINT_TIME")]
    time: bool,

    /// Linters to run (default: all discovered). Explicit linters override --fast-only.
    linters: Vec<String>,
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

    match cli.command {
        SubCommand::Version => {
            println!("flint {}", env!("CARGO_PKG_VERSION"));
        }
        SubCommand::Linters(args) => {
            let cfg = config::load(&config_dir).unwrap_or_default();
            let mise_tools = registry::read_mise_tools(&project_root);
            if args.json {
                print_linters_json(&registry);
            } else {
                print_linters(&registry, &mise_tools, &cfg);
            }
        }
        SubCommand::Init(args) => {
            init::run(&project_root, args.profile, args.yes)?;
        }
        SubCommand::Hook(args) => match args.command {
            HookCommand::Install => hook::install(&project_root)?,
        },
        SubCommand::Run(args) => {
            run(args, &project_root, &config_dir, &registry).await?;
        }
    }

    Ok(())
}

async fn run(
    args: RunArgs,
    project_root: &std::path::Path,
    config_dir: &std::path::Path,
    registry: &[registry::Check],
) -> Result<()> {
    let cfg = config::load(config_dir)?;

    // Filter registry to requested linters (or all if none specified).
    // Explicit linter names override --fast-only (same behaviour as golangci-lint).
    let explicit = !args.linters.is_empty();
    let checks: Vec<&registry::Check> = if explicit {
        let mut out = vec![];
        for name in &args.linters {
            match registry.iter().find(|c| c.name == name.as_str()) {
                Some(c) => out.push(c),
                None => {
                    eprintln!("flint: unknown linter: {name}");
                    std::process::exit(1);
                }
            }
        }
        out
    } else {
        registry.iter().collect()
    };

    // Discover which checks are declared in the consuming repo's mise.toml, and apply
    // --fast-only filter (skipped when linters are named explicitly).
    // mise guarantees declared tools are on PATH, so no PATH check needed.
    let mise_tools = registry::read_mise_tools(project_root);
    let active: Vec<&registry::Check> = checks
        .into_iter()
        .filter(|c| registry::check_active(c, &mise_tools))
        .filter(|c| explicit || !args.fast_only || c.category != registry::Category::Slow)
        .collect();

    if args.verbose {
        let names: Vec<&str> = active.iter().map(|c| c.name).collect();
        if names.is_empty() {
            eprintln!("flint: no active linters");
        } else {
            eprintln!("flint: active linters: {}", names.join(", "));
        }
    }

    let file_list = files::changed(
        project_root,
        &cfg,
        args.full,
        args.new_from_rev.as_deref(),
        args.to_ref.as_deref(),
    )?;

    if args.fix {
        // Pre-check, fix what's fixable, report outcome.
        // Exits 0 if everything was already clean; 1 if anything was fixed (uncommitted)
        // or still needs review.
        let check_results = runner::run(
            &active,
            &file_list,
            RunOptions {
                fix: false,
                verbose: false,
                short: true,
                time: false,
            },
            project_root,
            &cfg,
            config_dir,
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
                    time: false,
                },
                project_root,
                &cfg,
                config_dir,
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
            fix: false,
            verbose: args.verbose,
            short: args.short,
            time: args.time,
        },
        project_root,
        &cfg,
        config_dir,
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
        if args.short {
            // Partition by fixability. Emit the exact command for fixable checks
            // so AI callers can act without a reasoning step.
            let (fixable, reviewable): (Vec<&str>, Vec<&str>) = failed
                .iter()
                .copied()
                .partition(|name| is_fixable(name, &active));
            let mut segments = vec![];
            if !fixable.is_empty() {
                segments.push(format!("flint run --fix {}", fixable.join(" ")));
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
            eprintln!(
                "💡 Try `flint run --fix` to auto-fix lint issues, then re-run `flint run` to verify."
            );
        }
        std::process::exit(1);
    }

    Ok(())
}

fn print_linters_json(registry: &[registry::Check]) {
    let entries: Vec<serde_json::Value> = registry.iter().map(linter_json).collect();
    println!("{}", serde_json::to_string_pretty(&entries).unwrap());
}

pub fn linter_json(check: &registry::Check) -> serde_json::Value {
    let scope = match &check.kind {
        CheckKind::Template { scope, .. } => match scope {
            Scope::File => "file",
            Scope::Files => "files",
            Scope::Project => "project",
        },
        CheckKind::Special(_) => "special",
    };
    let patterns: Vec<&str> = check.patterns.to_vec();
    let config_file: Option<&str> = check.linter_config.map(|(filename, _)| filename);
    serde_json::json!({
        "name": check.name,
        "description": check.desc,
        "binary": if check.uses_binary() { check.bin_name } else { "(built-in)" },
        "patterns": patterns,
        "fix": check.has_fix(),
        "slow": check.category == registry::Category::Slow,
        "scope": scope,
        "config_file": config_file,
    })
}

fn is_fixable(name: &str, active: &[&registry::Check]) -> bool {
    active.iter().any(|c| c.name == name && c.has_fix())
}

fn print_linters(
    registry: &[registry::Check],
    mise_tools: &HashMap<String, String>,
    cfg: &config::Config,
) {
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
    let desc_w = registry
        .iter()
        .map(|c| c.desc.len())
        .max()
        .unwrap_or(11)
        .max(11);

    println!(
        "{:<name_w$}  {:<bin_w$}  {:<13}  {:<4}  {:<3}  {:<desc_w$}  PATTERNS",
        "NAME",
        "BINARY",
        "STATUS",
        "SPEED",
        "FIX",
        "DESCRIPTION",
        name_w = name_w,
        bin_w = bin_w,
        desc_w = desc_w,
    );
    println!("{}", "-".repeat(name_w + bin_w + desc_w + 42));

    for check in registry {
        let status = if registry::check_active(check, mise_tools) {
            if !check.uses_binary() || registry::binary_on_path(check.bin_name) {
                if check.name == "license-header" && cfg.checks.license_header.text.is_empty() {
                    "not configured"
                } else {
                    "active"
                }
            } else {
                "no binary"
            }
        } else if mise_tools.contains_key(check.bin_name) {
            "wrong version"
        } else {
            "missing"
        };
        let speed = if check.category == registry::Category::Slow {
            "slow"
        } else {
            "fast"
        };
        let fix = if check.has_fix() { "yes" } else { "no" };
        let patterns_str = check.patterns.join(" ");
        if patterns_str.is_empty() {
            println!(
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<4}  {:<3}  {:<desc_w$}",
                check.name,
                check.bin_name,
                status,
                speed,
                fix,
                check.desc,
                name_w = name_w,
                bin_w = bin_w,
                desc_w = desc_w,
            );
        } else {
            println!(
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<4}  {:<3}  {:<desc_w$}  {}",
                check.name,
                check.bin_name,
                status,
                speed,
                fix,
                check.desc,
                patterns_str,
                name_w = name_w,
                bin_w = bin_w,
                desc_w = desc_w,
            );
        }
    }
}
