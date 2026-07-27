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
  <a href="docs/check-model.md">How checks work</a> ·
  <a href="docs/linters.md">Linters</a> ·
  <a href="docs/why.md">Why flint?</a> ·
  <a href="docs/alternatives.md">Alternatives</a>
</p>
<!-- markdownlint-enable MD033 MD041 -->

Flint is a fast, simple lint runner that doesn't slow down your AI coding.

> 📖 Read the blog post: [Flint — a linter setup that doesn't slow down your AI agent](https://medium.com/grafana-labs/flint-a-linter-setup-that-doesnt-slow-down-your-ai-agent-e3a85044c4c2)

- **Fast** — native execution (no Docker), parallel, diff-aware
  (changed files only), opt-in (undeclared tools don't run), small binary
  cached by mise
- **Local + CI aligned** — one binary, one config model, local defaults tuned
  for day-to-day work and broader coverage in CI
- **Sensible defaults** — `flint init` scaffolds a working setup quickly, and most
  repos can stick with the generated defaults
- **Opinionated config** — Flint chooses canonical config filenames per linter,
  while still letting you keep them in a directory such as `.github/config`
- **AI-friendly** — quiet by default: clean runs print nothing, `--fix`
  surfaces only what still needs action
- **Separated ownership** — dedicated linters and formatters own their file
  types to avoid overlapping rules and editor-config conflicts
- **Predictable and updatable linter versions** — lint behavior stays stable
  until the repo intentionally updates pinned linter versions, for example via
  Renovate updates to `mise.toml`
- **Cross-platform** — Linux, macOS, Windows
- **Autofix** — `--fix` fixes what's fixable; reports what still needs review

Read the [background and principles](docs/why.md) and
[alternatives/comparisons](docs/alternatives.md).

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

   > [!IMPORTANT]
   > Flint's shared `default.json` preset now targets the current setup only.
   > It no longer ships the legacy custom managers that updated SHA-pinned
   > `raw.githubusercontent.com/.../<sha>/... # vX.Y.Z` references or
   > `*_VERSION` variables in `mise.toml`. If your repo still relies on those
   > v1 patterns, keep your own custom managers for them before extending the
   > preset.

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
   [CI workflow](https://github.com/grafana/docker-otel-lgtm/blob/main/.github/workflows/ci.yml).

5. Optional: install a git hook that runs `flint run --fix` before each commit:

   ```bash
   mise exec -- flint hook install
   ```

### Using

For normal local use, run:

```bash
mise run lint:fix
```

Flint is built to be quiet. A clean run prints nothing. `--fix` silently fixes
what it can and prints what still needs action — review items plus a reminder
to commit any fixes:

```text
[shellcheck]

In bad.sh line 2:
echo $1
     ^-- SC2086 (info): Double quote to prevent globbing and word splitting.
...
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
```

Terse enough for AI agents, nice for humans too.

**By default, Flint checks only changed tracked files.** Use `--full` to check
every matching tracked file. Flint also skips files marked
`linguist-generated` in `.gitattributes`; prefer that over Flint-only excludes
so GitHub and other tools can reuse the same metadata.

For more commands and flags, see the [CLI reference](docs/cli.md).

> [!NOTE]
> In rare cases (currently only `renovate-deps`) a failure may show up
> only in CI. That is a deliberate performance optimization — see
> [adaptive runs](docs/cli.md#adaptive-runs). When it happens, flint prints the
> command to reproduce locally (usually `--full` or the linter name).

For Flint contributor workflow and local testing tips, see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Linters

<!-- registry-table-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

### Languages

| Name                    | Linter                                           | Formatter                                                  |
| ----------------------- | ------------------------------------------------ | ---------------------------------------------------------- |
| C#                      | —                                                | [`dotnet-format`](docs/linters/dotnet-format.md)           |
| Go                      | [`golangci-lint`](docs/linters/golangci-lint.md) | [`gofmt`](docs/linters/gofmt.md)                           |
| Java                    | [`checkstyle`](docs/linters/checkstyle.md)       | [`google-java-format`](docs/linters/google-java-format.md) |
| JavaScript / TypeScript | [`biome`](docs/linters/biome.md)                 | [`biome-format`](docs/linters/biome-format.md)             |
| Kotlin                  | [`ktlint`](docs/linters/ktlint.md)               | [`ktlint`](docs/linters/ktlint.md)                         |
| Python                  | [`ruff`](docs/linters/ruff.md)                   | [`ruff-format`](docs/linters/ruff-format.md)               |
| Rust                    | [`cargo-clippy`](docs/linters/cargo-clippy.md)   | [`cargo-fmt`](docs/linters/cargo-fmt.md)                   |

### Files / Formats

| Name     | Linter                                     | Formatter                                      |
| -------- | ------------------------------------------ | ---------------------------------------------- |
| JSON     | [`biome`](docs/linters/biome.md)           | [`biome-format`](docs/linters/biome-format.md) |
| Markdown | [`rumdl`](docs/linters/rumdl.md)           | [`rumdl`](docs/linters/rumdl.md)               |
| Shell    | [`shellcheck`](docs/linters/shellcheck.md) | [`shfmt`](docs/linters/shfmt.md)               |
| TOML     | —                                          | [`taplo`](docs/linters/taplo.md)               |
| XML      | [`xmllint`](docs/linters/xmllint.md)       | —                                              |
| YAML     | [`ryl`](docs/linters/ryl.md)               | [`ryl`](docs/linters/ryl.md)                   |

### Tooling / CI

| Name                 | Check                                                                           |
| -------------------- | ------------------------------------------------------------------------------- |
| Dockerfile           | [`hadolint`](docs/linters/hadolint.md)                                          |
| GitHub Actions       | [`actionlint`](docs/linters/actionlint.md) / [`zizmor`](docs/linters/zizmor.md) |
| Kubernetes manifests | [`kube-linter`](docs/linters/kube-linter.md)                                    |

### General

| Name            | Check                                                          | Description                                |
| --------------- | -------------------------------------------------------------- | ------------------------------------------ |
| EditorConfig    | [`editorconfig-checker`](docs/linters/editorconfig-checker.md) | EditorConfig compliance                    |
| Flint setup     | [`flint-setup`](docs/linters/flint-setup.md)                   | Flint-managed setup and `mise.toml` layout |
| License headers | [`license-header`](docs/linters/license-header.md)             | Required file header text                  |
| Links           | [`lychee`](docs/linters/lychee.md)                             | Broken links                               |
| Renovate        | [`renovate-deps`](docs/linters/renovate-deps.md)               | Dependency update configuration            |
| Spelling        | [`typos`](docs/linters/typos.md)                               | Spelling in source and text files          |

<!-- registry-table-end -->

## FAQ

### How does Flint know which linters to run?

Flint activates checks from your repo's `mise.toml`: if a Flint-managed tool is
declared there, that check is active; if it is not declared, Flint skips it.

### What's the best way to exclude files from linting?

Flint never lints untracked files. This question is about files that are
tracked in git.

There are three main options:

1. Mark generated files in `.gitattributes` with `linguist-generated`
2. Add repo-wide Flint excludes in `flint.toml` via `settings.exclude`
3. Use a tool-specific exclude in the linter's own config, when that tool
   needs behavior Flint should not manage globally

**Recommended:** use `.gitattributes` for generated files whenever possible.
That lets Flint, GitHub, and other tools share the same generated-file
metadata. See the [CLI reference](docs/cli.md#changed-file-and-baseline-runs)
for details.

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
