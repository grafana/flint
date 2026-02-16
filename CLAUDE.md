# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This repository contains reusable mise task scripts for linting. These scripts are designed to be consumed as HTTP remote tasks in other repositories' `mise.toml` files, not run directly in this repository.

## Architecture

### Task Script Design Pattern

All task scripts follow these conventions:

- **Environment**: Scripts expect `MISE_PROJECT_ROOT` to be set (automatically provided by mise)
- **Metadata**: Shell scripts use `#MISE` comments for metadata; Python scripts use `# [MISE]` comments
- **Usage args**: Shell scripts use `#USAGE` comments to define CLI arguments that mise parses
- **Exit behavior**: Scripts exit with non-zero on errors for CI integration
- **AUTOFIX mode**: All lint scripts check the `AUTOFIX` environment variable. When `AUTOFIX=true`, linters that support fixing issues will automatically apply fixes; linters without fix capabilities silently ignore it. This allows consuming repos to run all lints with `AUTOFIX=true` via a single task (e.g., `mise run fix`) without needing per-linter configuration

### Script Categories

**`tasks/lint/`** - Linting validators:

- `super-linter.sh`: Runs Super-Linter via Docker/Podman, auto-detects runtime, handles SELinux on Fedora
- `links.sh`: Runs lychee link checker with two default checks (all links in modified files + local links in all files) and a `--full` flag for comprehensive checking
- `renovate-deps.py`: Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate locally and parsing its debug logs. With `AUTOFIX=true`, automatically regenerates and updates the file

### Key Design Decisions

1. **Container runtime detection**: `super-linter.sh` tries podman first (with SELinux "z" mount flag), falls back to Docker
2. **AUTOFIX mode**: Lint scripts that support fixing accept `--autofix` flag and `AUTOFIX` env var for unified fix workflows:
   - `super-linter.sh`: Filters out `FIX_*` env vars unless autofix is enabled
   - `renovate-deps.py`: Automatically regenerates and updates `.github/renovate-tracked-deps.json` when autofix is enabled
   - `links.sh`: Silently ignores the `AUTOFIX` env var (lychee has no autofix capability; no `--autofix` flag is exposed)
   - The `AUTOFIX` env var is how the `fix` meta-task propagates autofix through the dependency chain
3. **Diff-based link checking**: `links.sh` runs two checks by default (all links in modified files + local links in all files), use `--full` to check all links in all files; falls back to `--full` when config changes
4. **Renovate exclusions**: `RENOVATE_TRACKED_DEPS_EXCLUDE` allows skipping managers like `github-actions,github-runners`
5. **Consuming repos provide config**: Scripts reference config files (`.github/config/super-linter.env`, `.github/config/lychee.toml`) that consuming repos must provide

## Testing Changes

Since these are remote task scripts consumed by other repos:

1. Test changes by pointing a consuming repo's `mise.toml` to a local file path or Git branch
2. Verify scripts work with both Docker and Podman
3. Test with and without `AUTOFIX=true`:
   - `super-linter.sh`: Verify `FIX_*` vars are filtered correctly
   - `renovate-deps.py`: Verify it regenerates and updates the file
   - Link linters: Verify they run normally and don't output warnings
4. For Renovate scripts, ensure they handle missing deps gracefully

## Linting

**Always run `mise run fix` before committing changes.** This ensures all files pass CI linting (Biome formatting, shellcheck, etc.). Review the auto-fixed files before committing — auto-fixes may produce unexpected results.

```bash
# Auto-fix and verify (recommended dev workflow)
mise run fix

# Verify only (same command used in CI)
mise run lint
```

## Adding New Linters

When adding new lint scripts, follow these patterns:

1. **Add AUTOFIX support**: Check for `AUTOFIX` env var and implement fix behavior if the underlying tool supports it
2. **Silent fallback**: If the tool doesn't support autofix, silently ignore `AUTOFIX` (no warnings or errors)
3. **Consistent behavior**: Ensure the script works the same whether `AUTOFIX` is set or not for check-only tools
4. **Document support**: Update README.md table to show whether AUTOFIX is supported

## Commit Messages

This repository uses [Conventional Commits](https://www.conventionalcommits.org/) format, enforced by commitlint via Super-Linter. All commit messages must follow this format:

```
type(optional scope): description
```

Common types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`

## Script Conventions

- Shell scripts use `set -euo pipefail` for safety
- Python scripts check for `MISE_PROJECT_ROOT` and exit with clear error if missing
- Use `# shellcheck disable=` with justification when intentionally violating shellcheck rules
- Python scripts use `sys.exit(1)` on errors, print errors to stderr
