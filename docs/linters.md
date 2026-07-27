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
| Java                    | —                                           | [`google-java-format`](linters/google-java-format.md) |
| JavaScript / TypeScript | [`biome`](linters/biome.md)                 | [`biome-format`](linters/biome-format.md)             |
| Kotlin                  | [`ktlint`](linters/ktlint.md)               | [`ktlint`](linters/ktlint.md)                         |
| Python                  | [`ruff`](linters/ruff.md)                   | [`ruff-format`](linters/ruff-format.md)               |
| Rust                    | [`cargo-clippy`](linters/cargo-clippy.md)   | [`cargo-fmt`](linters/cargo-fmt.md)                   |

### Files / Formats

| Name     | Linter                                | Formatter                                 |
| -------- | ------------------------------------- | ----------------------------------------- |
| JSON     | [`biome`](linters/biome.md)           | [`biome-format`](linters/biome-format.md) |
| Markdown | [`rumdl`](linters/rumdl.md)           | [`rumdl`](linters/rumdl.md)               |
| Shell    | [`shellcheck`](linters/shellcheck.md) | [`shfmt`](linters/shfmt.md)               |
| TOML     | —                                     | [`taplo`](linters/taplo.md)               |
| XML      | [`xmllint`](linters/xmllint.md)       | —                                         |
| YAML     | [`ryl`](linters/ryl.md)               | [`ryl`](linters/ryl.md)                   |

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

Invoked once with all matched files as args; only changed files are passed.

### Scope: project

Invoked once with no file args; for checks with patterns set (e.g.
`cargo-clippy`), skipped entirely if no matching files changed, but runs on the
whole project when it does run. `golangci-lint` is the exception — it uses
`--new-from-rev` to scope analysis to changed code even within the project run.

### Scope: native

Implemented in-process rather than via a command template. These checks may run
without file arguments or use custom orchestration logic. See
[How Flint runs checks](check-model.md) for the higher-level model and when to
choose native vs template checks.
