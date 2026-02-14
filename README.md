# flint

Reusable mise lint task scripts for Super-Linter, lychee link checking, and Renovate dependency tracking.

Shared [mise](https://mise.jdx.dev/) lint task scripts for
[Super-Linter](https://github.com/super-linter/super-linter),
[lychee](https://lychee.cli.rs/), and
[Renovate tracked-deps](https://docs.renovatebot.com/) verification.

## Usage

Reference individual task scripts via HTTP remote tasks in your `mise.toml`:

```toml
# Shared lint tasks from flint
[tasks."lint:super-linter"]
description = "Run Super-Linter on the repository"
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/lint/super-linter.sh"
[tasks."lint:links"]
description = "Lint links in all files"
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/lint/links.sh"
[tasks."lint:local-links"]
description = "Lint links in local files"
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/lint/local-links.sh"
[tasks."lint:links-in-modified-files"]
description = "Lint links in modified files"
hide = true
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/lint/links-in-modified-files.sh"
[tasks."lint:renovate-deps"]
description = "Verify renovate-tracked-deps.json is up to date"
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/lint/renovate-deps.py"
[tasks."generate:renovate-tracked-deps"]
description = "Generate renovate-tracked-deps.json from Renovate's local analysis"
file = "https://raw.githubusercontent.com/grafana/flint/v1.0.0/tasks/generate/renovate-tracked-deps.py"
```

Then wire up the top-level `lint` and `fix` tasks (add any project-specific
subtasks to the `depends` list):

```toml
[tasks.lint]
description = "Run all lints"
depends = ["lint:super-linter", "lint:local-links", "lint:links-in-modified-files", "lint:renovate-deps"]

[tasks.fix]
description = "Auto-fix lint issues, regenerate tracked deps, then verify"
run = "AUTOFIX=true mise run lint:super-linter && mise run generate:renovate-tracked-deps && mise run lint"
```

## Required environment variables

Set these in your `mise.toml`:

| Variable | Required | Description |
|----------|----------|-------------|
| `SUPER_LINTER_VERSION` | yes | Super-Linter image tag (e.g. `v8.4.0@sha256:...`) |

## Optional environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SUPER_LINTER_ENV_FILE` | `.github/config/super-linter.env` | Path to the Super-Linter env file |
| `LYCHEE_CONFIG` | `.github/config/lychee.toml` | Path to the lychee config file |
| `LYCHEE_CONFIG_CHANGE_PATTERN` | `^(\.github/config/lychee\.toml\|\.mise/tasks/lint/.*\|mise\.toml)$` | Regex for files whose change triggers a full link check |
| `AUTOFIX` | unset | Set to `true` to enable Super-Linter auto-fix mode |
| `RENOVATE_TRACKED_DEPS_EXCLUDE` | unset | Comma-separated Renovate managers to exclude (e.g. `github-actions,github-runners`) |

## Provided tasks

| Task | Description |
|------|-------------|
| `lint:super-linter` | Run Super-Linter via Docker/Podman |
| `lint:links` | Check links in all files with lychee |
| `lint:local-links` | Check local file links with lychee |
| `lint:links-in-modified-files` | Check links only in files modified vs base branch |
| `lint:renovate-deps` | Verify `renovate-tracked-deps.json` is up to date |
| `generate:renovate-tracked-deps` | Generate `renovate-tracked-deps.json` from Renovate's local analysis |

## Per-repo configuration (not included)

Each consuming repo must provide its own:

- **Super-Linter env file** (`.github/config/super-linter.env`) — which
  validators to enable, which `FIX_*` vars to set
- **Linter config files** — `.golangci.yaml`, `.markdownlint.yaml`,
  `.yaml-lint.yml`, `.editorconfig`, etc.
- **Lychee config** (`.github/config/lychee.toml`) — exclusions, timeouts,
  remappings
- **Renovate config** (`.github/renovate.json5`) and committed snapshot
  (`.github/renovate-tracked-deps.json`)
