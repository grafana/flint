# AGENTS.md

This file provides guidance to AI coding agents when working
with code in this repository.

## Scope

This repository is flint v2: a single Rust binary.

## Repository Layout

- Usage documentation: `README.md`
- Agent knowledge index: `.github/agents/knowledge/README.md`

## Repository Overview

Flint discovers linting tools from the consuming repo's
`mise.toml`, runs them against changed files in parallel,
and produces identical behaviour locally and in CI.

## Knowledge Loading

For coding, fix, and refactoring tasks, consult
`.github/agents/knowledge/README.md` before making
substantial changes.

Use the knowledge index to load only the article(s)
relevant to the current task. Do not load the entire
knowledge folder by default.

## Linting

**Always run `mise run lint:fix` before committing changes.**
This ensures all files pass CI linting (Biome formatting,
shellcheck, etc.). Review the auto-fixed files before
committing — auto-fixes may produce unexpected results.

Linting can be automated via a Git pre-commit hook or an
agent-specific hook (e.g. a Claude Code `PreToolUse` hook
that intercepts `git push`). Use whichever fits your
workflow — both are optional. To install the Git hook:

```bash
# Auto-fix and verify (recommended dev workflow)
mise run lint:fix

# Verify only (same command used in CI)
mise run lint

# Install git pre-commit hook (one-time, opt-in)
flint hook install
```

## Commit Messages

This repository uses
[Conventional Commits](https://www.conventionalcommits.org/)
format for PR titles (enforced by CI). Since we use squash
merges, the PR title becomes the commit message on main.
PR titles must follow this format:

```text
type(optional scope): description
```

Common types: `feat`, `fix`, `chore`, `docs`, `refactor`,
`test`, `ci`

**Release impact:** This repository uses
[release-please](https://github.com/googleapis/release-please).
Only `feat:` and `fix:` trigger new releases, and breaking
changes (`feat!:` / `fix!:` or commits with a
`BREAKING CHANGE` footer) trigger a major version bump.
Use `docs:`, `ci:`, or `chore:` for changes that don't
affect consumers (documentation, CI workflows, repository
config).
Misusing `fix:` for non-functional changes creates
unnecessary releases.

## Execution Rules

Run tests with `cargo test`. Tests spin up temporary git
repos and run the real `flint` binary — they are
integration tests, not unit tests, so they can be slow.

The `cases` test runs all fixture cases under `tests/cases/`
in parallel by top-level directory (linter group). Two env
vars control its behaviour:

- `FLINT_CASES=<dir>` — run only cases matching that prefix,
  e.g. `FLINT_CASES=shellcheck` or
  `FLINT_CASES=shellcheck/clean`.
- `UPDATE_SNAPSHOTS=1` — regenerate golden stdout/stderr/exit
  in `test.toml` instead of asserting. Always review the diff
  before committing.

On failure the test prints a rerun hint, e.g.:
`FLINT_CASES=shellcheck/clean cargo test cases`

Always run `mise run lint:fix` before committing and review
auto-fixed files — auto-fixes may produce unexpected
results.

## `--fix` outcomes

`flint run --fix` models per-check results as `clean`,
`fixed`, `partial`, or `review`.

- `clean` — the fixer ran and found nothing to change
- `fixed` — the fixer resolved the issue; commit before pushing
- `partial` — a fixer ran but the check still failed
- `review` — no fixer was applied; human review is required

The process exit contract stays coarse:

- `0` — everything was already clean
- non-zero — something still needs action

Only `0` vs non-`0` is stable for callers. Use the summary
line for human/agent guidance, for example:

```text
flint: fixed: gofmt — commit before pushing
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
flint: fixed: gofmt — commit before pushing | partial: cargo-clippy
```
