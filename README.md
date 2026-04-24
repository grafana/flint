<!-- editorconfig-checker-disable -->
<!-- markdownlint-disable MD033 MD041 -->
<p align="center">
  <img src="assets/icon.svg" width="128" height="128" alt="flint logo">
</p>

<h1 align="center">flint — fast lint</h1>

<p align="center">
  <a href="https://github.com/grafana/flint/actions/workflows/lint.yml"><img src="https://github.com/grafana/flint/actions/workflows/lint.yml/badge.svg" alt="Lint"></a>
  <a href="https://github.com/grafana/flint/releases"><img src="https://img.shields.io/github/v/release/grafana/flint" alt="GitHub Release"></a>
</p>
<!-- markdownlint-enable MD033 MD041 -->
<!-- editorconfig-checker-enable -->

Linter runner built for speed and consistency:

- **Fast** — native execution (no Docker), parallel, diff-aware
  (changed files only), opt-in (undeclared tools don't run), small binary
  cached by mise
- **Local == CI** — one binary, one config, identical behavior
- **AI-friendly** — fix silently, surface only what needs review
- **Cross-platform** — Linux, macOS, Windows
- **Autofix** — `--fix` fixes what's fixable; reports what still needs review

Read the [background and principles](docs/why.md).

> [!TIP]
> **Legacy v1** (bash task scripts): see [README-V1.md](README-V1.md).

---

## Getting Started

### Installation

Add `flint` to your repo's `mise.toml`:

```toml
[tools]
"github:grafana/flint" = "0.20.3"
```

Bootstrap a repo with `flint init` (scaffolds config). Install a
pre-commit hook with `flint hook install`.
This is appropriate even if the repo already has an existing `mise.toml`;
`flint init` is not just for greenfield repos. You can choose which linters to
enable during the prompt, or trim the generated tool list afterward if you run
`flint init --yes`.

### mise.toml setup

Flint reads your `[tools]` section to discover which linters to run — declaring
a tool is the opt-in. No separate configuration needed to activate a check: if
`shellcheck` is in `[tools]`, flint runs shellcheck; if it isn't, that check is
skipped. `mise install` puts all declared tools on PATH; flint picks up whatever
is there.

Add the linting tools your project needs alongside the `flint` binary itself:

```toml
[tools]
"github:grafana/flint" = "0.20.3"

# Add whichever linters apply to your repo:
shellcheck              = "v0.11.0"
"github:mvdan/sh"       = "v3.13.1"  # activates shfmt
actionlint              = "1.7.10"
rumdl                   = "0.1.78"
"github:owenlamont/ryl" = "v0.6.0"
biome                   = "2.4.12"
rust                    = "1.87.0"    # activates cargo-fmt + cargo-clippy
go                      = "1.24.0"    # activates gofmt
lychee                  = "0.18.0"    # activates links check
"npm:renovate"          = "39.0.0"    # activates renovate-deps check
```

Then wire up lint tasks:

```toml
[tasks.lint]
description = "Run all lints"
run = "flint run"

[tasks."lint:fix"]
description = "Auto-fix lint issues"
run = "flint run --fix"
```

### CI setup

```yaml
- name: Checkout code
  uses: actions/checkout@...
  with:
    fetch-depth: 0 # needed for merge-base detection

- name: Setup mise
  uses: jdx/mise-action@...

- name: Lint
  env:
    GITHUB_TOKEN: ${{ github.token }}
    GITHUB_HEAD_SHA: ${{ github.event.pull_request.head.sha }}
  run: mise run lint
```

`GITHUB_HEAD_SHA` tells flint which commit is the PR head when running in CI.
`fetch-depth: 0` is required for merge-base detection.

---

## Reference

### CLI

See the [CLI reference](docs/cli.md) for commands and flags.

### Config (`flint.toml`)

Optional. Place in the repo root (or in `FLINT_CONFIG_DIR` — see below).
All settings have defaults.

```toml
[settings]
# base_branch = "dev"                   # branch to diff against; defaults to "main"
exclude = ["CHANGELOG.md", "vendor/**"] # glob patterns — exclude matching files

[checks.links]
config = ".github/config/lychee.toml" # lychee config path
check_all_local = true                # second pass: local links in all files

[checks.renovate-deps]
exclude_managers = ["github-actions", "cargo"] # skip these Renovate managers
```

### `FLINT_CONFIG_DIR`

Set this env var to consolidate config files in one directory (e.g. `.github/config`):

```toml
# mise.toml
[env]
FLINT_CONFIG_DIR = ".github/config"
```

When set, `flint.toml` is loaded from that directory, and each linter that
supports an explicit config path via a CLI flag will have it injected
automatically when the corresponding file exists there (see the "Config file"
column in the table below).
Files that are absent are silently skipped. Some tools still rely on project-root
discovery semantics, and some alternate upstream config locations are rejected to
avoid config drift.

**Note:** `editorconfig-checker`'s config file
(`.editorconfig-checker.json`) controls its own settings, not `.editorconfig`
itself — editorconfig discovery always walks up from the file being linted and
cannot be redirected via a flag.

### Built-in linter registry

Click a name in the table below for details. See the
[linter reference](docs/linters.md) for scope semantics and per-linter notes.

<!-- editorconfig-checker-disable -->
<!-- registry-table-start -->
<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->

| Name                                                           | Description                                                         | Fix |
| -------------------------------------------------------------- | ------------------------------------------------------------------- | --- |
| [`actionlint`](docs/linters.md#actionlint)                     | Lint GitHub Actions workflow files                                  | —   |
| [`biome`](docs/linters.md#biome)                               | Lint JS/TS/JSON files                                               | yes |
| [`biome-format`](docs/linters.md#biome-format)                 | Format JS/TS/JSON files                                             | yes |
| [`cargo-clippy`](docs/linters.md#cargo-clippy)                 | Lint Rust code; runs on all .rs files, not just changed             | yes |
| [`cargo-fmt`](docs/linters.md#cargo-fmt)                       | Format Rust code; runs on all .rs files, not just changed           | yes |
| [`codespell`](docs/linters.md#codespell)                       | Check for common spelling mistakes                                  | yes |
| [`dotnet-format`](docs/linters.md#dotnet-format)               | Format C# code                                                      | yes |
| [`editorconfig-checker`](docs/linters.md#editorconfig-checker) | Check files comply with EditorConfig settings                       | —   |
| [`gofmt`](docs/linters.md#gofmt)                               | Format Go code                                                      | yes |
| [`golangci-lint`](docs/linters.md#golangci-lint)               | Lint Go code; uses --new-from-rev to scope analysis to changed code | —   |
| [`google-java-format`](docs/linters.md#google-java-format)     | Format Java code                                                    | yes |
| [`hadolint`](docs/linters.md#hadolint)                         | Lint Dockerfiles                                                    | —   |
| [`ktlint`](docs/linters.md#ktlint)                             | Lint and format Kotlin code                                         | yes |
| [`license-header`](docs/linters.md#license-header)             | Check source files have the required license header                 | —   |
| [`lychee`](docs/linters.md#lychee)                             | Check for broken links                                              | —   |
| [`renovate-deps`](docs/linters.md#renovate-deps)               | Verify Renovate dependency snapshot is up to date                   | yes |
| [`ruff`](docs/linters.md#ruff)                                 | Lint Python code                                                    | yes |
| [`ruff-format`](docs/linters.md#ruff-format)                   | Format Python code                                                  | yes |
| [`rumdl`](docs/linters.md#rumdl)                               | Lint Markdown files for style and consistency                       | yes |
| [`shellcheck`](docs/linters.md#shellcheck)                     | Lint shell scripts for common mistakes                              | —   |
| [`shfmt`](docs/linters.md#shfmt)                               | Format shell scripts                                                | yes |
| [`taplo`](docs/linters.md#taplo)                               | Format TOML files                                                   | yes |
| [`xmllint`](docs/linters.md#xmllint)                           | Validate XML files are well-formed                                  | —   |
| [`yaml-lint`](docs/linters.md#yaml-lint)                       | Lint YAML files for style and consistency                           | yes |

<!-- registry-table-end -->
<!-- editorconfig-checker-enable -->

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
