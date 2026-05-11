use super::Cli;
use clap::CommandFactory;
use std::path::Path;

const CLI_FLAGS_GENERATED_COMMENT: &str =
    "<!-- Generated. Run `mise run generate` to regenerate. -->";
const CLI_FLAGS_START: &str = "<!-- run-flags-start -->";
const CLI_FLAGS_END: &str = "<!-- run-flags-end -->";

fn generate_run_flags_table() -> String {
    let mut command = Cli::command();
    let run = command
        .find_subcommand_mut("run")
        .expect("run subcommand must exist");

    let mut rows = vec![(
        "Flag".to_string(),
        "Env var".to_string(),
        "Description".to_string(),
    )];

    for arg in run.get_arguments() {
        let Some(long) = arg.get_long() else {
            continue;
        };
        if arg.is_hide_set() {
            continue;
        }

        let mut flag = format!("`--{long}`");
        if arg.get_action().takes_values()
            && let Some(names) = arg.get_value_names()
        {
            for name in names {
                flag.push(' ');
                flag.push_str(name);
            }
        }

        let env = arg
            .get_env()
            .map(|v| format!("`{}`", v.to_string_lossy()))
            .unwrap_or_else(|| "—".to_string());
        let help = arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default()
            .replace('\n', " ");

        rows.push((flag, env, help));
    }

    let flag_width = rows
        .iter()
        .map(|(flag, _, _)| flag.len())
        .max()
        .unwrap_or(4);
    let env_width = rows.iter().map(|(_, env, _)| env.len()).max().unwrap_or(7);
    let desc_width = rows
        .iter()
        .map(|(_, _, desc)| desc.len())
        .max()
        .unwrap_or(11);

    let mut out = String::new();
    out.push_str(CLI_FLAGS_GENERATED_COMMENT);
    out.push('\n');
    out.push_str(&format!(
        "| {0:<flag_width$} | {1:<env_width$} | {2:<desc_width$} |\n",
        rows[0].0, rows[0].1, rows[0].2
    ));
    out.push_str(&format!(
        "| {0:-<flag_width$} | {1:-<env_width$} | {2:-<desc_width$} |\n",
        "", "", ""
    ));
    for (flag, env, desc) in rows.into_iter().skip(1) {
        out.push_str(&format!(
            "| {flag:<flag_width$} | {env:<env_width$} | {desc:<desc_width$} |\n"
        ));
    }

    out.trim_end().to_string()
}

fn strip_blank_lines(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_section(haystack: &str, start_marker: &str, end_marker: &str) -> String {
    let start = haystack
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing {start_marker} marker"))
        + start_marker.len();
    let end = haystack
        .find(end_marker)
        .unwrap_or_else(|| panic!("missing {end_marker} marker"));
    strip_blank_lines(&haystack[start..end])
}

fn replace_section(haystack: &str, start_marker: &str, end_marker: &str, body: &str) -> String {
    let start = haystack
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing {start_marker} marker"))
        + start_marker.len();
    let end = haystack
        .find(end_marker)
        .unwrap_or_else(|| panic!("missing {end_marker} marker"));
    format!(
        "{}\n{}\n{}{}",
        &haystack[..start],
        body,
        end_marker,
        &haystack[end + end_marker.len()..]
    )
}

#[test]
fn cli_docs_in_sync() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_path = manifest_dir.join("docs/cli.md");
    let cli = std::fs::read_to_string(&cli_path).expect("docs/cli.md must be readable");
    let expected = generate_run_flags_table();

    if std::env::var("UPDATE_README").is_ok() {
        let updated = replace_section(&cli, CLI_FLAGS_START, CLI_FLAGS_END, &expected);
        std::fs::write(&cli_path, updated).expect("failed to write docs/cli.md");
        return;
    }

    let actual = extract_section(&cli, CLI_FLAGS_START, CLI_FLAGS_END);
    let expected_norm = strip_blank_lines(&expected);
    assert_eq!(
        actual, expected_norm,
        "docs/cli.md run flags table is out of sync.\nRun `mise run generate` to regenerate."
    );
}

#[test]
fn run_flags_have_env_var_bindings() {
    let mut command = Cli::command();
    let run = command
        .find_subcommand_mut("run")
        .expect("run subcommand must exist");

    let missing = run
        .get_arguments()
        .filter(|arg| arg.get_long().is_some() && !arg.is_hide_set())
        .filter(|arg| arg.get_env().is_none())
        .map(|arg| format!("--{}", arg.get_long().expect("long flag")))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "every `flint run` long flag must have an env var binding; missing: {}",
        missing.join(", ")
    );
}
