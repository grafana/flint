<!-- editorconfig-checker-disable -->
<p align="center">
  <img src=".idea/icon.svg" width="128" height="128" alt="flint logo">
</p>

<h1 align="center">flint</h1>

<p align="center">
  <a href="https://github.com/grafana/flint/actions/workflows/lint.yml"><img src="https://github.com/grafana/flint/actions/workflows/lint.yml/badge.svg" alt="Lint"></a>
  <a href="https://github.com/grafana/flint/releases"><img src="https://img.shields.io/github/v/release/grafana/flint" alt="GitHub Release"></a>
</p>
<!-- editorconfig-checker-enable -->

A toolbox of reusable [mise](https://mise.jdx.dev/) lint task scripts.
Pick the ones you need — each task is independent and can be adopted
on its own.

**Available tasks:**

| Task                 | Tool                                                          |
| -------------------- | ------------------------------------------------------------- |
| `lint:super-linter`  | [Super-Linter](https://github.com/super-linter/super-linter)  |
| `lint:links`         | [lychee](https://lychee.cli.rs/)                              |
| `lint:renovate-deps` | [Renovate](https://docs.renovatebot.com/) dependency tracking |

## How it works

Flint relies on two tools that each play a distinct role:

### mise — the task runner

[mise](https://mise.jdx.dev/) is a polyglot dev tool manager and task
runner. In the context of flint, mise serves two purposes:

1. **Installing tools.** mise's `[tools]` section pins exact versions
   of the linters each task needs (e.g., `lychee`, `node`,
   `"npm:renovate"`). Running `mise install` gives every developer and
   CI runner the same versions, so local runs are consistent with CI.

2. **Running tasks.** mise downloads task scripts from this repository
   via HTTP, wires them into your project as local commands
   (`mise run lint`, `mise run fix`), and passes flags and environment
   variables through to each script. You don't need to clone flint —
   mise fetches the scripts directly from GitHub URLs pinned in your
   `mise.toml`.

### Renovate — the dependency updater

[Renovate](https://docs.renovatebot.com/) is an automated dependency update bot.
Extending the flint [Renovate preset](#automatic-version-updates-with-renovate)
(`default.json`) is essential for any repository that uses flint — without it,
SHA-pinned flint URLs and `_VERSION` variables in `mise.toml` would never get
updated. The preset ships custom managers that detect these patterns and open
PRs to bump both flint itself and the tools it runs
(e.g., Super-Linter, lychee).

Optionally, the [`lint:renovate-deps`](#lintrenovate-deps) task adds a second
layer: it runs Renovate locally to detect which dependencies Renovate is
tracking, compares this against a committed snapshot, and fails if they
diverge — catching cases where a dependency silently falls off Renovate's
radar.

## Usage

⚠️ **Important**: Always pin to a specific version, never use `main`.
The main branch may contain breaking changes.
See [CHANGELOG.md](CHANGELOG.md) for version history.

Add whichever tasks you need as HTTP remote tasks in your `mise.toml`,
pinned to the commit SHA of a release tag with a version comment:

<!-- editorconfig-checker-disable -->

```toml
# Pick the tasks you need from flint (https://github.com/grafana/flint)
[tasks."lint:super-linter"]
description = "Run Super-Linter on the repository"
file = "https://raw.githubusercontent.com/grafana/flint/5bb3726cfe3305072457c0c4fa85dce5ca154680/tasks/lint/super-linter.sh" # v0.6.0
[tasks."lint:links"]
description = "Check for broken links in changed files + all local links"
file = "https://raw.githubusercontent.com/grafana/flint/5bb3726cfe3305072457c0c4fa85dce5ca154680/tasks/lint/links.sh" # v0.6.0
[tasks."lint:renovate-deps"]
description = "Verify renovate-tracked-deps.json is up to date"
file = "https://raw.githubusercontent.com/grafana/flint/5bb3726cfe3305072457c0c4fa85dce5ca154680/tasks/lint/renovate-deps.py" # v0.6.0
```

<!-- editorconfig-checker-enable -->

The SHA pin ensures the URL is immutable (tag-based URLs can change
if a tag is force-pushed), and the `# v0.3.0` comment tells Renovate
which version is currently pinned.

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

Finally, extend the flint [Renovate preset](#automatic-version-updates-with-renovate)
in your `renovate.json5` to keep flint and its tools up to date:

```json5
{
  extends: ["github>grafana/flint"],
}
```

Without this, SHA-pinned flint URLs and tool versions (e.g.,
`SUPER_LINTER_VERSION`) in `mise.toml` will never receive automated
updates.

## Example

See [grafana/docker-otel-lgtm][example-repo] for a real-world example
of a repository using flint. Its [CONTRIBUTING.md][example-contributing]
describes the developer workflow, and its [mise.toml][example-mise]
shows how the tasks are wired up.

[example-repo]: https://github.com/grafana/docker-otel-lgtm
[example-contributing]: https://github.com/grafana/docker-otel-lgtm/blob/main/CONTRIBUTING.md
[example-mise]: https://github.com/grafana/docker-otel-lgtm/blob/main/mise.toml

## Tasks

### `lint:super-linter`

Runs [Super-Linter](https://github.com/super-linter/super-linter)
via Docker or Podman. Auto-detects the container runtime (prefers
Podman, falls back to Docker) and handles SELinux bind-mount flags
on Fedora.

**mise** fetches this script from the SHA-pinned URL in `mise.toml`
and runs it as `mise run lint:super-linter`. The
`SUPER_LINTER_VERSION` environment variable (set in `mise.toml`)
controls which Super-Linter image is pulled. **Renovate**, via the
flint preset, opens PRs to bump both the flint script URL and the
`SUPER_LINTER_VERSION` value when new versions are available.

**Slim vs full image:** Super-Linter publishes a slim image
(`slim-v8.4.0`) that is ~2 GB smaller than the full image. The slim
image excludes Rust, .NET/C#, PowerShell, and ARM template linters.
Flint defaults to the slim image. To use the full image instead, set
`SUPER_LINTER_VERSION` to the non-prefixed tag (e.g.
`v8.4.0@sha256:...`) and update the Renovate `depName` comment
accordingly (drop the `versioning` override so Renovate uses standard
Docker versioning).

**Flags:**

| Flag        | Description                                                  |
| ----------- | ------------------------------------------------------------ |
| `--autofix` | Enable autofix mode (enables `FIX_*` vars from the env file) |

When autofix is not enabled, all `FIX_*` lines are filtered out of
the env file before running Super-Linter.

**Environment variables:**

<!-- editorconfig-checker-disable -->

| Variable                | Default                           | Required | Description                                                                                   |
| ----------------------- | --------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| `SUPER_LINTER_VERSION`  | —                                 | yes      | Super-Linter image tag (e.g. `slim-v8.4.0@sha256:...` for slim, `v8.4.0@sha256:...` for full) |
| `SUPER_LINTER_ENV_FILE` | `.github/config/super-linter.env` | no       | Path to the Super-Linter env file                                                             |

<!-- editorconfig-checker-enable -->

### `lint:links`

Checks links with [lychee](https://lychee.cli.rs/). By default, it
runs two checks: **all links (local + remote) in modified files** and
**local file links in all files**. This keeps CI fast while catching
both broken remote links in changed content and broken internal links
across the whole repository.

**mise** fetches this script and runs it as `mise run lint:links`.
Lychee is installed via mise's `[tools]` section — add
`lychee = "<version>"` to your `mise.toml`. **Renovate**, via the
flint preset, opens PRs to bump the flint script URL when a new
version is available.

**Flags:**

<!-- editorconfig-checker-disable -->

| Flag                   | Description                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------ |
| `--full`               | Check all links (local + remote) in all files (single run)                           |
| `--base <ref>`         | Base branch to compare against (default: `origin/$GITHUB_BASE_REF` or `origin/main`) |
| `--head <ref>`         | Head commit to compare against (default: `$GITHUB_HEAD_SHA` or `HEAD`)               |
| `--lychee-args <args>` | Extra arguments to pass to lychee                                                    |
| `<file>...`            | Files to check (default: `.`; only used with `--full`)                               |

<!-- editorconfig-checker-enable -->

When running in default mode, if a config change is detected
(matching `LYCHEE_CONFIG_CHANGE_PATTERN`), the script falls back
to `--full` behavior.

**GitHub URL remaps:**

When running on a PR branch, the script automatically remaps GitHub
`/blob/<base-branch>/` and `/tree/<base-branch>/` URLs so that links
to the base branch resolve against the PR branch instead. This
ensures that links like `/blob/main/README.md` don't break when
the file was added or moved in the PR.

For `/blob/` URLs, three ordered remap rules are applied
(lychee uses first-match-wins):

1. **Line-number anchors** (`#L123`): GitHub renders these with
   JavaScript, so lychee can never verify the fragment. The anchor
   is stripped and the file is checked on the PR branch.
2. **Other fragment URLs** (`#section`): Remapped to
   `raw.githubusercontent.com` where lychee can verify the fragment
   in the raw file content (workaround for
   [lychee#1729](https://github.com/lycheeverse/lychee/issues/1729)).
3. **Non-fragment URLs**: Remapped from the base branch to the PR
   branch (the original behavior).

For `/tree/` URLs, rules 1 and 3 apply (no raw remap needed).

Set `LYCHEE_SKIP_GITHUB_REMAPS=true` to disable all GitHub-specific
remaps as an escape hatch if they cause unexpected behavior.

**Environment variables:**

<!-- editorconfig-checker-disable -->

| Variable                       | Default                                                              | Description                                                          |
| ------------------------------ | -------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `LYCHEE_CONFIG`                | `.github/config/lychee.toml`                                         | Path to the lychee config file                                       |
| `LYCHEE_CONFIG_CHANGE_PATTERN` | `^(\.github/config/lychee\.toml\|\.mise/tasks/lint/.*\|mise\.toml)$` | Regular expression for files whose change triggers a full link check |
| `LYCHEE_SKIP_GITHUB_REMAPS`    | unset                                                                | Set to `true` to disable all GitHub URL remaps                       |

<!-- editorconfig-checker-enable -->

**Examples:**

```bash
mise run lint:links                # All links in modified + local links in all files (default)
mise run lint:links --full         # All links in all files
```

### `lint:renovate-deps`

Verifies `.github/renovate-tracked-deps.json` is up to date by
running Renovate locally and parsing its debug logs.

**mise** fetches this script and runs it as `mise run lint:renovate-deps`.
The Renovate CLI is installed via mise's `[tools]` section — add
`node = "<version>"` and `"npm:renovate" = "<version>"` to your
`mise.toml`. **Renovate** plays a dual role here: the flint preset
keeps the script URL up to date, while the script itself runs Renovate
locally in `--platform=local` mode to discover which dependencies
Renovate is tracking and compares them against a committed snapshot.

**Flags:**

| Flag        | Description                                            |
| ----------- | ------------------------------------------------------ |
| `--autofix` | Automatically regenerate and update the committed file |

**Environment variables:**

<!-- editorconfig-checker-disable -->

| Variable                        | Default | Description                                                                         |
| ------------------------------- | ------- | ----------------------------------------------------------------------------------- |
| `RENOVATE_TRACKED_DEPS_EXCLUDE` | unset   | Comma-separated Renovate managers to exclude (e.g. `github-actions,github-runners`) |

<!-- editorconfig-checker-enable -->

#### Why this exists

Renovate silently stops tracking a dependency when it can no longer
parse the version reference (typo in a comment annotation,
unsupported syntax, moved file, etc.). When that happens, the
dependency freezes in place with no PR and no dashboard entry — it
simply disappears from Renovate's radar.

The Dependency Dashboard catches _known_ dependencies that are
pending or in error, but it cannot show you a dependency that
Renovate no longer sees at all. This linter closes that gap by
keeping a committed snapshot of every dependency Renovate tracks
and failing CI when the two diverge.

#### How the linter works

The `lint:renovate-deps` task runs Renovate locally in
`--platform=local` mode, parses its debug log for the
`packageFiles with updates` message, and generates a dependency
list (grouped by file and manager). It then diffs this against the
committed `.github/renovate-tracked-deps.json`:

- If they match → linter passes
- If they differ → linter fails with a unified diff showing which
  dependencies were added or removed
- With `--autofix` flag (or `AUTOFIX=true` env var) → automatically
  regenerates and updates the committed file

#### Typical workflow

- **A dependency disappears** (e.g., someone removes a
  `# renovate:` comment or changes a file that Renovate was
  matching) → CI fails, showing the removed dependency in the diff.
  The author can then decide whether the removal was intentional or
  accidental.

- **A new dependency is added** → CI fails because the committed
  snapshot is stale. Run `mise run fix` (or
  `AUTOFIX=true mise run lint:renovate-deps`) to regenerate and
  update the file, then commit.

- **Routine regeneration** → After any change to `renovate.json5`,
  Dockerfiles, `go.mod`, `package.json`, or other files Renovate
  scans, the linter will detect the change and require
  regeneration.

## How AUTOFIX Works

Lint scripts that support fixing accept an `--autofix` flag. Autofix
can also be enabled via the `AUTOFIX=true` environment variable, which
is how the `fix` meta-task propagates it through the dependency chain.

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

Linters that don't support autofix (like lychee link checker)
silently ignore the `AUTOFIX` environment variable.

## Automatic version updates with Renovate

Flint provides a [Renovate shareable preset](https://docs.renovatebot.com/config-presets/)
with custom managers that automatically update:

- **SHA-pinned flint versions** in `mise.toml`
  (`raw.githubusercontent.com` URLs with commit SHA and version
  comment)
- **`_VERSION` variables** in `mise.toml` (e.g., `SUPER_LINTER_VERSION`)

Add this to your `renovate.json5`:

```json5
{
  extends: ["github>grafana/flint"],
}
```

## Per-repo configuration

Each task expects certain config files that your repository must
provide. You only need the files for the tasks you adopt:

- **`lint:super-linter`** — Super-Linter env file
  (`.github/config/super-linter.env`) to select which validators
  to enable and which `FIX_*` vars to set, plus any linter config
  files (`.golangci.yaml`, `.markdownlint.yaml`, `.yaml-lint.yml`,
  `.editorconfig`, etc.)
- **`lint:links`** — Lychee config
  (`.github/config/lychee.toml`) for exclusions, timeouts,
  remappings
- **`lint:renovate-deps`** — Renovate config
  (`.github/renovate.json5`) and committed snapshot
  (`.github/renovate-tracked-deps.json`)
- **Renovate preset** — Add `"github>grafana/flint"` to your
  `renovate.json5` `extends` array to enable automatic updates of
  flint URLs and tool versions

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

**Always pin to a specific commit SHA** in your `mise.toml` file
URLs with a version comment (e.g., `# v0.6.0`). Never reference
`main` directly as it may contain unreleased breaking changes. To
find the commit SHA for a release tag, run
`git rev-parse v0.6.0`.

## Releasing

Releases are automated via
[Release Please](https://github.com/googleapis/release-please).
When conventional commits land on `main`, Release Please opens
(or updates) a release PR with a changelog.

> **Note:** CI checks don't trigger automatically on release-please
> PRs because they are created with `GITHUB_TOKEN`. To run CI,
> either click **Update branch** or **close and reopen** the PR.
