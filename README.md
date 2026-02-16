<p align="center">
  <img src=".idea/icon.svg" width="128" height="128" alt="flint logo">
</p>

<h1 align="center">flint</h1>

<p align="center">
  <a href="https://github.com/grafana/flint/actions/workflows/lint.yml"><img src="https://github.com/grafana/flint/actions/workflows/lint.yml/badge.svg" alt="Lint"></a>
  <a href="https://github.com/grafana/flint/releases"><img src="https://img.shields.io/github/v/release/grafana/flint" alt="GitHub Release"></a>
</p>

A toolbox of reusable [mise](https://mise.jdx.dev/) lint task scripts. Pick the ones you need — each task is independent and can be adopted on its own.

**Available tasks:**

| Task                 | Tool                                                          |
| -------------------- | ------------------------------------------------------------- |
| `lint:super-linter`  | [Super-Linter](https://github.com/super-linter/super-linter)  |
| `lint:links`         | [lychee](https://lychee.cli.rs/)                              |
| `lint:renovate-deps` | [Renovate](https://docs.renovatebot.com/) dependency tracking |

## Usage

⚠️ **Important**: Always pin to a specific version tag (e.g., `v0.1.0`), never use `main`. The main branch may contain breaking changes. See [CHANGELOG.md](CHANGELOG.md) for version history.

Add whichever tasks you need as HTTP remote tasks in your `mise.toml`:

```toml
# Pick the tasks you need from flint
[tasks."lint:super-linter"]
description = "Run Super-Linter on the repository"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/super-linter.sh"
[tasks."lint:links"]
description = "Check for broken links in changed files + all local links"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/links.sh"
[tasks."lint:renovate-deps"]
description = "Verify renovate-tracked-deps.json is up to date"
file = "https://raw.githubusercontent.com/grafana/flint/v0.1.0/tasks/lint/renovate-deps.py"
```

Then wire up top-level `lint` and `fix` tasks that reference whichever tasks
you adopted (add any project-specific subtasks to the `depends` list):

```toml
[tasks.lint]
description = "Run all lints"
depends = ["lint:super-linter", "lint:links", "lint:renovate-deps"]

[tasks.fix]
description = "Auto-fix lint issues and regenerate tracked deps"
run = "AUTOFIX=true mise run lint"
```

## Tasks

### `lint:super-linter`

Runs [Super-Linter](https://github.com/super-linter/super-linter) via Docker or Podman. Auto-detects the container runtime (prefers Podman, falls back to Docker) and handles SELinux bind-mount flags on Fedora.

**Flags:**

| Flag        | Description                                                  |
| ----------- | ------------------------------------------------------------ |
| `--autofix` | Enable autofix mode (enables `FIX_*` vars from the env file) |

When autofix is not enabled, all `FIX_*` lines are filtered out of the env file before running Super-Linter.

**Environment variables:**

| Variable                | Default                           | Required | Description                                       |
| ----------------------- | --------------------------------- | -------- | ------------------------------------------------- |
| `SUPER_LINTER_VERSION`  | —                                 | yes      | Super-Linter image tag (e.g. `v8.4.0@sha256:...`) |
| `SUPER_LINTER_ENV_FILE` | `.github/config/super-linter.env` | no       | Path to the Super-Linter env file                 |

### `lint:links`

Checks links with [lychee](https://lychee.cli.rs/). By default it runs two checks: **all links (local + remote) in modified files** and **local file links in all files**. This keeps CI fast while catching both broken remote links in changed content and broken internal links across the whole repository.

**Flags:**

| Flag                   | Description                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------ |
| `--full`               | Check all links (local + remote) in all files (single run)                           |
| `--base <ref>`         | Base branch to compare against (default: `origin/$GITHUB_BASE_REF` or `origin/main`) |
| `--head <ref>`         | Head commit to compare against (default: `$GITHUB_HEAD_SHA` or `HEAD`)               |
| `--lychee-args <args>` | Extra arguments to pass to lychee                                                    |
| `<file>...`            | Files to check (default: `.`; only used with `--full`)                               |

When running in default mode, if a config change is detected (matching `LYCHEE_CONFIG_CHANGE_PATTERN`), the script falls back to `--full` behavior.

**Environment variables:**

| Variable                       | Default                                                              | Description                                                          |
| ------------------------------ | -------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `LYCHEE_CONFIG`                | `.github/config/lychee.toml`                                         | Path to the lychee config file                                       |
| `LYCHEE_CONFIG_CHANGE_PATTERN` | `^(\.github/config/lychee\.toml\|\.mise/tasks/lint/.*\|mise\.toml)$` | Regular expression for files whose change triggers a full link check |

**Examples:**

```bash
mise run lint:links                # All links in modified files + local links in all files (default)
mise run lint:links --full         # All links in all files
```

### `lint:renovate-deps`

Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate locally and parsing its debug logs.

**Flags:**

| Flag        | Description                                            |
| ----------- | ------------------------------------------------------ |
| `--autofix` | Automatically regenerate and update the committed file |

**Environment variables:**

| Variable                        | Default | Description                                                                         |
| ------------------------------- | ------- | ----------------------------------------------------------------------------------- |
| `RENOVATE_TRACKED_DEPS_EXCLUDE` | unset   | Comma-separated Renovate managers to exclude (e.g. `github-actions,github-runners`) |

#### Why this exists

Renovate silently stops tracking a dependency when it can no longer parse the version reference (typo in a comment annotation, unsupported syntax, moved file, etc.). When that happens, the dependency freezes in place with no PR and no dashboard entry — it simply disappears from Renovate's radar.

The Dependency Dashboard catches _known_ dependencies that are pending or in error, but it cannot show you a dependency that Renovate no longer sees at all. This linter closes that gap by keeping a committed snapshot of every dependency Renovate tracks and failing CI when the two diverge.

#### How it works

The `lint:renovate-deps` task runs Renovate locally in `--platform=local` mode, parses its debug log for the `packageFiles with updates` message, and generates a dependency list (grouped by file and manager). It then diffs this against the committed `.github/renovate-tracked-deps.json`:

- If they match → linter passes
- If they differ → linter fails with a unified diff showing which dependencies were added or removed
- With `AUTOFIX=true` → automatically regenerates and updates the committed file

#### Typical workflow

- **A dependency disappears** (e.g., someone removes a `# renovate:` comment or changes a file that Renovate was matching) → CI fails, showing the removed dependency in the diff. The author can then decide whether the removal was intentional or accidental.

- **A new dependency is added** → CI fails because the committed snapshot is stale. Run `mise run fix` (or `AUTOFIX=true mise run lint:renovate-deps`) to regenerate and update the file, then commit.

- **Routine regeneration** → After any change to `renovate.json5`, Dockerfiles, `go.mod`, `package.json`, or other files Renovate scans, the linter will detect the change and require regeneration.

## How AUTOFIX Works

Lint scripts that support fixing accept an `--autofix` flag. Autofix can also be enabled via the `AUTOFIX=true` environment variable, which is how the `fix` meta-task propagates it through the dependency chain.

**Check mode** (default):

```bash
mise run lint              # Check all linters, fail on issues
mise run lint:super-linter # Check code style, fail on issues
mise run lint:renovate-deps # Verify tracked deps, fail if out of date
```

**Fix mode:**

```bash
mise run fix                                  # Auto-fix all fixable issues
# Or run individual linters:
mise run lint:super-linter --autofix          # Apply code fixes
mise run lint:renovate-deps --autofix         # Regenerate tracked deps
```

Linters that don't support autofix (like lychee link checker) silently ignore the `AUTOFIX` environment variable.

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
      matchStrings: [
        "https://raw\\.githubusercontent\\.com/(?<depName>[^/]+/[^/]+)/(?<currentValue>v[^/]+)/",
      ],
      datasourceTemplate: "github-tags",
    },
  ],
}
```

This matches all `raw.githubusercontent.com` URLs in `mise.toml` and updates
the version tag (e.g., `v0.1.0`) when a new release is published.

## Per-repo configuration

Each task expects certain config files that your repository must provide. You only need the files for the tasks you adopt:

- **`lint:super-linter`** — Super-Linter env file (`.github/config/super-linter.env`) to select which validators to enable and which `FIX_*` vars to set, plus any linter config files (`.golangci.yaml`, `.markdownlint.yaml`, `.yaml-lint.yml`, `.editorconfig`, etc.)
- **`lint:links`** — Lychee config (`.github/config/lychee.toml`) for exclusions, timeouts, remappings
- **`lint:renovate-deps`** — Renovate config (`.github/renovate.json5`) and committed snapshot (`.github/renovate-tracked-deps.json`)

## Versioning

This project uses [Semantic Versioning](https://semver.org/). Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md) and will result in a major version bump.

**Always pin to a specific version** in your `mise.toml` file URLs. Never reference `main` directly as it may contain unreleased breaking changes.
