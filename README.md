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

Linter runner built for speed, consistency, and low setup friction:

- **Fast** — native execution (no Docker), parallel, diff-aware
  (changed files only), opt-in (undeclared tools don't run), small binary
  cached by mise
- **Local == CI** — one binary, one config, identical behavior
- **Sensible defaults** — `flint init` scaffolds a working setup quickly, and most
  repos can stick with the generated defaults
- **Opinionated config** — Flint chooses canonical config filenames per linter,
  while still letting you keep them in a directory such as `.github/config`
- **AI-friendly** — fix silently, surface only what needs review
- **Separated ownership** — dedicated linters and formatters own their file
  types to avoid overlapping rules and editor-config conflicts
- **Predictable and updatable linter versions** — lint behavior stays stable
  until the repo intentionally updates pinned linter versions, for example via
  Renovate updates to `mise.toml`
- **Cross-platform** — Linux, macOS, Windows
- **Autofix** — `--fix` fixes what's fixable; reports what still needs review

Read the [background and principles](docs/why.md) and
[alternatives/comparisons](docs/alternatives.md).

> [!TIP]
> **Legacy v1** (bash task scripts): see [README-V1.md](README-V1.md).

---

## Getting Started

### Installation

Add `flint` to your repo's `mise.toml`:

```toml
[tools]
"github:grafana/flint" = "0.21.0"
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
ShellCheck's Flint-managed tool key is present in `[tools]`, flint runs
shellcheck; otherwise that check is skipped. `mise install` puts all declared
tools on PATH; flint picks up whatever is there.

Add the linting tools your project needs alongside the `flint` binary itself:

```toml
[tools]
"github:grafana/flint" = "0.21.0"

# Add whichever linters apply to your repo:
"github:koalaman/shellcheck" = "0.11.0"
shfmt                   = "v3.13.1"
actionlint              = "1.7.10"
rumdl                   = "0.1.78"
ruff                    = "0.15.12"
"aqua:owenlamont/ryl"   = "0.6.0"
taplo                   = "0.10.0"
biome                   = "2.4.12"
rust                    = "1.95.0"    # activates cargo-fmt + cargo-clippy
go                      = "1.26.2"    # activates gofmt
lychee                  = "0.22.0"    # activates links check
"npm:renovate"          = "43.141.6"  # activates renovate-deps check
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
    GITHUB_REPOSITORY: ${{ github.repository }}
    GITHUB_BASE_REF: ${{ github.base_ref }}
    GITHUB_HEAD_REF: ${{ github.head_ref }}
    PR_HEAD_REPO: ${{ github.event.pull_request.head.repo.full_name || github.repository }}
    GITHUB_TOKEN: ${{ github.token }}
  run: mise run lint
```

The GitHub environment variables let flint remap base-branch links to the PR
branch when link checking. `fetch-depth: 0` is required for merge-base
detection.

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
automatically when the corresponding canonical Flint-managed file exists there
(see the "Config file" column in the table below).
Files that are absent are silently skipped. Some tools still rely on project-root
discovery semantics, and some alternate upstream config locations are rejected to
avoid config drift. In practice, Flint is opinionated about which config filename
each linter should use, but flexible about the directory those files live in.

> [!NOTE]
> `editorconfig-checker`'s config file (`.editorconfig-checker.json`) controls
> its own settings, not `.editorconfig` itself. Editorconfig discovery always
> walks up from the file being linted and cannot be redirected via a flag.

When a formatter explicitly owns line length for a file type, Flint writes that
carve-out into the shared root `.editorconfig` so editors and
`editorconfig-checker` stay aligned. Today this applies to Markdown via `rumdl`,
Rust via `rustfmt`, and Java via `google-java-format`.

> [!NOTE]
> Biome is also root-discovered on purpose. Flint treats root `biome.jsonc` as
> the canonical Biome config rather than managing it through
> `FLINT_CONFIG_DIR`.

### Built-in linter registry

Click a name in the table below for details. See the
[linter reference](docs/linters.md) for scope semantics and per-linter notes.

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
| [`flint-setup`](docs/linters.md#flint-setup)                   | Keep Flint setup current and mise.toml lint tooling canonical       | yes |
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
| [`ryl`](docs/linters.md#ryl)                                   | Lint YAML files for style and consistency                           | yes |
| [`shellcheck`](docs/linters.md#shellcheck)                     | Lint shell scripts for common mistakes                              | —   |
| [`shfmt`](docs/linters.md#shfmt)                               | Format shell scripts                                                | yes |
| [`taplo`](docs/linters.md#taplo)                               | Format TOML files                                                   | yes |
| [`xmllint`](docs/linters.md#xmllint)                           | Validate XML files are well-formed                                  | —   |

<!-- registry-table-end -->

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
