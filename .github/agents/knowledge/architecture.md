# Architecture

## Module Map

- **`src/registry.rs`**: Static linter registry. Defines
  `Check` (builder pattern) and `builtin()` which returns
  the full list of built-in checks. This is where new
  linters are added.
- **`src/runner.rs`**: Executes checks against a file list.
  Handles parallel execution (check mode) and serial
  execution (fix mode, to avoid concurrent writes).
- **`src/config.rs`**: Loads `flint.toml` from the project
  root. All fields have defaults — the file is optional.
- **`src/files.rs`**: Git-aware file discovery. Returns
  changed files relative to the merge base, or all files
  with `--full`.
- **`src/linters/`**: Custom logic for special checks that
  can't be expressed as a simple command template:
  - `lychee.rs`: Link checking orchestration
  - `renovate_deps.rs`: Renovate snapshot verification
- **`src/main.rs`**: CLI parsing (clap), orchestration,
  output formatting.
- **`tests/e2e.rs`**: End-to-end tests. Spin up a temp git
  repo, write files, run the flint binary, assert on
  stdout/stderr and exit code.

## Check Kinds

A `Check` is either a `Template` (a command string with
`{FILE}`, `{FILES}`, or `{MERGE_BASE}` placeholders) or a
`Special` (custom Rust logic in `src/linters/`).

Template scopes:

- `File` — invoked once per matched file (`{FILE}`)
- `Files` — invoked once with all matched files (`{FILES}`)
- `Project` — invoked once with no file args; skipped
  entirely if no matching files changed

## Baseline Expansion

Normal changed-file runs keep each check scoped to changed files. Before
execution, `src/main.rs` also computes a set of checks that need a full file
list to establish a new baseline.

A check is expanded to all matching files when:

- it was not active at the merge base, meaning its tool was newly added to
  `mise.toml`
- its resolved tool version changed in `mise.toml`
- its registered `.linter_config(...)` file changed under `FLINT_CONFIG_DIR`
- `flint.toml` changed under `[settings]`
- `flint.toml` changed the check-specific section for a special check

This is per-check. Unaffected checks still receive the normal changed-file list.
Explicit `--full` bypasses this selection because every check is already using
the all-files list. Config-change triggers use the raw git change list before
`settings.exclude` is applied, so excluded config paths still expand the affected
check.
