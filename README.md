# flint

Reusable mise lint task scripts for Super-Linter, lychee link checking, and Renovate dependency tracking.

Shared [mise](https://mise.jdx.dev/) lint task scripts for
[Super-Linter](https://github.com/super-linter/super-linter),
[lychee](https://lychee.cli.rs/), and
[Renovate tracked-deps](https://docs.renovatebot.com/) verification.

## Usage

⚠️ **Important**: Always pin to a specific version tag (e.g., `v0.1.0`), never use `main`. The main branch may contain breaking changes. See [CHANGELOG.md](CHANGELOG.md) for version history.

Reference individual task scripts via HTTP remote tasks in your `mise.toml`:

```toml
# Shared lint tasks from flint
[tasks."lint:super-linter"]
description = "Run Super-Linter on the repository"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/super-linter.sh"
[tasks."lint:links"]
description = "Lint links in all files"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/links.sh"
[tasks."lint:local-links"]
description = "Lint links in local files"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/local-links.sh"
[tasks."lint:links-in-modified-files"]
description = "Lint links in modified files"
hide = true
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/links-in-modified-files.sh"
[tasks."lint:renovate-deps"]
description = "Verify renovate-tracked-deps.json is up to date"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/renovate-deps.py"
```

Then wire up the top-level `lint` and `fix` tasks (add any project-specific
subtasks to the `depends` list):

```toml
[tasks.lint]
description = "Run all lints"
depends = ["lint:super-linter", "lint:local-links", "lint:links-in-modified-files", "lint:renovate-deps"]

[tasks.fix]
description = "Auto-fix lint issues and regenerate tracked deps"
run = "AUTOFIX=true mise run lint"
```

## Required environment variables

Set these in your `mise.toml`:

| Variable               | Required | Description                                       |
| ---------------------- | -------- | ------------------------------------------------- |
| `SUPER_LINTER_VERSION` | yes      | Super-Linter image tag (e.g. `v8.4.0@sha256:...`) |

## Optional environment variables

| Variable                        | Default                                                              | Description                                                                           |
| ------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `SUPER_LINTER_ENV_FILE`         | `.github/config/super-linter.env`                                    | Path to the Super-Linter env file                                                     |
| `LYCHEE_CONFIG`                 | `.github/config/lychee.toml`                                         | Path to the lychee config file                                                        |
| `LYCHEE_CONFIG_CHANGE_PATTERN`  | `^(\.github/config/lychee\.toml\|\.mise/tasks/lint/.*\|mise\.toml)$` | Regular expression for files whose change triggers a full link check                  |
| `AUTOFIX`                       | unset                                                                | Set to `true` to enable autofix mode (Super-Linter fixes, renovate-deps regeneration) |
| `RENOVATE_TRACKED_DEPS_EXCLUDE` | unset                                                                | Comma-separated Renovate managers to exclude (e.g. `github-actions,github-runners`)   |

## Provided tasks

| Task                           | Description                                       | AUTOFIX Support         |
| ------------------------------ | ------------------------------------------------- | ----------------------- |
| `lint:super-linter`            | Run Super-Linter via Docker/Podman                | ✅ Enables `FIX_*` vars |
| `lint:links`                   | Check links in all files with lychee              | ❌ Ignored              |
| `lint:local-links`             | Check local file links with lychee                | ❌ Ignored              |
| `lint:links-in-modified-files` | Check links only in files modified vs base branch | ❌ Ignored              |
| `lint:renovate-deps`           | Verify `renovate-tracked-deps.json` is up to date | ✅ Regenerates file     |

## How AUTOFIX Works

All lint scripts support the `AUTOFIX` environment variable for a unified fix workflow:

**Check mode** (default):

```bash
mise run lint              # Check all linters, fail on issues
mise run lint:super-linter # Check code style, fail on issues
mise run lint:renovate-deps # Verify tracked deps, fail if out of date
```

**Fix mode** (`AUTOFIX=true`):

```bash
mise run fix               # Auto-fix all fixable issues
# Or run individual linters:
AUTOFIX=true mise run lint:super-linter   # Apply code fixes
AUTOFIX=true mise run lint:renovate-deps  # Regenerate tracked deps
```

Linters that don't support autofix (like lychee link checker) silently ignore the `AUTOFIX` variable and run normally. This allows you to run all lints with `AUTOFIX=true` without errors.

## Renovate Tracked Deps Linter

### Why this exists

Renovate silently stops tracking a dependency when it can no longer parse the version reference (typo in a comment annotation, unsupported syntax, moved file, etc.). When that happens, the dependency freezes in place with no PR and no dashboard entry — it simply disappears from Renovate's radar.

The Dependency Dashboard catches _known_ dependencies that are pending or in error, but it cannot show you a dependency that Renovate no longer sees at all. This linter closes that gap by keeping a committed snapshot of every dependency Renovate tracks and failing CI when the two diverge.

### How it works

The `lint:renovate-deps` task runs Renovate locally in `--platform=local` mode, parses its debug log for the `packageFiles with updates` message, and generates a dependency list (grouped by file and manager). It then diffs this against the committed `.github/renovate-tracked-deps.json`:

- If they match → linter passes
- If they differ → linter fails with a unified diff showing which dependencies were added or removed
- With `AUTOFIX=true` → automatically regenerates and updates the committed file

### Typical workflow

- **A dependency disappears** (e.g., someone removes a `# renovate:` comment or changes a file that Renovate was matching) → CI fails, showing the removed dependency in the diff. The author can then decide whether the removal was intentional or accidental.

- **A new dependency is added** → CI fails because the committed snapshot is stale. Run `mise run fix` (or `AUTOFIX=true mise run lint:renovate-deps`) to regenerate and update the file, then commit.

- **Routine regeneration** → After any change to `renovate.json5`, Dockerfiles, `go.mod`, `package.json`, or other files Renovate scans, the linter will detect the change and require regeneration.

## Automatic version updates with Renovate

To let Renovate automatically update the pinned flint version in your
`mise.toml`, add this custom manager to your `renovate.json5`:

```json5
{
  customManagers: [
    {
      customType: "regex",
      description: "Update raw.githubusercontent.com version tags in mise.toml",
      managerFilePatterns: ["/^mise\\.toml$/"],
      matchStrings: ["https://raw\\.githubusercontent\\.com/(?<depName>[^/]+/[^/]+)/(?<currentValue>v[^/]+)/"],
      datasourceTemplate: "github-tags",
    },
  ],
}
```

This matches all `raw.githubusercontent.com` URLs in `mise.toml` and updates
the version tag (e.g., `v0.1.0`) when a new release is published.

## Per-repo configuration (not included)

Each consuming repository must provide its own:

- **Super-Linter env file** (`.github/config/super-linter.env`) — which
  validators to enable, which `FIX_*` vars to set
- **Linter config files** — `.golangci.yaml`, `.markdownlint.yaml`,
  `.yaml-lint.yml`, `.editorconfig`, etc.
- **Lychee config** (`.github/config/lychee.toml`) — exclusions, timeouts,
  remappings
- **Renovate config** (`.github/renovate.json5`) and committed snapshot
  (`.github/renovate-tracked-deps.json`)

## Versioning

This project uses [Semantic Versioning](https://semver.org/). Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md) and will result in a major version bump.

**Always pin to a specific version** in your `mise.toml` file URLs. Never reference `main` directly as it may contain unreleased breaking changes.
