# AGENTS.md

This file provides guidance to AI coding agents when working
with code in this repository.

## Versions

This repository contains two generations of flint:

- **v1** (stable): reusable bash task scripts consumed as
  HTTP remote tasks. See [AGENTS-V1.md](AGENTS-V1.md).
- **v2** (in development, `feat/flint-v2` branch): a single
  Rust binary. See [AGENTS-V2.md](AGENTS-V2.md).

## Linting

**Always run `mise run fix` before committing changes.**
This ensures all files pass CI linting (Biome formatting,
shellcheck, etc.). Review the auto-fixed files before
committing — auto-fixes may produce unexpected results.

Linting can be automated via a Git pre-commit hook or an
agent-specific hook (e.g. a Claude Code `PreToolUse` hook
that intercepts `git push`). Use whichever fits your
workflow — both are optional. To install the Git hook:

```bash
# Auto-fix and verify (recommended dev workflow)
mise run fix

# Verify only (same command used in CI)
mise run lint

# Install git pre-commit hook (one-time, opt-in)
mise run setup:pre-commit-hook
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
