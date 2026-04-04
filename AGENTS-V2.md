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

Always run `mise run lint:fix` before committing and review auto-fixed files —
auto-fixes may produce unexpected results.
