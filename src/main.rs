mod config;
mod files;
mod hook;
mod init;
mod linters;
mod registry;
mod runner;
mod setup;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use registry::{CheckKind, FixBehavior, LinterConfig, RunPolicy, Scope, SpecialKind};
use runner::{CheckResult, RunOptions};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
    /// Apply non-interactive migrations to mise.toml (replace obsolete tool keys).
    Update,
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
                print_linters_json(&registry);
            } else {
                print_linters(&registry, &mise_tools, &cfg);
            }
        }
        SubCommand::Init(args) => {
            init::run(
                &project_root,
                args.profile,
                args.yes,
                args.flint_rev.as_deref(),
            )?;
        }
        SubCommand::Update => {
            init::update(&project_root, &config_dir)?;
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

    let file_list = files::changed(
        project_root,
        &cfg,
        args.full,
        args.new_from_rev.as_deref(),
        args.to_ref.as_deref(),
    )?;

    // Discover which checks are declared in the consuming repo's mise.toml, and apply
    // --fast-only policy (skipped when linters are named explicitly, relevance-gated for
    // adaptive checks). mise guarantees declared tools are on PATH, so no PATH check needed.
    let mise_tools = registry::read_mise_tools(project_root);
    let flint_setup_selected = checks
        .iter()
        .any(|c| matches!(&c.kind, CheckKind::Special(SpecialKind::FlintSetup)));
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
    let active: Vec<&registry::Check> = {
        let mut out = vec![];
        for c in checks {
            if registry::check_active(c, &mise_tools) {
                let include = if explicit || !args.fast_only {
                    true
                } else {
                    match c.run_policy {
                        RunPolicy::Fast => true,
                        RunPolicy::Slow => false,
                        RunPolicy::Adaptive => match &c.kind {
                            CheckKind::Special(SpecialKind::RenovateDeps) => {
                                linters::renovate_deps::is_relevant(&file_list, project_root)
                            }
                            _ => true,
                        },
                    }
                };
                if include {
                    out.push(c);
                }
            } else if explicit {
                eprintln!(
                    "flint: linter {name} is not active (binary not installed or not declared in mise.toml)",
                    name = c.name
                );
                std::process::exit(1);
            }
        }
        out
    };

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
            RunContext {
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
            finish_fix_outcomes(vec![classify_single_pass_fix(setup_result)]);
        } else if !setup_result.ok {
            let failed = [setup_result.name.as_str()];
            if args.short {
                eprintln!("flint: 1 check failed — flint run --fix {}", failed[0]);
            } else {
                eprintln!("\nflint: 1 check failed ({})", failed[0]);
                eprintln!(
                    "💡 Try `flint run --fix` to auto-fix lint issues, then re-run `flint run` to verify."
                );
            }
            std::process::exit(1);
        }
    }
    let active: Vec<&registry::Check> = active
        .into_iter()
        .filter(|check| !is_flint_setup(check))
        .collect();

    if active.is_empty() {
        return Ok(());
    }

    if let Some((check, config)) = active.iter().find_map(|check| {
        unsupported_config(check, project_root, config_dir).map(|config| (*check, config))
    }) {
        let canonical = check
            .linter_config
            .as_ref()
            .map(canonical_config_path)
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
    let run_ctx = RunContext {
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
                .partition(|c| supports_single_pass_fix(c));

        let mut outcomes = vec![];

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

            let (fixable, reviewable): (Vec<CheckResult>, Vec<CheckResult>) = check_results
                .into_iter()
                .filter(|r| !r.ok)
                .partition(|r| is_fixable(&r.name, &legacy_checks));
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
                            } else {
                                outcomes.push(FixOutcome::Fixed(r.name));
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
                        outcomes.push(FixOutcome::Fixed(r.name));
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

        finish_fix_outcomes(outcomes);
        return Ok(());
    }

    let results = run_checks(
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

#[derive(Clone, Copy)]
struct RunContext<'a> {
    active_checks: &'a [&'a registry::Check],
    project_root: &'a Path,
    cfg: &'a config::Config,
    config_dir: &'a Path,
}

enum FixOutcome {
    Clean,
    Fixed(String),
    Partial(CheckResult),
    Review(CheckResult),
}

impl FixOutcome {
    fn result(&self) -> Option<&CheckResult> {
        match self {
            Self::Partial(result) | Self::Review(result) => Some(result),
            Self::Clean | Self::Fixed(_) => None,
        }
    }
}

fn finish_fix_outcomes(outcomes: Vec<FixOutcome>) {
    // Emit linter output for checks that need manual review so the caller has
    // the failure details without a second flint invocation.
    for r in outcomes.iter().filter_map(FixOutcome::result) {
        eprintln!("[{}]", r.name);
        if !r.stdout.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&r.stdout));
        }
        if !r.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&r.stderr));
        }
    }

    let mut fixed = vec![];
    let mut partial = vec![];
    let mut review = vec![];
    for outcome in outcomes {
        match outcome {
            FixOutcome::Clean => {}
            FixOutcome::Fixed(name) => fixed.push(name),
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
        std::process::exit(1);
    }
}

fn classify_single_pass_fix(result: CheckResult) -> FixOutcome {
    if result.ok {
        if result.changed {
            FixOutcome::Fixed(result.name)
        } else {
            FixOutcome::Clean
        }
    } else if result.changed {
        FixOutcome::Partial(result)
    } else {
        FixOutcome::Review(result)
    }
}

fn is_flint_setup(check: &registry::Check) -> bool {
    matches!(&check.kind, CheckKind::Special(SpecialKind::FlintSetup))
}

async fn run_checks(
    checks: &[&registry::Check],
    file_list: &files::FileList,
    baseline_file_list: Option<&files::FileList>,
    baseline_names: &HashSet<String>,
    opts: RunOptions,
    ctx: RunContext<'_>,
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
                ctx.active_checks,
                file_list,
                opts,
                ctx.project_root,
                ctx.cfg,
                ctx.config_dir,
            )
            .await?,
        );
    }
    if !baseline.is_empty() {
        let files = baseline_file_list.unwrap_or(file_list);
        results.extend(
            runner::run(
                &baseline,
                ctx.active_checks,
                files,
                opts,
                ctx.project_root,
                ctx.cfg,
                ctx.config_dir,
            )
            .await?,
        );
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

fn baseline_check_names(
    active: &[&registry::Check],
    file_list: &files::FileList,
    project_root: &Path,
    config_dir: &Path,
    current_tools: &HashMap<String, String>,
) -> HashSet<String> {
    if file_list.full {
        return HashSet::new();
    }
    let Some(merge_base) = file_list.merge_base.as_deref() else {
        return HashSet::new();
    };

    let changed = changed_rel_paths(file_list, project_root);
    let previous_tools = registry::read_mise_tools_at_ref(project_root, merge_base);
    if registry::flint_version_changed(&previous_tools, current_tools) {
        return active.iter().map(|check| check.name.to_string()).collect();
    }

    let flint_config = config_rel_path(project_root, config_dir, "flint.toml");
    let flint_config_changed = changed.contains(&flint_config);
    let flint_toml =
        flint_config_changed.then(|| flint_toml_change(project_root, config_dir, merge_base));

    active
        .iter()
        .filter(|check| {
            !registry::check_active(check, &previous_tools)
                || registry::tool_version_changed(check, &previous_tools, current_tools)
                || flint_toml.as_ref().is_some_and(|change| {
                    change.settings_changed
                        || (matches!(check.kind, CheckKind::Special(_))
                            && change.check_changed(check.name))
                })
                || check.baseline_config.as_ref().is_some_and(|config| {
                    changed.contains(&config_file_rel_path(project_root, config_dir, config))
                })
                || (check.name == "editorconfig-checker"
                    && changed.contains(&config_file_rel_path(
                        project_root,
                        config_dir,
                        &registry::ConfigFile::project(".editorconfig"),
                    )))
        })
        .map(|check| check.name.to_string())
        .collect()
}

fn unsupported_config(
    check: &registry::Check,
    project_root: &Path,
    config_dir: &Path,
) -> Option<String> {
    check
        .unsupported_configs
        .iter()
        .find(|config| config_present(project_root, config_dir, config))
        .map(|config| config_file_rel_path(project_root, config_dir, config))
}

struct FlintTomlChange {
    current: toml::Value,
    previous: toml::Value,
    settings_changed: bool,
}

impl FlintTomlChange {
    fn check_changed(&self, name: &str) -> bool {
        toml_section(&self.current, &["checks", name])
            != toml_section(&self.previous, &["checks", name])
    }
}

fn flint_toml_change(project_root: &Path, config_dir: &Path, merge_base: &str) -> FlintTomlChange {
    let rel = config_rel_path(project_root, config_dir, "flint.toml");
    let current_path = project_root.join(&rel);
    let current = read_toml_file(&current_path);
    let previous = read_toml_at_ref(project_root, merge_base, &rel);
    let settings_changed =
        toml_section(&current, &["settings"]) != toml_section(&previous, &["settings"]);
    FlintTomlChange {
        current,
        previous,
        settings_changed,
    }
}

fn read_toml_file(path: &Path) -> toml::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()))
}

fn config_present(project_root: &Path, config_dir: &Path, config: &registry::ConfigFile) -> bool {
    let path = config_file_abs_path(project_root, config_dir, config);
    match config.presence {
        registry::ConfigMatch::Exists => path.exists(),
        registry::ConfigMatch::TomlSection(section) => {
            toml_section(&read_toml_file(&path), section).is_some()
        }
        registry::ConfigMatch::IniSection(section) => ini_section_exists(&path, section),
    }
}

fn ini_section_exists(path: &Path, section: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .is_some_and(|name| name.trim() == section)
    })
}

fn read_toml_at_ref(project_root: &Path, git_ref: &str, rel_path: &str) -> toml::Value {
    let spec = format!("{git_ref}:{rel_path}");
    std::process::Command::new("git")
        .args(["show", &spec])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()))
}

fn toml_section<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn changed_rel_paths(file_list: &files::FileList, project_root: &Path) -> HashSet<String> {
    if !file_list.changed_paths.is_empty() {
        return file_list.changed_paths.iter().cloned().collect();
    }

    file_list
        .files
        .iter()
        .filter_map(|path| path.strip_prefix(project_root).ok())
        .map(normalize_path)
        .collect()
}

fn config_rel_path(project_root: &Path, config_dir: &Path, file: &str) -> String {
    let path = if config_dir.is_absolute() {
        config_dir.join(file)
    } else {
        project_root.join(config_dir).join(file)
    };
    path.strip_prefix(project_root)
        .map(normalize_path)
        .unwrap_or_else(|_| normalize_path(&PathBuf::from(file)))
}

fn config_file_abs_path(
    project_root: &Path,
    config_dir: &Path,
    config: &registry::ConfigFile,
) -> PathBuf {
    match config.base {
        registry::ConfigBase::ProjectRoot => project_root.join(config.path),
        registry::ConfigBase::ConfigDir => {
            if config_dir.is_absolute() {
                config_dir.join(config.path)
            } else {
                project_root.join(config_dir).join(config.path)
            }
        }
    }
}

fn config_file_rel_path(
    project_root: &Path,
    config_dir: &Path,
    config: &registry::ConfigFile,
) -> String {
    let path = config_file_abs_path(project_root, config_dir, config);
    path.strip_prefix(project_root)
        .map(normalize_path)
        .unwrap_or_else(|_| normalize_path(&PathBuf::from(config.path)))
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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
    let config_file = check
        .linter_config
        .as_ref()
        .map(LinterConfig::canonical_location);
    serde_json::json!({
        "name": check.name,
        "description": check.desc,
        "binary": if check.uses_binary() { check.bin_name } else { "(built-in)" },
        "patterns": patterns,
        "fix": check.has_fix(),
        "run_policy": run_policy_label(check.run_policy),
        "slow": check.run_policy == RunPolicy::Slow,
        "scope": scope,
        "config_file": config_file,
    })
}

fn canonical_config_path(config: &LinterConfig) -> String {
    config.canonical_location()
}

fn run_policy_label(run_policy: RunPolicy) -> &'static str {
    match run_policy {
        RunPolicy::Fast => "fast",
        RunPolicy::Slow => "slow",
        RunPolicy::Adaptive => "adaptive",
    }
}

fn is_fixable(name: &str, active: &[&registry::Check]) -> bool {
    active.iter().any(|c| c.name == name && c.has_fix())
}

fn supports_single_pass_fix(check: &registry::Check) -> bool {
    check.has_fix()
        && check.fix_behavior() == FixBehavior::Definitive
        && matches!(
            check.kind,
            CheckKind::Template {
                scope: Scope::File | Scope::Files,
                ..
            }
        )
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
        "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {:<desc_w$}  PATTERNS",
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
    println!("{}", "-".repeat(name_w + bin_w + desc_w + 46));

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
        let speed = run_policy_label(check.run_policy);
        let fix = if check.has_fix() { "yes" } else { "no" };
        let patterns_str = check.patterns.join(" ");
        if patterns_str.is_empty() {
            println!(
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {:<desc_w$}",
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
                "{:<name_w$}  {:<bin_w$}  {:<13}  {:<8}  {:<3}  {:<desc_w$}  {}",
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
