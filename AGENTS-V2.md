# flint v2

## Scope

Guidance for working on flint v2 — the Rust binary.
For v1 (bash task scripts), see [AGENTS-V1.md](AGENTS-V1.md).

## Repository Layout

- Usage documentation: `README.md`
- Agent knowledge index: `.github/agents/knowledge/README.md`

## Repository Overview

v2 is a single Rust binary (`flint`) that discovers linting
tools from the consuming repo's `mise.toml`, runs them
against changed files in parallel, and produces identical
output locally and in CI.

## Knowledge Loading

For coding, fix, and refactoring tasks, consult `.github/agents/knowledge/README.md`
before making substantial changes.

Use the knowledge index to load only the article(s) relevant to the current task.
Do not load the entire knowledge folder by default.

## Execution Rules

Run tests with `cargo test`. Tests spin up temporary git repos and run the real
`flint` binary — they are integration tests, not unit tests, so they can be slow.

The `cases` test runs all fixture cases under `tests/cases/` in parallel by
top-level directory (linter group). Two env vars control its behaviour:

- `FLINT_CASES=<dir>` — run only cases matching that prefix, e.g.
  `FLINT_CASES=shellcheck` or `FLINT_CASES=shellcheck/clean`.
- `UPDATE_SNAPSHOTS=1` — regenerate golden stdout/stderr/exit in `test.toml`
  instead of asserting. Always review the diff before committing.

On failure the test prints a rerun hint, e.g.:
`FLINT_CASES=shellcheck/clean cargo test cases`

Always run `mise run lint:fix` before committing and review auto-fixed files —
auto-fixes may produce unexpected results.

When working on Biome support, treat `.github/config/biome.jsonc` as the single
flint-managed Biome config. Do not add parallel support for `biome.json` unless
there is an explicit design change.
