---
name: flint
description: Configure and run Flint linting in repositories using mise, and interpret its fix results.
---

# Flint

Flint discovers supported linters from the consuming repository's `mise.toml`.
Use `flint linters` (or `flint linters --json`) to inspect availability; do not
assume every built-in check is enabled. Run from the target repository with its
mise tools available.

## Configure

For a new setup, use `flint init` to choose a profile, or
`flint init --only <check> ...` for selected checks. Review the proposed changes;
use `--yes` only when applying them is authorized. Preserve existing tool choices.
`flint.toml` is optional. Respect `FLINT_CONFIG_DIR` if the repository sets it;
Flint-managed linter configs belong there, not in competing parallel files.

## Run and interpret

- `flint run` checks changed files; `flint run --full` checks all eligible files.
- `flint run <linter> ...` selects specific checks.
- `flint run --fix` applies available fixes. Review the resulting diff.
- `flint changed-files` inspects file selection without running checks.

Only zero versus non-zero exit status is stable. By default, a successful fix
still exits non-zero because the changes need review and committing. Read the
summary: `clean` needs no action, `fixed` needs review and committing, `partial`
means a fixer ran but the check still failed, and `review` means no fixer was
applied. Do not repeatedly run fixers merely to chase a zero exit code, and do not
commit or push without authorization. `--allow-fixed` changes the exit behavior
for fully fixed results, not whether changes require review.

For exact flags and constraints, use `flint --help`, subcommand `--help`, or
`flint usage` for the machine-readable Usage spec generated from the same Clap
command definitions. Local and CI selection policies can differ; use `--full`
when an explicit full baseline is needed.
