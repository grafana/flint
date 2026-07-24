# Linter reference

Per-linter Flint behavior, config locations, and notes. Where available, each
linter heading links to the upstream project page, and each config filename
links to the relevant upstream configuration docs.

## Overview

<!-- linter-overview-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

### Languages

| Name                    | Linter                            | Formatter                                   |
| ----------------------- | --------------------------------- | ------------------------------------------- |
| C#                      | —                                 | [`dotnet-format`](#dotnet-format)           |
| Go                      | [`golangci-lint`](#golangci-lint) | [`gofmt`](#gofmt)                           |
| Java                    | —                                 | [`google-java-format`](#google-java-format) |
| JavaScript / TypeScript | [`biome`](#biome)                 | [`biome-format`](#biome-format)             |
| Kotlin                  | [`ktlint`](#ktlint)               | [`ktlint`](#ktlint)                         |
| Python                  | [`ruff`](#ruff)                   | [`ruff-format`](#ruff-format)               |
| Rust                    | [`cargo-clippy`](#cargo-clippy)   | [`cargo-fmt`](#cargo-fmt)                   |

### Files / Formats

| Name     | Linter                            | Formatter                         |
| -------- | --------------------------------- | --------------------------------- |
| Dotenv   | [`dotenv-linter`](#dotenv-linter) | [`dotenv-linter`](#dotenv-linter) |
| JSON     | [`biome`](#biome)                 | [`biome-format`](#biome-format)   |
| Markdown | [`rumdl`](#rumdl)                 | [`rumdl`](#rumdl)                 |
| Shell    | [`shellcheck`](#shellcheck)       | [`shfmt`](#shfmt)                 |
| TOML     | —                                 | [`taplo`](#taplo)                 |
| XML      | [`xmllint`](#xmllint)             | —                                 |
| YAML     | [`ryl`](#ryl)                     | [`ryl`](#ryl)                     |

### Tooling / CI

| Name           | Check                                             |
| -------------- | ------------------------------------------------- |
| Dockerfile     | [`hadolint`](#hadolint)                           |
| GitHub Actions | [`actionlint`](#actionlint) / [`zizmor`](#zizmor) |

### General

| Name            | Check                                           | Description                                |
| --------------- | ----------------------------------------------- | ------------------------------------------ |
| EditorConfig    | [`editorconfig-checker`](#editorconfig-checker) | EditorConfig compliance                    |
| Flint setup     | [`flint-setup`](#flint-setup)                   | Flint-managed setup and `mise.toml` layout |
| License headers | [`license-header`](#license-header)             | Required file header text                  |
| Links           | [`lychee`](#lychee)                             | Broken links                               |
| Renovate        | [`renovate-deps`](#renovate-deps)               | Dependency update configuration            |
| Spelling        | [`typos`](#typos)                               | Spelling in source and text files          |

<!-- linter-overview-end -->

## Linters

<!-- linter-details-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->
### [`actionlint`](https://github.com/rhysd/actionlint)

|          |                                                                                  |
| -------- | -------------------------------------------------------------------------------- |
| Fix      | no                                                                               |
| Binary   | `actionlint`                                                                     |
| Scope    | [file](#scope-file)                                                              |
| Patterns | `.github/workflows/*.yml .github/workflows/*.yaml`                               |
| Config   | [`actionlint.yml`](https://github.com/rhysd/actionlint/blob/main/docs/config.md) |

Lint GitHub Actions workflow files

### [`biome`](https://biomejs.dev/)

|          |                                                              |
| -------- | ------------------------------------------------------------ |
| Fix      | yes                                                          |
| Binary   | `biome`                                                      |
| Scope    | [file](#scope-file)                                          |
| Patterns | `*.json *.jsonc *.js *.ts *.jsx *.tsx`                       |
| Config   | [`biome.jsonc`](https://biomejs.dev/guides/configure-biome/) |

Lint JS/TS/JSON files

### [`biome-format`](https://biomejs.dev/)

|          |                                                              |
| -------- | ------------------------------------------------------------ |
| Fix      | yes                                                          |
| Binary   | `biome`                                                      |
| Scope    | [file](#scope-file)                                          |
| Patterns | `*.json *.jsonc *.js *.ts *.jsx *.tsx`                       |
| Config   | [`biome.jsonc`](https://biomejs.dev/guides/configure-biome/) |

Format JS/TS/JSON files

### [`cargo-clippy`](https://doc.rust-lang.org/clippy/configuration.html)

|          |                           |
| -------- | ------------------------- |
| Fix      | yes                       |
| Binary   | `cargo-clippy`            |
| Scope    | [project](#scope-project) |
| Patterns | `*.rs`                    |

Lint Rust code; runs on all .rs files, not just changed

### [`cargo-fmt`](https://github.com/rust-lang/rustfmt)

|          |                                                                                               |
| -------- | --------------------------------------------------------------------------------------------- |
| Fix      | yes                                                                                           |
| Binary   | `rustfmt`                                                                                     |
| Scope    | [project](#scope-project)                                                                     |
| Patterns | `*.rs`                                                                                        |
| Config   | [`rustfmt.toml`](https://github.com/rust-lang/rustfmt?tab=readme-ov-file#configuring-rustfmt) |

Format Rust code; runs on all .rs files, not just changed

### [`dotenv-linter`](https://github.com/dotenv-linter/dotenv-linter)

|          |                       |
| -------- | --------------------- |
| Fix      | yes                   |
| Binary   | `dotenv-linter`       |
| Scope    | [files](#scope-files) |
| Patterns | `.env .env.* *.env`   |

Lint dotenv environment files without printing their values

Checks only explicit .env-style files: .env, .env.* and files ending in .env.
Flint passes file paths rather than a directory, so an unrelated YAML, Compose,
or application config file is never scanned. Check mode is read-only; fix mode
uses dotenv-linter's no-backup option and remains serialized with other Flint
fixers. Do not commit secret-bearing .env files.

### [`dotnet-format`](https://learn.microsoft.com/dotnet/core/tools/dotnet-format)

|          |                       |
| -------- | --------------------- |
| Fix      | yes                   |
| Binary   | `dotnet`              |
| Scope    | [files](#scope-files) |
| Patterns | `*.cs`                |

Format C# code

### [`editorconfig-checker`](https://github.com/editorconfig-checker/editorconfig-checker)

|          |                                                                                                                               |
| -------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Fix      | no                                                                                                                            |
| Binary   | `ec`                                                                                                                          |
| Scope    | [files](#scope-files)                                                                                                         |
| Patterns | `*`                                                                                                                           |
| Config   | [`.editorconfig-checker.json`](https://github.com/editorconfig-checker/editorconfig-checker?tab=readme-ov-file#configuration) |

Check files comply with EditorConfig settings

`editorconfig-checker` defers to formatters: it runs on all files
but automatically skips file types owned by an active formatter. If
none of those formatters are installed, `editorconfig-checker` checks
those files itself.

Flint writes shared `.editorconfig` carve-outs for known
formatter-owned line length: today that means `rumdl` for `*.md`,
`rustfmt` for `*.rs`, and `google-java-format` for `*.java`. Those
sections use `max_line_length = off` so editors and
`editorconfig-checker` share the same intent instead of relying on
checker-specific JSON excludes. If a matching section already
exists, `flint init` rewrites its `max_line_length` to `off`
instead of leaving a formatter-conflicting numeric value in place.

### `flint-setup`

|          |                         |
| -------- | ----------------------- |
| Fix      | yes                     |
| Binary   | (built-in)              |
| Scope    | [native](#scope-native) |
| Patterns | `mise.toml`             |

Keep Flint setup current and mise.toml lint tooling canonical

Checks the repo's Flint-managed setup state and `mise.toml` layout.

This verifies and fixes Flint-managed setup:

- apply versioned Flint setup migrations
- replace obsolete lint tool keys with their supported successors
- reject unsupported legacy lint tools that need repo migrations
- sort `[tools]` entries into Flint's canonical order
- keep lint-managed tool entries under the `# Linters` header
- keep runtime, SDK, and unknown tool entries above that header

With `--fix`, rewrites Flint-managed config in place and applies any
currently actionable setup migration.

### [`gofmt`](https://pkg.go.dev/cmd/gofmt)

|          |                     |
| -------- | ------------------- |
| Fix      | yes                 |
| Binary   | `gofmt`             |
| Scope    | [file](#scope-file) |
| Patterns | `*.go`              |

Format Go code

### [`golangci-lint`](https://golangci-lint.run/)

|          |                                                                   |
| -------- | ----------------------------------------------------------------- |
| Fix      | no                                                                |
| Binary   | `golangci-lint`                                                   |
| Scope    | [project](#scope-project)                                         |
| Patterns | `*.go`                                                            |
| Config   | [`.golangci.yml`](https://golangci-lint.run/usage/configuration/) |

Lint Go code; uses --new-from-rev to scope analysis to changed code

### [`google-java-format`](https://github.com/google/google-java-format)

|          |                       |
| -------- | --------------------- |
| Fix      | yes                   |
| Binary   | `google-java-format`  |
| Scope    | [files](#scope-files) |
| Patterns | `*.java`              |

Format Java code

### [`hadolint`](https://github.com/hadolint/hadolint)

|          |                                                                                       |
| -------- | ------------------------------------------------------------------------------------- |
| Fix      | no                                                                                    |
| Binary   | `hadolint`                                                                            |
| Scope    | [file](#scope-file)                                                                   |
| Patterns | `Dockerfile Dockerfile.* *.dockerfile`                                                |
| Config   | [`.hadolint.yaml`](https://github.com/hadolint/hadolint?tab=readme-ov-file#configure) |

Lint Dockerfiles

### [`ktlint`](https://github.com/ktlint/ktlint)

|          |                       |
| -------- | --------------------- |
| Fix      | yes                   |
| Binary   | `ktlint`              |
| Scope    | [files](#scope-files) |
| Patterns | `*.kt *.kts`          |

Lint and format Kotlin code

### `license-header`

|        |                                             |
| ------ | ------------------------------------------- |
| Fix    | no                                          |
| Binary | (built-in)                                  |
| Scope  | [native](#scope-native)                     |
| Config | via `[checks.license-header]` in flint.toml |

Check source files have the required license header

Disabled by default. Configure in `flint.toml`:

```toml
[checks.license-header]
text = "SPDX-License-Identifier: Apache-2.0"
patterns = ["*.java", "*.kt"]
lines_to_check = 5
```

- `text` — required header text to find near the top of each file
- `patterns` — glob patterns selecting which files to check
- `lines_to_check` — how many leading lines to search; defaults to `5`

`text` may be multi-line. Flint joins the first `lines_to_check` lines with
newlines and checks whether that text contains the configured header snippet.

### [`lychee`](https://lychee.cli.rs/)

|        |                                    |
| ------ | ---------------------------------- |
| Fix    | no                                 |
| Binary | `lychee`                           |
| Scope  | [native](#scope-native)            |
| Config | via `[checks.links]` in flint.toml |

Check for broken links

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires `lychee` in `[tools]`.

Default behavior: checks all links in changed files. In CI, Flint also adds a
full-repository safeguard pass over local links in all files so broken internal
links in unchanged docs still fail the build. Outside that CI safeguard, setting
`check_all_local = true` in `flint.toml` adds the same local-links-only pass
over all files.

Outside CI, flint also enables a local lychee request cache by default to
speed up repeated runs. Flint stores that cache under `.lychee_cache/` and
creates the directory on first use. Set `FLINT_LYCHEE_SKIP_LOCAL_CACHE=true`
to opt out. If your lychee config already sets `cache = true`, flint leaves
caching to lychee instead.

In CI, `lychee` requires `GITHUB_TOKEN` so GitHub link checks can authenticate.
On GitHub Actions PR runs in changed-file mode, link remaps also require
`GITHUB_REPOSITORY`, `GITHUB_BASE_REF`, `GITHUB_HEAD_REF`, and `PR_HEAD_REPO`.
GitHub Actions provides the first three; set `PR_HEAD_REPO` from
`github.event.pull_request.head.repo.full_name`. The CI local-links safeguard
pass and `--full` do not require the PR remap metadata.

Configure via `flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```

### [`renovate-deps`](https://docs.renovatebot.com/)

|            |                                                                                                                            |
| ---------- | -------------------------------------------------------------------------------------------------------------------------- |
| Fix        | yes                                                                                                                        |
| Binary     | `renovate`                                                                                                                 |
| Scope      | [native](#scope-native)                                                                                                    |
| Patterns   | `renovate.json renovate.json5 .github/renovate.json .github/renovate.json5 .renovaterc .renovaterc.json .renovaterc.json5` |
| Run policy | adaptive — see [when does this run?](linters/renovate-deps.md#when-does-this-run)                                          |

Verify Renovate dependency snapshot is up to date

Verifies `renovate-tracked-deps.json` next to the active Renovate
config is up to date by running Renovate locally and comparing its
output against the committed snapshot.
It also checks that dependencies extracted from different files but
resolving to the same upstream package match the same Renovate
package rules. That catches config splits like `actionlint` vs
`rhysd/actionlint` before Renovate stops grouping them consistently.
Requires `renovate` in `[tools]`.

In CI, `renovate-deps` requires `GITHUB_COM_TOKEN` or `GITHUB_TOKEN`
so Renovate can authenticate GitHub requests. If `GITHUB_COM_TOKEN` is
unset, flint forwards `GITHUB_TOKEN` to Renovate as `GITHUB_COM_TOKEN`.

When `flint init` writes a new `flint.toml`, it includes this section if
`renovate-deps` is selected.

With `--fix`, automatically regenerates and commits the snapshot.
For custom/regex managers, prefer canonical `depNameTemplate` values
for grouping and explicit `packageNameTemplate` values for datasource
lookups when those identities differ.
See [the renovate-deps guide](linters/renovate-deps.md) for examples.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

### [`ruff`](https://docs.astral.sh/ruff/)

|          |                                                           |
| -------- | --------------------------------------------------------- |
| Fix      | yes                                                       |
| Binary   | `ruff`                                                    |
| Scope    | [file](#scope-file)                                       |
| Patterns | `*.py`                                                    |
| Config   | [`ruff.toml`](https://docs.astral.sh/ruff/configuration/) |

Lint Python code

### [`ruff-format`](https://docs.astral.sh/ruff/)

|          |                                                           |
| -------- | --------------------------------------------------------- |
| Fix      | yes                                                       |
| Binary   | `ruff`                                                    |
| Scope    | [file](#scope-file)                                       |
| Patterns | `*.py`                                                    |
| Config   | [`ruff.toml`](https://docs.astral.sh/ruff/configuration/) |

Format Python code

### [`rumdl`](https://rumdl.dev/)

|          |                                                                       |
| -------- | --------------------------------------------------------------------- |
| Fix      | yes                                                                   |
| Binary   | `rumdl`                                                               |
| Scope    | [files](#scope-files)                                                 |
| Patterns | `*.md`                                                                |
| Config   | [`.rumdl.toml`](https://rumdl.dev/mdformat-comparison/#configuration) |

Lint Markdown files for style and consistency

### [`ryl`](https://github.com/owenlamont/ryl)

|          |                                                                                 |
| -------- | ------------------------------------------------------------------------------- |
| Fix      | yes                                                                             |
| Binary   | `ryl`                                                                           |
| Scope    | [files](#scope-files)                                                           |
| Patterns | `*.yml *.yaml`                                                                  |
| Config   | [`.yamllint.yml`](https://yamllint.readthedocs.io/en/stable/configuration.html) |

Lint YAML files for style and consistency

### [`shellcheck`](https://github.com/koalaman/shellcheck)

|          |                                                                                       |
| -------- | ------------------------------------------------------------------------------------- |
| Fix      | no                                                                                    |
| Binary   | `shellcheck`                                                                          |
| Scope    | [file](#scope-file)                                                                   |
| Patterns | `*.sh *.bash *.bats`                                                                  |
| Config   | [`.shellcheckrc`](https://github.com/koalaman/shellcheck/blob/master/shellcheck.1.md) |

Lint shell scripts for common mistakes

### [`shfmt`](https://github.com/mvdan/sh)

|          |                     |
| -------- | ------------------- |
| Fix      | yes                 |
| Binary   | `shfmt`             |
| Scope    | [file](#scope-file) |
| Patterns | `*.sh *.bash`       |

Format shell scripts

### [`taplo`](https://taplo.tamasfe.dev/)

|          |                                                                    |
| -------- | ------------------------------------------------------------------ |
| Fix      | yes                                                                |
| Binary   | `taplo`                                                            |
| Scope    | [file](#scope-file)                                                |
| Patterns | `*.toml`                                                           |
| Config   | [`.taplo.toml`](https://taplo.tamasfe.dev/configuration/file.html) |

Format TOML files

Formats TOML files with [Taplo](https://taplo.tamasfe.dev/).

This check intentionally stays basic: it uses `taplo fmt --check` for
verification and `taplo fmt` for `--fix`. That keeps behavior aligned with
flint's existing formatter-style checks.

Current caveat: Taplo's published docs currently advertise TOML 1.0.0
support, so treat this check as TOML 1.0-oriented for now.

### [`typos`](https://github.com/crate-ci/typos)

|          |                                                                                  |
| -------- | -------------------------------------------------------------------------------- |
| Fix      | yes                                                                              |
| Binary   | `typos`                                                                          |
| Scope    | [files](#scope-files)                                                            |
| Patterns | `*`                                                                              |
| Config   | [`_typos.toml`](https://github.com/crate-ci/typos/blob/master/docs/reference.md) |

Check for common spelling mistakes

### [`xmllint`](https://github.com/jonwiggins/xmloxide)

|          |                       |
| -------- | --------------------- |
| Fix      | no                    |
| Binary   | `xmllint`             |
| Scope    | [files](#scope-files) |
| Patterns | `*.xml`               |

Validate XML files are well-formed

### [`zizmor`](https://github.com/zizmorcore/zizmor)

|          |                                                       |
| -------- | ----------------------------------------------------- |
| Fix      | yes                                                   |
| Binary   | `zizmor`                                              |
| Scope    | [files](#scope-files)                                 |
| Patterns | `.github/workflows/*.yml .github/workflows/*.yaml`    |
| Config   | [`zizmor.yml`](https://docs.zizmor.sh/configuration/) |

Audit GitHub Actions workflows for security issues

zizmor can drift without file changes: its `ref-version-mismatch`
audit resolves pinned action hashes against GitHub's tag API at
run-time. When a maintainer moves a mutable tag (e.g. `v6` advances
to a new patch), workflows pinned to the old commit but commented
`# v6` become inconsistent without any local file change. Flint
scans only files changed in the PR, so drift in untouched workflows
stays invisible until something edits them. Run `flint run --full`
periodically (e.g. weekly `schedule:` workflow) to catch this.
<!-- linter-details-end -->

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
