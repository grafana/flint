# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This repository contains reusable mise task scripts for linting. These scripts are designed to be consumed as HTTP remote tasks in other repositories' `mise.toml` files, not run directly in this repo.

## Architecture

### Task Script Design Pattern

All task scripts follow these conventions:

- **Environment**: Scripts expect `MISE_PROJECT_ROOT` to be set (automatically provided by mise)
- **Metadata**: Shell scripts use `#MISE` comments for metadata; Python scripts use `# [MISE]` comments
- **Usage args**: Shell scripts use `#USAGE` comments to define CLI arguments that mise parses
- **Exit behavior**: Scripts exit with non-zero on errors for CI integration

### Script Categories

**`tasks/lint/`** - Linting validators:
- `super-linter.sh`: Runs Super-Linter via Docker/Podman, auto-detects runtime, handles SELinux on Fedora
- `links.sh`, `local-links.sh`: Run lychee link checker with different scopes
- `links-in-modified-files.sh`: Smart link linting that checks config changes and only lints modified files
- `renovate-deps.py`: Verifies `.github/renovate-tracked-deps.json` is up to date

**`tasks/generate/`** - Generators:
- `renovate-tracked-deps.py`: Generates dependency snapshot by running Renovate locally and parsing its debug logs

### Key Design Decisions

1. **Container runtime detection**: `super-linter.sh` tries podman first (with SELinux "z" mount flag), falls back to docker
2. **AUTOFIX mode**: Super-Linter script filters out `FIX_*` env vars unless `AUTOFIX=true`
3. **Diff-based link checking**: `links-in-modified-files.sh` optimizes CI by only checking modified files, unless config changed
4. **Renovate exclusions**: `RENOVATE_TRACKED_DEPS_EXCLUDE` allows skipping managers like `github-actions,github-runners`
5. **Consuming repos provide config**: Scripts reference config files (`.github/config/super-linter.env`, `.github/config/lychee.toml`) that consuming repos must provide

## Testing Changes

Since these are remote task scripts consumed by other repos:

1. Test changes by pointing a consuming repo's `mise.toml` to a local file path or git branch
2. Verify scripts work with both Docker and Podman
3. Test with and without `AUTOFIX=true` for super-linter changes
4. For Renovate scripts, ensure they handle missing deps gracefully

## Script Conventions

- Shell scripts use `set -euo pipefail` for safety
- Python scripts check for `MISE_PROJECT_ROOT` and exit with clear error if missing
- Use `# shellcheck disable=` with justification when intentionally violating shellcheck rules
- Python scripts use `sys.exit(1)` on errors, print errors to stderr
