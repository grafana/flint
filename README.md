# lint-tasks

Shared [mise](https://mise.jdx.dev/) task scripts for linting with
[Super-Linter](https://github.com/super-linter/super-linter) and
[lychee](https://lychee.cli.rs/).

## Usage

Add the shared tasks to your project's `mise.toml`:

```toml
[task_config]
includes = ["git::https://github.com/grafana/lint-tasks.git//tasks?ref=v1.0.0"]
```

Then wire up the top-level `lint` and `fix` tasks (add any project-specific
subtasks to the `depends` list):

```toml
[tasks.lint]
description = "Run all lints"
depends = ["lint:super-linter", "lint:local-links", "lint:links-in-modified-files"]

[tasks.fix]
description = "Auto-fix lint issues then verify"
run = "AUTOFIX=true mise run lint:super-linter && mise run lint"
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

## Provided tasks

| Task | Description |
|------|-------------|
| `lint:super-linter` | Run Super-Linter via Docker/Podman |
| `lint:links` | Check links in all files with lychee |
| `lint:local-links` | Check local file links with lychee |
| `lint:links-in-modified-files` | Check links only in files modified vs base branch |

## Per-repo configuration (not included)

Each consuming repo must provide its own:

- **Super-Linter env file** (`.github/config/super-linter.env`) — which
  validators to enable, which `FIX_*` vars to set
- **Linter config files** — `.golangci.yaml`, `.markdownlint.yaml`,
  `.yaml-lint.yml`, `.editorconfig`, etc.
- **Lychee config** (`.github/config/lychee.toml`) — exclusions, timeouts,
  remappings
