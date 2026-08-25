# Linter reference

Browse Flint checks by language or purpose. Each linter links to a dedicated
page with its behavior, configuration, and examples.

## Overview

<!-- linter-overview-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

### Languages

| Name                    | Linter                                      | Formatter                                             |
| ----------------------- | ------------------------------------------- | ----------------------------------------------------- |
| C#                      | —                                           | [`dotnet-format`](linters/dotnet-format.md)           |
| Go                      | [`golangci-lint`](linters/golangci-lint.md) | [`gofmt`](linters/gofmt.md)                           |
| Java                    | [`checkstyle`](linters/checkstyle.md)       | [`google-java-format`](linters/google-java-format.md) |
| JavaScript / TypeScript | [`biome`](linters/biome.md)                 | [`biome-format`](linters/biome-format.md)             |
| Kotlin                  | [`ktlint`](linters/ktlint.md)               | [`ktlint`](linters/ktlint.md)                         |
| Python                  | [`ruff`](linters/ruff.md)                   | [`ruff-format`](linters/ruff-format.md)               |
| Rust                    | [`cargo-clippy`](linters/cargo-clippy.md)   | [`cargo-fmt`](linters/cargo-fmt.md)                   |

### Files / Formats

| Name     | Linter                                      | Formatter                                   |
| -------- | ------------------------------------------- | ------------------------------------------- |
| Dotenv   | [`dotenv-linter`](linters/dotenv-linter.md) | [`dotenv-linter`](linters/dotenv-linter.md) |
| JSON     | [`biome`](linters/biome.md)                 | [`biome-format`](linters/biome-format.md)   |
| Markdown | [`rumdl`](linters/rumdl.md)                 | [`rumdl`](linters/rumdl.md)                 |
| Shell    | [`shellcheck`](linters/shellcheck.md)       | [`shfmt`](linters/shfmt.md)                 |
| TOML     | —                                           | [`taplo`](linters/taplo.md)                 |
| XML      | [`xmllint`](linters/xmllint.md)             | —                                           |
| YAML     | [`ryl`](linters/ryl.md)                     | [`ryl`](linters/ryl.md)                     |

### Tooling / CI

| Name                 | Check                                                                 |
| -------------------- | --------------------------------------------------------------------- |
| Dockerfile           | [`hadolint`](linters/hadolint.md)                                     |
| GitHub Actions       | [`actionlint`](linters/actionlint.md) / [`zizmor`](linters/zizmor.md) |
| Kubernetes manifests | [`kube-linter`](linters/kube-linter.md)                               |

### General

| Name            | Check                                                     | Description                                |
| --------------- | --------------------------------------------------------- | ------------------------------------------ |
| EditorConfig    | [`editorconfig-checker`](linters/editorconfig-checker.md) | EditorConfig compliance                    |
| Flint setup     | [`flint-setup`](linters/flint-setup.md)                   | Flint-managed setup and `mise.toml` layout |
| License headers | [`license-header`](linters/license-header.md)             | Required file header text                  |
| Links           | [`lychee`](linters/lychee.md)                             | Broken links                               |
| Renovate        | [`renovate-deps`](linters/renovate-deps.md)               | Dependency update configuration            |
| Spelling        | [`typos`](linters/typos.md)                               | Spelling in source and text files          |

<!-- linter-overview-end -->

## Scopes

### Scope: file

Invoked once per matched file.

### Scope: files

Invoked in one or more batches with Flint-selected files as args. Batches are
used when the rendered command would be large. In `--full` mode, this includes
every eligible tracked file.

### Scope: project

Invoked once with no file args; for checks with patterns set, skipped entirely
if no matching files changed, but runs on the whole project when it does run.
The template checks using this scope are explicit exceptions to Flint's
file-selection contract: `cargo-fmt`, `cargo-clippy`, and `golangci-lint` need
project/package context. `golangci-lint` uses `--new-from-rev` to scope reported
findings to changed code, but may still inspect the project. Native
project-wide exceptions are described below.

### Scope: native

Implemented in-process rather than via a command template. Native checks declare
whether they honor Flint's selected paths. Most do; `kube-linter`,
`renovate-deps`, and `flint-setup` are explicit project-wide exceptions.
Kube-linter recursively selects configured manifests; renovate-deps examines
fixed dependency metadata paths; and flint-setup checks its fixed setup files.
See [How Flint runs checks](check-model.md) for the higher-level model and when
to choose native vs template checks.
