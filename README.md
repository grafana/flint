<!-- markdownlint-disable MD033 MD041 -->
<p align="center">
  <img src="assets/icon.svg" width="128" height="128" alt="flint logo">
</p>

<h1 align="center">flint — fast lint</h1>

<p align="center">
  <a href="https://github.com/grafana/flint/actions/workflows/lint.yml"><img src="https://github.com/grafana/flint/actions/workflows/lint.yml/badge.svg" alt="Lint"></a>
  <a href="https://github.com/grafana/flint/releases"><img src="https://img.shields.io/github/v/release/grafana/flint" alt="GitHub Release"></a>
</p>

<p align="center">
  <a href="docs/cli.md">CLI reference</a> ·
  <a href="docs/linters.md">Linters</a> ·
  <a href="docs/why.md">Why flint?</a> ·
  <a href="docs/alternatives.md">Alternatives</a>
</p>
<!-- markdownlint-enable MD033 MD041 -->

Linter runner built for speed, consistency, and low setup friction:

- **Fast** — native execution (no Docker), parallel, diff-aware
  (changed files only), opt-in (undeclared tools don't run), small binary
  cached by mise
- **Local + CI aligned** — one binary, one config model, local defaults tuned
  for day-to-day work and broader coverage in CI
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

### Install

1. Install [mise](https://mise.jdx.dev/).

2. Add Flint to your repo:

   ```bash
   mise use --pin aqua:grafana/flint
   ```

3. Optional: if you use Renovate, create your Renovate config before init.
   Flint can then patch it to include the Flint preset, which helps keep
   linter and Flint updates grouped with less PR noise.

4. Let Flint scaffold the setup:

   ```bash
   mise exec -- flint init
   ```

   During `flint init`, you can:

   - choose which linters to enable
   - add the standard `mise` lint tasks
   - write `flint.toml` when needed
   - create `.github/workflows/lint.yml` when the repo does not already have one
   - add linting guidance to `AGENTS.md` or `CLAUDE.md` (or create `AGENTS.md`)

   If you want non-interactive setup, run `mise exec -- flint init --yes` and
   trim any generated linter pins afterward.

   For a real setup example, see grafana/docker-otel-lgtm's
   [`mise.toml`](https://github.com/grafana/docker-otel-lgtm/blob/main/mise.toml),
   [`flint.toml`](https://github.com/grafana/docker-otel-lgtm/blob/main/.github/config/flint.toml), and
   [lint workflow](https://github.com/grafana/docker-otel-lgtm/blob/main/.github/workflows/lint.yml).

5. Optional: install a git hook that runs `flint run --fix` before each commit:

   ```bash
   mise exec -- flint hook install
   ```

### Using

For normal local use, run:

```bash
mise run lint:fix
```

Flint fixes what it can, tells you when everything is already good, and tells
you what still needs review.

**By default, Flint checks only changed files.** Use `--full` to check every
matching file.

For more commands and flags, see the [CLI reference](docs/cli.md).

> [!NOTE]
> In rare cases (currently only `renovate-deps`) a failure may show up
> only in CI. That is a deliberate performance optimization — see
> [adaptive runs](docs/cli.md#adaptive-runs). When it happens, flint prints the
> command to reproduce locally (usually `--full` or the linter name).

## Linters

<!-- registry-table-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

### Languages

| Name                    | Linter                                           | Formatter                                                  |
| ----------------------- | ------------------------------------------------ | ---------------------------------------------------------- |
| C#                      | —                                                | [`dotnet-format`](docs/linters.md#dotnet-format)           |
| Go                      | [`golangci-lint`](docs/linters.md#golangci-lint) | [`gofmt`](docs/linters.md#gofmt)                           |
| Java                    | —                                                | [`google-java-format`](docs/linters.md#google-java-format) |
| JavaScript / TypeScript | [`biome`](docs/linters.md#biome)                 | [`biome-format`](docs/linters.md#biome-format)             |
| Kotlin                  | [`ktlint`](docs/linters.md#ktlint)               | [`ktlint`](docs/linters.md#ktlint)                         |
| Python                  | [`ruff`](docs/linters.md#ruff)                   | [`ruff-format`](docs/linters.md#ruff-format)               |
| Rust                    | [`cargo-clippy`](docs/linters.md#cargo-clippy)   | [`cargo-fmt`](docs/linters.md#cargo-fmt)                   |

### Files / Formats

| Name     | Linter                                     | Formatter                                      |
| -------- | ------------------------------------------ | ---------------------------------------------- |
| JSON     | [`biome`](docs/linters.md#biome)           | [`biome-format`](docs/linters.md#biome-format) |
| Markdown | [`rumdl`](docs/linters.md#rumdl)           | [`rumdl`](docs/linters.md#rumdl)               |
| Shell    | [`shellcheck`](docs/linters.md#shellcheck) | [`shfmt`](docs/linters.md#shfmt)               |
| TOML     | —                                          | [`taplo`](docs/linters.md#taplo)               |
| XML      | [`xmllint`](docs/linters.md#xmllint)       | —                                              |
| YAML     | [`ryl`](docs/linters.md#ryl)               | [`ryl`](docs/linters.md#ryl)                   |

### Tooling / CI

| Name           | Check                                      |
| -------------- | ------------------------------------------ |
| Dockerfile     | [`hadolint`](docs/linters.md#hadolint)     |
| GitHub Actions | [`actionlint`](docs/linters.md#actionlint) |

### General

| Name            | Check                                                          | Description                                |
| --------------- | -------------------------------------------------------------- | ------------------------------------------ |
| EditorConfig    | [`editorconfig-checker`](docs/linters.md#editorconfig-checker) | EditorConfig compliance                    |
| Flint setup     | [`flint-setup`](docs/linters.md#flint-setup)                   | Flint-managed setup and `mise.toml` layout |
| License headers | [`license-header`](docs/linters.md#license-header)             | Required file header text                  |
| Links           | [`lychee`](docs/linters.md#lychee)                             | Broken links                               |
| Renovate        | [`renovate-deps`](docs/linters.md#renovate-deps)               | Dependency update configuration            |
| Spelling        | [`typos`](docs/linters.md#typos)                               | Spelling in source and text files          |

<!-- registry-table-end -->

## FAQ

### How does Flint know which linters to run?

Flint activates checks from your repo's `mise.toml`: if a Flint-managed tool is
declared there, that check is active; if it is not declared, Flint skips it.

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
