#[cfg(test)]
mod cli_docs_tests;
mod config;
mod files;
mod hook;
mod init;
mod linter_output;
mod linters;
mod project_root;
mod regions;
mod registry;
#[path = "main_run_policy.rs"]
mod run_policy;
mod runner;
mod setup;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use registry::CheckKind;
use runner::{CheckResult, RunContext as RunnerRunContext, RunOptions};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[path = "main_baseline.rs"]
mod baseline;

use baseline::*;

#[derive(Parser, Debug)]
#[command(name = "flint", bin_name = "flint", about = "flint — fast lint")]
#[command(subcommand_required = true, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    /// Lint the code.
    Run(RunArgs),
    /// Print the repository files selected by Flint's changed-file discovery.
    ChangedFiles(ChangedFilesArgs),
    /// Run a Flint-owned check against caller-selected files.
    Checker(CheckerArgs),
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
    /// Install a pre-commit hook that runs `flint run --fix`.
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
    #[arg(long, value_enum, conflicts_with = "only")]
    profile: Option<init::Profile>,

    /// Configure only the named checks. May be followed by multiple check names.
    #[arg(long, value_name = "CHECK", num_args = 1.., conflicts_with = "profile")]
    only: Vec<String>,

    /// Pin flint itself through cargo at this git revision for prerelease validation.
    #[arg(long, value_name = "REV")]
    flint_rev: Option<String>,

    /// Apply changes without prompting for confirmation.
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Fix what's fixable, report what still needs review.
    /// Exits 1 if anything was fixed (uncommitted) or needs review; 0 if already clean.
    /// Only 0 vs non-0 is stable for callers.
    #[arg(long, env = "FLINT_FIX")]
    fix: bool,

    /// In --fix mode, exit 0 when all reported issues were fixed successfully.
    /// Still exits non-zero when any check is partial or needs review.
    #[arg(long, env = "FLINT_ALLOW_FIXED", requires = "fix")]
    allow_fixed: bool,

    /// Lint all files instead of only changed files.
    #[arg(long, env = "FLINT_FULL")]
    full: bool,

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

    /// Linters to run (default: all discovered).
    /// Explicit names bypass the local relevance gate.
    linters: Vec<String>,
}

#[derive(Args, Debug)]
struct CheckerArgs {
    #[command(subcommand)]
    command: CheckerCommand,
}

#[derive(Subcommand, Debug)]
enum CheckerCommand {
    /// Verify Renovate's dependency snapshot for the supplied paths.
    RenovateDeps(RenovateDepsCheckerArgs),
    /// Check links in caller-selected files, plus repository-wide local links in CI.
    Lychee(LycheeCheckerArgs),
}

#[derive(Args, Debug)]
struct RenovateDepsCheckerArgs {
    /// Read newline-delimited repository-relative paths from PATH, or stdin for "-".
    /// The final line does not need a trailing newline.
    #[arg(long, value_name = "PATH", conflicts_with = "files")]
    files_from: Option<String>,

    /// Repository-relative paths selected by the caller.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

    /// Update the tracked dependency snapshot when it is stale.
    #[arg(long)]
    fix: bool,

    /// Show Renovate's diagnostic output.
    #[arg(long)]
    verbose: bool,
}

#[derive(Args, Debug)]
struct LycheeCheckerArgs {
    /// Read newline-delimited repository-relative paths from PATH, or stdin for "-".
    /// The final line does not need a trailing newline.
    #[arg(long, value_name = "PATH", conflicts_with = "files")]
    files_from: Option<String>,

    /// Repository-relative paths selected by the caller.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct ChangedFilesArgs {
    /// Select all eligible tracked files instead of only changed files.
    #[arg(long)]
    full: bool,

    /// Include paths matching GLOB. May be repeated; patterns are combined with OR.
    #[arg(long, value_name = "GLOB", action = clap::ArgAction::Append)]
    include: Vec<String>,

    /// Exclude paths matching GLOB. May be repeated; exclusions take precedence over includes.
    #[arg(long, value_name = "GLOB", action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Select changes after git revision REV (default: merge base with base branch).
    #[arg(long, value_name = "REV", env = "FLINT_NEW_FROM_REV")]
    new_from_rev: Option<String>,

    /// Compare changed files to this ref (default: HEAD).
    #[arg(long, value_name = "REF", env = "FLINT_TO_REF")]
    to_ref: Option<String>,

    /// Separate paths with NUL bytes instead of newlines.
    /// Use this when paths may contain whitespace or newlines.
    #[arg(long)]
    null: bool,
}

impl From<&RunArgs> for FixSummaryOptions {
    fn from(args: &RunArgs) -> Self {
        Self {
            allow_fixed: args.allow_fixed,
            short: args.short,
            verbose: args.verbose,
            time: args.time,
        }
    }
}

fn use_filtered_run_policy(args: &RunArgs, explicit: bool, is_ci: bool) -> bool {
    if explicit || args.full {
        return false;
    }

    !is_ci
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_root = project_root::detect();
    // Canonicalize to resolve symlinks (e.g. /private/... on macOS).
    // dunce::canonicalize strips the \\?\ verbatim prefix on Windows that
    // git and other tools don't handle.
    let project_root = dunce::canonicalize(&project_root).unwrap_or(project_root);

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
                linter_output::print_json(&registry, &mise_tools, &cfg);
            } else {
                linter_output::print_table(&registry, &mise_tools, &cfg);
            }
        }
        SubCommand::ChangedFiles(args) => {
            let cfg = config::load(&config_dir)?;
            let file_list = files::changed(
                &project_root,
                &cfg,
                args.full,
                args.new_from_rev.as_deref(),
                args.to_ref.as_deref(),
            )?;
            let file_list =
                files::apply_filters(&project_root, file_list, &args.include, &args.exclude)?;
            print_changed_files(&project_root, &file_list.files, args.null)?;
        }
        SubCommand::Checker(args) => {
            run_checker(args, &project_root, &config_dir).await?;
        }
        SubCommand::Init(args) => {
            if args.only.is_empty() {
                init::run(
                    &project_root,
                    args.profile,
                    args.yes,
                    args.flint_rev.as_deref(),
                )?;
            } else {
                init::run_with_only(
                    &project_root,
                    None,
                    &args.only,
                    args.yes,
                    args.flint_rev.as_deref(),
                )?;
            }
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

async fn run_checker(args: CheckerArgs, project_root: &Path, config_dir: &Path) -> Result<()> {
    let cfg = config::load(config_dir)?;
    let (name, out) = match args.command {
        CheckerCommand::RenovateDeps(args) => {
            let selected = read_checker_files(args.files_from.as_deref(), args.files)?;
            let file_list = checker_file_list(project_root, selected)?;
            (
                "renovate-deps",
                linters::renovate_deps::run_selected(
                    &cfg.checks.renovate_deps,
                    args.fix,
                    args.verbose,
                    project_root,
                    &file_list,
                )
                .await,
            )
        }
        CheckerCommand::Lychee(args) => {
            let selected = read_checker_files(args.files_from.as_deref(), args.files)?;
            let mut file_list = checker_file_list(project_root, selected)?;
            // The caller has already made a diff selection. Retain diff mode so
            // Flint's CI local-link follow-up still runs, without Git discovery.
            file_list.merge_base = Some("caller-selected".to_string());
            (
                "lychee",
                linters::lychee::run(
                    &cfg.checks.lychee,
                    &cfg.settings,
                    &file_list,
                    project_root,
                    config_dir,
                )
                .await,
            )
        }
    };
    let mut message = out.stdout;
    message.extend(out.stderr);
    let sarif = checker_sarif(name, out.ok, &message);
    serde_json::to_writer_pretty(std::io::stdout(), &sarif)?;
    println!();
    if !out.ok {
        std::process::exit(1);
    }
    Ok(())
}

fn read_checker_files(files_from: Option<&str>, files: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    match files_from {
        Some("-") => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            Ok(input
                .split('\n')
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect())
        }
        Some(path) => Ok(std::fs::read_to_string(path)?
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect()),
        None => Ok(files),
    }
}

fn checker_file_list(project_root: &Path, selected: Vec<PathBuf>) -> Result<files::FileList> {
    let mut files = Vec::with_capacity(selected.len());
    let mut changed_paths = Vec::with_capacity(selected.len());
    for path in selected {
        let path = if path.is_absolute() {
            path
        } else {
            project_root.join(path)
        };
        let relative = path.strip_prefix(project_root).with_context(|| {
            format!(
                "checker file is outside the project root: {}",
                path.display()
            )
        })?;
        changed_paths.push(
            relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
        );
        files.push(path);
    }
    Ok(files::FileList {
        files,
        changed_paths,
        merge_base: None,
        full: false,
    })
}

fn checker_sarif(name: &str, ok: bool, message: &[u8]) -> serde_json::Value {
    let results = if ok {
        vec![]
    } else {
        vec![serde_json::json!({
            "ruleId": name,
            "level": "error",
            "message": { "text": String::from_utf8_lossy(message) },
        })]
    };
    serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": { "name": "flint", "informationUri": "https://github.com/grafana/flint", "rules": [{ "id": name, "name": name }] } },
            "results": results,
        }],
    })
}

fn print_changed_files(project_root: &Path, files: &[PathBuf], nul: bool) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    write_changed_files(&mut stdout, project_root, files, nul)
}

fn write_changed_files<W: Write>(
    stdout: &mut W,
    project_root: &Path,
    files: &[PathBuf],
    nul: bool,
) -> Result<()> {
    let separator = if nul { "\0" } else { "\n" };
    for path in files {
        let relative = path
            .strip_prefix(project_root)
            .with_context(|| format!("file is outside project root: {}", path.display()))?;
        let relative = relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        if let Err(error) = stdout.write_all(relative.as_bytes()) {
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(error.into());
        }
        if let Err(error) = stdout.write_all(separator.as_bytes()) {
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(error.into());
        }
    }
    match stdout.flush() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn run(
    args: RunArgs,
    project_root: &std::path::Path,
    config_dir: &std::path::Path,
    registry: &[registry::Check],
) -> Result<()> {
    let cfg = config::load(config_dir)?;

    // Filter registry to requested linters (or all if none specified).
    // Explicit linter names bypass filtered local defaults (same behaviour as golangci-lint).
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

    let file_list = files::changed(
        project_root,
        &cfg,
        args.full,
        args.new_from_rev.as_deref(),
        args.to_ref.as_deref(),
    )?;
    // Discover which checks are declared in the consuming repo's mise.toml.
    // Outside CI, plain `flint run` relevance-gates checks that declare an
    // `adaptive_relevance` hook. Explicit linter names and `--full` bypass the
    // gate; CI always runs the full set.
    // mise guarantees declared tools are on PATH, so no PATH check needed.
    let mise_tools = registry::read_mise_tools(project_root);
    let is_ci = linters::env::is_ci_from(|name| std::env::var(name).ok());
    let use_filtered_policy = use_filtered_run_policy(&args, explicit, is_ci);
    let flint_setup_selected = checks.iter().any(|c| c.kind.is_setup());
    if !flint_setup_selected {
        if let Some((old, new)) = registry::find_obsolete_key(&mise_tools) {
            eprintln!("flint: obsolete tool key in mise.toml: {old:?} (replaced by {new:?})");
            eprintln!("  Run `flint run --fix flint-setup` to apply the migration automatically.");
            std::process::exit(1);
        }
        if let Some((old, hint)) = registry::find_unsupported_key(&mise_tools) {
            eprintln!("flint: unsupported legacy lint tool in mise.toml: {old:?}");
            eprintln!("  Migration required: {hint}.");
            eprintln!("  Run `flint init` to upgrade the lint toolchain.");
            std::process::exit(1);
        }
    }
    let mut eligible = vec![];
    for c in checks {
        if registry::check_active(c, &mise_tools) {
            eligible.push(c);
        } else if explicit {
            eprintln!(
                "flint: linter {name} is not active (binary not installed or not declared in mise.toml)",
                name = c.name
            );
            std::process::exit(1);
        }
    }

    // Baseline triggers must bypass adaptive local relevance. In particular, a
    // check-specific flint.toml change can be the only relevant changed file.
    let baseline_candidates =
        baseline_check_names(&eligible, &file_list, project_root, config_dir, &mise_tools);
    let relevance_ctx = AdaptiveRunContext {
        file_list: &file_list,
        project_root,
    };
    let active: Vec<&registry::Check> = eligible
        .into_iter()
        .filter(|c| {
            !use_filtered_policy
                || baseline_candidates.contains(c.name)
                || c.adaptive_relevance.is_none_or(|hook| hook(&relevance_ctx))
        })
        .collect();

    let mut setup_check_result = None;
    let mut setup_fix_outcome = None;
    let setup_check = active.iter().copied().find(|check| is_flint_setup(check));
    if let Some(check) = setup_check {
        let setup_results = run_checks(
            &[check],
            &file_list,
            None,
            &HashSet::new(),
            RunOptions {
                fix: args.fix,
                verbose: args.verbose,
                short: args.short,
                time: args.time,
            },
            ExecutionContext {
                active_checks: &active,
                project_root,
                cfg: &cfg,
                config_dir,
            },
        )
        .await?;
        let setup_result = setup_results
            .into_iter()
            .next()
            .expect("flint-setup preflight produced a result");
        if args.fix {
            let stop_after_setup = setup_result_blocks_fix(&setup_result);
            let setup_outcome = classify_single_pass_fix(setup_result);
            if stop_after_setup
                || matches!(
                    setup_outcome,
                    FixOutcome::Partial(_) | FixOutcome::Review(_)
                )
            {
                finish_fix_outcomes(vec![setup_outcome], (&args).into());
                return Ok(());
            } else {
                setup_fix_outcome = Some(setup_outcome);
            }
        } else if setup_result.ok {
            // Clean setup never affects later lint execution.
        } else if setup_result_blocks_check(&setup_result) {
            let failed = [setup_result.name.as_str()];
            if args.short {
                let command = mise_fix_command(project_root)
                    .unwrap_or_else(|| format!("flint run --fix {}", failed[0]));
                eprintln!("flint: 1 check failed — {command}");
            } else {
                eprintln!("\nflint: 1 check failed ({})", failed[0]);
                eprintln!(
                    "💡 Try `{}` to auto-fix lint issues, then re-run `flint run` to verify.",
                    fix_command(project_root)
                );
            }
            std::process::exit(1);
        } else {
            setup_check_result = Some(setup_result);
        }
    }
    let active: Vec<&registry::Check> = active
        .into_iter()
        .filter(|check| !is_flint_setup(check))
        .collect();

    if active.is_empty() {
        if let Some(outcome) = setup_fix_outcome {
            finish_fix_outcomes(vec![outcome], (&args).into());
        }
        if let Some(setup_result) = setup_check_result {
            finish_check_results(vec![setup_result], &active, args.short, project_root);
        }
        return Ok(());
    }

    if let Some((check, config)) = active.iter().find_map(|check| {
        unsupported_config(check, project_root, config_dir).map(|config| (*check, config))
    }) {
        let canonical = check
            .linter_config
            .as_ref()
            .map(run_policy::canonical_config_path)
            .or_else(|| {
                check
                    .baseline_config
                    .as_ref()
                    .map(|config| config_file_rel_path(project_root, config_dir, config))
            })
            .unwrap_or_else(|| "the flint-managed config".to_string());
        eprintln!(
            "flint: unsupported {name} config file found: {config}\n  Flint only supports {canonical} for {name}. Move the config to the supported location or remove the alternate file.",
            name = check.name
        );
        std::process::exit(1);
    }

    if args.verbose {
        let names: Vec<&str> = active.iter().map(|c| c.name).collect();
        if names.is_empty() {
            eprintln!("flint: no active linters");
        } else {
            eprintln!("flint: active linters: {}", names.join(", "));
        }
    }

    let baseline_names =
        baseline_check_names(&active, &file_list, project_root, config_dir, &mise_tools);
    let baseline_file_list = if baseline_names.is_empty() {
        None
    } else {
        Some(files::all(project_root, &cfg)?)
    };
    let run_ctx = ExecutionContext {
        active_checks: &active,
        project_root,
        cfg: &cfg,
        config_dir,
    };

    if args.fix {
        // Exits 0 if everything was already clean; 1 if anything was fixed (uncommitted)
        // or still needs review.
        let (single_pass_fixable, legacy_checks): (Vec<&registry::Check>, Vec<&registry::Check>) =
            active
                .iter()
                .copied()
                .partition(|c| run_policy::supports_single_pass_fix(c));

        let fix_summary: FixSummaryOptions = (&args).into();
        let mut outcomes = setup_fix_outcome.into_iter().collect::<Vec<_>>();

        if !legacy_checks.is_empty() {
            let check_results = run_checks(
                &legacy_checks,
                &file_list,
                baseline_file_list.as_ref(),
                &baseline_names,
                RunOptions {
                    fix: false,
                    verbose: false,
                    short: true,
                    time: false,
                },
                run_ctx,
            )
            .await?;

            outcomes.extend(
                check_results
                    .iter()
                    .filter(|r| r.ok)
                    .cloned()
                    .map(FixOutcome::Clean),
            );

            let (fixable, reviewable): (Vec<CheckResult>, Vec<CheckResult>) = check_results
                .into_iter()
                .filter(|r| !r.ok)
                .partition(|r| run_policy::is_fixable(&r.name, &legacy_checks));
            outcomes.extend(reviewable.into_iter().map(FixOutcome::Review));

            let mut to_verify = vec![];
            if !fixable.is_empty() {
                let fixable_names: Vec<&str> = fixable.iter().map(|r| r.name.as_str()).collect();
                let to_fix: Vec<&registry::Check> = legacy_checks
                    .iter()
                    .filter(|c| fixable_names.contains(&c.name))
                    .copied()
                    .collect();
                let fix_results = run_checks(
                    &to_fix,
                    &file_list,
                    baseline_file_list.as_ref(),
                    &baseline_names,
                    RunOptions {
                        fix: true,
                        verbose: false,
                        short: true,
                        time: false,
                    },
                    run_ctx,
                )
                .await?;
                for r in fix_results {
                    if r.ok {
                        if let Some(check) = legacy_checks.iter().find(|c| c.name == r.name) {
                            if check.fix_behavior() == registry::FixBehavior::PartialNeedsVerify {
                                to_verify.push(r.name);
                            } else if matches!(check.kind, CheckKind::Native(_)) {
                                outcomes.push(classify_single_pass_fix(r));
                            } else {
                                outcomes.push(FixOutcome::Fixed(r));
                            }
                        }
                    } else {
                        outcomes.push(FixOutcome::Partial(r));
                    }
                }
            }
            if !to_verify.is_empty() {
                let verify_names: Vec<&str> = to_verify.iter().map(String::as_str).collect();
                let to_verify_checks: Vec<&registry::Check> = legacy_checks
                    .iter()
                    .filter(|c| verify_names.contains(&c.name))
                    .copied()
                    .collect();
                let verify_results = run_checks(
                    &to_verify_checks,
                    &file_list,
                    baseline_file_list.as_ref(),
                    &baseline_names,
                    RunOptions {
                        fix: false,
                        verbose: false,
                        short: true,
                        time: false,
                    },
                    run_ctx,
                )
                .await?;
                for r in verify_results {
                    if r.ok {
                        outcomes.push(FixOutcome::Fixed(r));
                    } else {
                        outcomes.push(FixOutcome::Partial(r));
                    }
                }
            }
        }

        if !single_pass_fixable.is_empty() {
            let fix_results = run_checks(
                &single_pass_fixable,
                &file_list,
                baseline_file_list.as_ref(),
                &baseline_names,
                RunOptions {
                    fix: true,
                    verbose: false,
                    short: true,
                    time: false,
                },
                run_ctx,
            )
            .await?;
            for r in fix_results {
                outcomes.push(classify_single_pass_fix(r));
            }
        }

        finish_fix_outcomes(outcomes, fix_summary);
        return Ok(());
    }

    let mut results = run_checks(
        &active,
        &file_list,
        baseline_file_list.as_ref(),
        &baseline_names,
        RunOptions {
            fix: false,
            verbose: args.verbose,
            short: args.short,
            time: args.time,
        },
        run_ctx,
    )
    .await?;

    if let Some(setup_result) = setup_check_result {
        results.push(setup_result);
    }
    finish_check_results(results, &active, args.short, project_root);

    Ok(())
}

#[derive(Clone, Copy)]
struct ExecutionContext<'a> {
    active_checks: &'a [&'a registry::Check],
    project_root: &'a Path,
    cfg: &'a config::Config,
    config_dir: &'a Path,
}

#[derive(Clone, Copy)]
struct FixSummaryOptions {
    allow_fixed: bool,
    short: bool,
    verbose: bool,
    time: bool,
}

struct AdaptiveRunContext<'a> {
    file_list: &'a files::FileList,
    project_root: &'a Path,
}

impl registry::AdaptiveRelevanceContext for AdaptiveRunContext<'_> {
    fn file_list(&self) -> &files::FileList {
        self.file_list
    }

    fn project_root(&self) -> &Path {
        self.project_root
    }
}

enum FixOutcome {
    Clean(CheckResult),
    Fixed(CheckResult),
    Partial(CheckResult),
    Review(CheckResult),
}

impl FixOutcome {
    fn result(&self) -> Option<&CheckResult> {
        match self {
            Self::Clean(result)
            | Self::Fixed(result)
            | Self::Partial(result)
            | Self::Review(result) => Some(result),
        }
    }
}

fn finish_fix_outcomes(outcomes: Vec<FixOutcome>, opts: FixSummaryOptions) {
    let FixSummaryOptions {
        allow_fixed,
        short,
        verbose,
        time,
    } = opts;
    if !short {
        for r in outcomes.iter().filter_map(FixOutcome::result) {
            if verbose || !r.ok || time {
                eprintln!(
                    "[{}]{}",
                    r.name,
                    runner::format_duration_suffix(time, r.duration)
                );
            }
            if verbose || !r.ok {
                if !r.stdout.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&r.stdout));
                }
                if !r.stderr.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&r.stderr));
                }
            }
        }
    }

    let mut fixed = vec![];
    let mut partial = vec![];
    let mut review = vec![];
    for outcome in outcomes {
        match outcome {
            FixOutcome::Clean(_) => {}
            FixOutcome::Fixed(result) => fixed.push(result.name),
            FixOutcome::Partial(result) => partial.push(result.name),
            FixOutcome::Review(result) => review.push(result.name),
        }
    }
    fixed.sort();
    partial.sort();
    review.sort();
    let mut segments = vec![];
    if !fixed.is_empty() {
        // Exit 1 even when fixes were applied: in a pre-push context the fixed
        // files are uncommitted. The caller must commit them first.
        segments.push(format!(
            "fixed: {} — commit before pushing",
            fixed.join(", ")
        ));
    }
    if !partial.is_empty() {
        segments.push(format!("partial: {}", partial.join(", ")));
    }
    if !review.is_empty() {
        segments.push(format!("review: {}", review.join(", ")));
    }
    if !segments.is_empty() {
        eprintln!("flint: {}", segments.join(" | "));
        if !allow_fixed || !partial.is_empty() || !review.is_empty() {
            std::process::exit(1);
        }
    }
}

fn finish_check_results(
    results: Vec<CheckResult>,
    active: &[&registry::Check],
    short: bool,
    project_root: &Path,
) {
    let mut failed: Vec<&str> = results
        .iter()
        .filter(|r| !r.ok)
        .map(|r| r.name.as_str())
        .collect();
    failed.sort();

    if failed.is_empty() {
        return;
    }

    let n = failed.len();
    let noun = if n == 1 { "check" } else { "checks" };
    if short {
        // Partition by fixability. Emit the exact command for fixable checks
        // so AI callers can act without a reasoning step.
        let (fixable, reviewable): (Vec<&str>, Vec<&str>) = failed
            .iter()
            .copied()
            .partition(|name| run_policy::is_fixable(name, active));
        let mut segments = vec![];
        if !fixable.is_empty() {
            if let Some(command) = mise_fix_command(project_root) {
                segments.push(command);
            } else {
                segments.push(format!("flint run --fix {}", fixable.join(" ")));
            }
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
            "💡 Try `{}` to auto-fix lint issues, then re-run `flint run` to verify.",
            fix_command(project_root)
        );
    }
    std::process::exit(1);
}

/// Return the mise task that invokes Flint's fixer, when the consuming
/// repository declares one. This is deliberately best-effort: a malformed or
/// absent mise.toml must not turn a lint failure into a different failure.
fn fix_command(project_root: &Path) -> String {
    mise_fix_command(project_root).unwrap_or_else(|| "flint run --fix".to_string())
}

fn mise_fix_command(project_root: &Path) -> Option<String> {
    let path = project_root.join("mise.toml");
    let Ok(content) = std::fs::read_to_string(path) else {
        return None;
    };
    let Ok(document) = content.parse::<toml_edit::DocumentMut>() else {
        return None;
    };
    let tasks = document
        .get("tasks")
        .and_then(toml_edit::Item::as_table_like)?;
    tasks.iter().find_map(|(name, task)| {
        let run = task
            .as_table_like()
            .and_then(|task| task.get("run"))
            .and_then(toml_edit::Item::as_str);
        (run == Some("flint run --fix") && is_safe_mise_task_name(name))
            .then(|| format!("mise run {name}"))
    })
}

fn is_safe_mise_task_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-'))
}

#[cfg(test)]
mod fix_command_tests {
    use super::fix_command;

    #[test]
    fn finds_mise_fix_task() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("mise.toml"),
            "[tasks.\"lint:fix\"]\nrun = \"flint run --fix\"\n",
        )
        .unwrap();
        assert_eq!(fix_command(root.path()), "mise run lint:fix");
    }

    #[test]
    fn falls_back_for_unsafe_mise_fix_task_name() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("mise.toml"),
            "[tasks.\"lint fix\"]\nrun = \"flint run --fix\"\n",
        )
        .unwrap();
        assert_eq!(fix_command(root.path()), "flint run --fix");
    }
}

fn classify_single_pass_fix(result: CheckResult) -> FixOutcome {
    if result.ok {
        if result.changed {
            FixOutcome::Fixed(result)
        } else {
            FixOutcome::Clean(result)
        }
    } else if result.changed {
        FixOutcome::Partial(result)
    } else {
        FixOutcome::Review(result)
    }
}

fn setup_result_kind(result: &CheckResult) -> registry::SetupOutcome {
    result
        .setup_outcome
        .unwrap_or(registry::SetupOutcome::Fatal)
}

fn setup_result_blocks_check(result: &CheckResult) -> bool {
    matches!(
        setup_result_kind(result),
        registry::SetupOutcome::Blocking | registry::SetupOutcome::Fatal
    )
}

fn setup_result_blocks_fix(result: &CheckResult) -> bool {
    matches!(
        setup_result_kind(result),
        registry::SetupOutcome::Blocking | registry::SetupOutcome::Fatal
    )
}

fn is_flint_setup(check: &registry::Check) -> bool {
    check.kind.is_setup()
}

async fn run_checks(
    checks: &[&registry::Check],
    file_list: &files::FileList,
    baseline_file_list: Option<&files::FileList>,
    baseline_names: &HashSet<String>,
    opts: RunOptions,
    ctx: ExecutionContext<'_>,
) -> Result<Vec<CheckResult>> {
    let (baseline, normal): (Vec<_>, Vec<_>) = checks
        .iter()
        .copied()
        .partition(|c| baseline_names.contains(c.name));

    let mut results = vec![];
    if !normal.is_empty() {
        results.extend(
            runner::run(
                &normal,
                RunnerRunContext {
                    active_checks: ctx.active_checks,
                    file_list,
                    project_root: ctx.project_root,
                    cfg: ctx.cfg,
                    config_dir: ctx.config_dir,
                },
                opts,
            )
            .await?,
        );
    }
    if !baseline.is_empty() {
        let files = baseline_file_list.unwrap_or(file_list);
        results.extend(
            runner::run(
                &baseline,
                RunnerRunContext {
                    active_checks: ctx.active_checks,
                    file_list: files,
                    project_root: ctx.project_root,
                    cfg: ctx.cfg,
                    config_dir: ctx.config_dir,
                },
                opts,
            )
            .await?,
        );
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

/// Renders one linter's machine-readable metadata.
pub fn linter_json(
    check: &registry::Check,
    status: &str,
    declared_version: Option<&str>,
) -> serde_json::Value {
    linter_output::linter_json(check, status, declared_version)
}

#[cfg(test)]
mod tests {
    use super::{
        FlintTomlChange, RunArgs, unsupported_config, use_filtered_run_policy, write_changed_files,
    };
    use crate::{config, registry};
    use std::io;
    use std::io::Write;
    use std::path::Path;

    fn run_args() -> RunArgs {
        RunArgs {
            fix: false,
            allow_fixed: false,
            full: false,
            verbose: false,
            short: false,
            new_from_rev: None,
            to_ref: None,
            time: false,
            linters: Vec::new(),
        }
    }

    #[test]
    fn changed_files_treats_broken_pipe_as_success() {
        struct ClosedPipe;

        impl Write for ClosedPipe {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let root = Path::new("/tmp/project");
        let files = [root.join("file.txt")];
        let mut stdout = ClosedPipe;
        assert!(write_changed_files(&mut stdout, root, &files, false).is_ok());
    }

    #[test]
    fn flint_toml_check_changes_accept_underscore_aliases() {
        let unchanged_alias = FlintTomlChange {
            current: toml::from_str("[checks.renovate_deps]\nexclude_managers = []\n").unwrap(),
            previous: toml::from_str("[checks.renovate-deps]\nexclude_managers = []\n").unwrap(),
            settings_changed: false,
        };
        assert!(!unchanged_alias.check_changed("renovate-deps"));

        let changed_alias = FlintTomlChange {
            current: toml::from_str("[checks.renovate_deps]\nexclude_managers = [\"npm\"]\n")
                .unwrap(),
            previous: toml::from_str("[checks.renovate_deps]\nexclude_managers = []\n").unwrap(),
            settings_changed: false,
        };
        assert!(changed_alias.check_changed("renovate-deps"));
    }

    fn mise_tools_from(content: &str) -> std::collections::HashMap<String, String> {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("mise.toml"), content).expect("write mise.toml");
        registry::read_mise_tools(dir.path())
    }

    #[test]
    fn linters_table_matches_fixture_without_fake_bins() {
        let mise_tools = mise_tools_from(
            r#"[tools]
biome = "2.3.14"
"aqua:owenlamont/ryl" = "0.6.0"
taplo = "0.10.0"
lychee = "0.22.0"
"npm:renovate" = "43.92.1"
shellcheck = "v0.11.0"
shfmt = "v3.13.1"
actionlint = "1.7.10"
zizmor = "1.25.2"
editorconfig-checker = "v3.6.1"
ruff = "0.15.0"
typos = "1.46.0"
rumdl = "0.1.78"
rust = { version = "1.94.1", components = "clippy,rustfmt" }
"#,
        );
        let cfg = config::Config::default();
        let installed = [
            "actionlint",
            "biome",
            "cargo-clippy",
            "typos",
            "ec",
            "lychee",
            "renovate",
            "ruff",
            "rumdl",
            "rustfmt",
            "ryl",
            "shellcheck",
            "shfmt",
            "taplo",
            "zizmor",
        ];

        let table = crate::linter_output::render_linters_table(
            &registry::builtin(),
            &mise_tools,
            &cfg,
            |bin| installed.contains(&bin),
        );

        assert_eq!(
            table,
            r#"NAME                  BINARY              STATUS         SPEED     FIX  DESCRIPTION                                                            PATTERNS
---------------------------------------------------------------------------------------------------------------------------------------------------------
flint-setup           (built-in)          active         fast      yes  Keep Flint setup current and mise.toml lint tooling canonical          mise.toml
shellcheck            shellcheck          active         fast      no   Lint shell scripts for common mistakes                                 *.sh *.bash *.bats
shfmt                 shfmt               active         fast      yes  Format shell scripts                                                   *.sh *.bash
rumdl                 rumdl               active         fast      yes  Lint Markdown files for style and consistency                          *.md
ryl                   ryl                 active         fast      yes  Lint YAML files for style and consistency                              *.yml *.yaml
kube-linter           kube-linter         missing        fast      no   Lint explicitly selected Kubernetes resources                          k8s/*.yml k8s/*.yaml kubernetes/*.yml kubernetes/*.yaml manifests/*.yml manifests/*.yaml
taplo                 taplo               active         fast      yes  Format TOML files                                                      *.toml
actionlint            actionlint          active         fast      no   Lint GitHub Actions workflow files                                     .github/workflows/*.yml .github/workflows/*.yaml
zizmor                zizmor              active         fast      yes  Audit GitHub Actions workflows for security issues                     .github/workflows/*.yml .github/workflows/*.yaml
hadolint              hadolint            missing        fast      no   Lint Dockerfiles                                                       Dockerfile Dockerfile.* *.dockerfile
xmllint               xmllint             missing        fast      no   Validate XML files are well-formed                                     *.xml
typos                 typos               active         fast      yes  Check for common spelling mistakes                                     *
dotenv-linter         dotenv-linter       missing        fast      yes  Lint dotenv environment files without printing their values            .env .env.* *.env
editorconfig-checker  ec                  active         fast      no   Check files comply with EditorConfig settings                          *
golangci-lint         golangci-lint       missing        fast      no   Lint Go code; uses --new-from-rev to scope analysis to changed code    *.go
ruff                  ruff                active         fast      yes  Lint Python code                                                       *.py
ruff-format           ruff                active         fast      yes  Format Python code                                                     *.py
biome                 biome               active         fast      yes  Lint JS/TS/JSON files                                                  *.json *.jsonc *.js *.ts *.jsx *.tsx
biome-format          biome               active         fast      yes  Format JS/TS/JSON files                                                *.json *.jsonc *.js *.ts *.jsx *.tsx
cargo-clippy          cargo-clippy        active         fast      yes  Lint Rust code; runs on all .rs files, not just changed                *.rs
cargo-fmt             rustfmt             active         fast      yes  Format Rust code; runs on all .rs files, not just changed              *.rs
gofmt                 gofmt               missing        fast      yes  Format Go code                                                         *.go
regex-replace         (built-in)          not configured  fast      yes  Apply configured regular-expression replacements to source files
google-java-format    google-java-format  missing        fast      yes  Format Java code                                                       *.java
checkstyle            checkstyle          missing        fast      no   Check Java source against a repository-owned Checkstyle configuration  *.java
ktlint                ktlint              missing        fast      yes  Lint and format Kotlin code                                            *.kt *.kts
dotnet-format         dotnet              missing        fast      yes  Format C# code                                                         *.cs
lychee                lychee              active         fast      no   Check for broken links
renovate-deps         renovate            active         adaptive  yes  Verify Renovate dependency snapshot is up to date                      renovate.json renovate.json5 .github/renovate.json .github/renovate.json5 .renovaterc .renovaterc.json .renovaterc.json5
license-header        (built-in)          not configured  fast      no   Check source files have the required license header
"#
        );
    }

    #[test]
    fn linter_json_exposes_registry_metadata() {
        let check = registry::builtin()
            .into_iter()
            .find(|check| check.name == "rumdl")
            .expect("rumdl check");

        let json = crate::linter_json(&check, "active", Some("0.2.31"));

        assert_eq!(json["status"], "active");
        assert_eq!(json["declared_version"], "0.2.31");
        assert_eq!(json["install_key"], "rumdl");
        assert_eq!(json["scope"], "files");
        assert_eq!(json["formatter"], true);
        assert!(json["project_url"].as_str().is_some());
        assert!(json["baseline_config"].as_str().is_some());
    }

    #[test]
    fn linter_json_and_status_use_bare_tool_alias_fallback() {
        let check = registry::Check::files("ryl", "ryl {FILES}", &["*.yml"])
            .mise_tool("aqua:owenlamont/ryl")
            .version_req(">=1.0.0");
        let cfg = config::Config::default();

        let active_tools = mise_tools_from("[tools]\nryl = \"1.2.3\"\n");
        let json = crate::linter_output::linter_json_for(&check, &active_tools, &cfg, |_| true);
        assert_eq!(json["status"], "active");
        assert_eq!(json["declared_version"], "1.2.3");

        let wrong_version_tools = mise_tools_from("[tools]\nryl = \"0.6.0\"\n");
        assert_eq!(
            crate::linter_output::linter_status(&check, &wrong_version_tools, &cfg, |_| true),
            "wrong version"
        );
    }

    #[test]
    fn linter_status_reports_no_binary_and_not_configured() {
        let cfg = config::Config::default();
        let shellcheck = registry::builtin()
            .into_iter()
            .find(|check| check.name == "shellcheck")
            .expect("shellcheck check");

        let active_without_binary = mise_tools_from("[tools]\nshellcheck = \"v0.11.0\"\n");
        assert_eq!(
            crate::linter_output::linter_status(&shellcheck, &active_without_binary, &cfg, |_| {
                false
            }),
            "no binary"
        );

        let license_header = registry::builtin()
            .into_iter()
            .find(|check| check.name == "license-header")
            .expect("license-header check");
        assert_eq!(
            crate::linter_output::linter_status(
                &license_header,
                &std::collections::HashMap::new(),
                &cfg,
                |_| true
            ),
            "not configured"
        );
    }

    #[test]
    fn display_binary_marks_builtins() {
        let license_header = registry::builtin()
            .into_iter()
            .find(|check| check.name == "license-header")
            .expect("license-header check");
        assert_eq!(
            crate::linter_output::display_binary(&license_header),
            "(built-in)"
        );
    }

    #[test]
    fn filtered_run_policy_is_default_outside_ci() {
        assert!(use_filtered_run_policy(&run_args(), false, false));
    }

    #[test]
    fn filtered_run_policy_is_disabled_by_default_in_ci() {
        assert!(!use_filtered_run_policy(&run_args(), false, true));
    }

    #[test]
    fn filtered_run_policy_is_disabled_for_full_runs() {
        let mut args = run_args();
        args.full = true;

        assert!(!use_filtered_run_policy(&args, false, false));
        assert!(!use_filtered_run_policy(&args, false, true));
    }

    #[test]
    fn filtered_run_policy_is_disabled_for_explicit_linter_selection() {
        assert!(!use_filtered_run_policy(&run_args(), true, false));
        assert!(!use_filtered_run_policy(&run_args(), true, true));
    }

    #[test]
    fn typos_supported_root_config_is_not_flagged_when_config_dir_is_project_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("_typos.toml"), "[default.extend-words]\n").unwrap();
        let typos = registry::builtin()
            .into_iter()
            .find(|check| check.name == "typos")
            .expect("typos check");

        let found = unsupported_config(&typos, dir.path(), Path::new("."));

        assert_eq!(found, None);
    }

    #[test]
    fn rustfmt_root_config_is_still_flagged_when_config_dir_is_project_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("rustfmt.toml"), "max_width = 120\n").unwrap();
        let rustfmt = registry::builtin()
            .into_iter()
            .find(|check| check.name == "cargo-fmt")
            .expect("cargo-fmt check");

        let found = unsupported_config(&rustfmt, dir.path(), Path::new("."));

        assert_eq!(found, Some("rustfmt.toml".to_string()));
    }

    #[test]
    fn zizmor_supported_root_config_is_not_flagged_when_config_dir_is_project_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("zizmor.yml"), "rules: {}\n").unwrap();
        let zizmor = registry::builtin()
            .into_iter()
            .find(|check| check.name == "zizmor")
            .expect("zizmor check");

        let found = unsupported_config(&zizmor, dir.path(), Path::new("."));

        assert_eq!(found, None);
    }

    #[test]
    fn zizmor_root_config_is_still_flagged_when_config_dir_is_elsewhere() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".github/config")).unwrap();
        std::fs::write(dir.path().join("zizmor.yml"), "rules: {}\n").unwrap();
        let zizmor = registry::builtin()
            .into_iter()
            .find(|check| check.name == "zizmor")
            .expect("zizmor check");

        let found = unsupported_config(&zizmor, dir.path(), Path::new(".github/config"));

        assert_eq!(found, Some("zizmor.yml".to_string()));
    }
}
