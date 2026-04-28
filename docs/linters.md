# Built-in linter registry

Every supported check, its config file (when applicable), and its scope. The
[summary table lives in the README](../README.md#built-in-linter-registry).

> [!NOTE]
> Biome is the exception to `FLINT_CONFIG_DIR`: its real CLI does not work
> reliably with a nested managed config, so flint treats root `biome.jsonc` as
> the canonical Biome config. Flint is opinionated here: use JSONC, not
> `biome.json`.

<!-- linter-details-start -->
<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->
## `actionlint`

|             |                                                    |
| ----------- | -------------------------------------------------- |
| Description | Lint GitHub Actions workflow files                 |
| Fix         | no                                                 |
| Binary      | `actionlint`                                       |
| Scope       | [file](#scope-file)                                |
| Patterns    | `.github/workflows/*.yml .github/workflows/*.yaml` |
| Config      | `actionlint.yml`                                   |

## `biome`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint JS/TS/JSON files                  |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scope-file)                    |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

## `biome-format`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Format JS/TS/JSON files                |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scope-file)                    |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

## `cargo-clippy`

|             |                                                         |
| ----------- | ------------------------------------------------------- |
| Description | Lint Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                     |
| Binary      | `cargo-clippy`                                          |
| Scope       | [project](#scope-project)                               |
| Patterns    | `*.rs`                                                  |

## `cargo-fmt`

|             |                                                           |
| ----------- | --------------------------------------------------------- |
| Description | Format Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                       |
| Binary      | `rustfmt`                                                 |
| Scope       | [project](#scope-project)                                 |
| Patterns    | `*.rs`                                                    |
| Config      | `rustfmt.toml`                                            |

## `codespell`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Check for common spelling mistakes |
| Fix         | yes                                |
| Binary      | `codespell`                        |
| Scope       | [files](#scope-files)              |
| Patterns    | `*`                                |
| Config      | `.codespellrc`                     |

## `dotnet-format`

|             |                       |
| ----------- | --------------------- |
| Description | Format C# code        |
| Fix         | yes                   |
| Binary      | `dotnet`              |
| Scope       | [files](#scope-files) |
| Patterns    | `*.cs`                |

## `editorconfig-checker`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Check files comply with EditorConfig settings |
| Fix         | no                                            |
| Binary      | `ec`                                          |
| Scope       | [files](#scope-files)                         |
| Patterns    | `*`                                           |
| Config      | `.editorconfig-checker.json`                  |

## `flint-setup`

|             |                                                               |
| ----------- | ------------------------------------------------------------- |
| Description | Keep Flint setup current and mise.toml lint tooling canonical |
| Fix         | yes                                                           |
| Binary      | (built-in)                                                    |
| Scope       | [special](#scope-special)                                     |
| Patterns    | `mise.toml`                                                   |

Checks the repo's Flint-managed setup state and `mise.toml` layout.

This verifies and fixes Flint-managed setup:

- apply versioned Flint setup migrations
- replace obsolete lint tool keys with their supported successors
- reject unsupported legacy lint tools that need repo migrations
- sort `[tools]` entries into Flint's canonical order
- keep lint-managed tool entries under the `# Linters` header
- keep runtime, SDK, and unknown tool entries above that header

With `--fix`, rewrites Flint-managed config in place and advances
`settings.setup_migration_version` when a migration applies.

## `gofmt`

|             |                     |
| ----------- | ------------------- |
| Description | Format Go code      |
| Fix         | yes                 |
| Binary      | `gofmt`             |
| Scope       | [file](#scope-file) |
| Patterns    | `*.go`              |

## `golangci-lint`

|             |                                                                     |
| ----------- | ------------------------------------------------------------------- |
| Description | Lint Go code; uses --new-from-rev to scope analysis to changed code |
| Fix         | no                                                                  |
| Binary      | `golangci-lint`                                                     |
| Scope       | [project](#scope-project)                                           |
| Patterns    | `*.go`                                                              |
| Config      | `.golangci.yml`                                                     |

## `google-java-format`

|             |                       |
| ----------- | --------------------- |
| Description | Format Java code      |
| Fix         | yes                   |
| Binary      | `google-java-format`  |
| Scope       | [files](#scope-files) |
| Patterns    | `*.java`              |

## `hadolint`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint Dockerfiles                       |
| Fix         | no                                     |
| Binary      | `hadolint`                             |
| Scope       | [file](#scope-file)                    |
| Patterns    | `Dockerfile Dockerfile.* *.dockerfile` |
| Config      | `.hadolint.yaml`                       |

## `ktlint`

|             |                             |
| ----------- | --------------------------- |
| Description | Lint and format Kotlin code |
| Fix         | yes                         |
| Binary      | `ktlint`                    |
| Scope       | [files](#scope-files)       |
| Patterns    | `*.kt *.kts`                |

## `license-header`

|             |                                                     |
| ----------- | --------------------------------------------------- |
| Description | Check source files have the required license header |
| Fix         | no                                                  |
| Binary      | (built-in)                                          |
| Scope       | [special](#scope-special)                           |

## `lychee`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Check for broken links             |
| Fix         | no                                 |
| Binary      | `lychee`                           |
| Scope       | [special](#scope-special)          |
| Config      | via `[checks.links]` in flint.toml |

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires `lychee` in `[tools]`.

Default behavior: checks all links in changed files. When
`check_all_local = true` in `flint.toml`, adds a second pass over local links
in all files — useful when broken internal links from unchanged files also
matter.

Configure via `flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```

## `renovate-deps`

|             |                                                                                                                            |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| Description | Verify Renovate dependency snapshot is up to date                                                                          |
| Fix         | yes                                                                                                                        |
| Binary      | `renovate`                                                                                                                 |
| Scope       | [special](#scope-special)                                                                                                  |
| Patterns    | `renovate.json renovate.json5 .github/renovate.json .github/renovate.json5 .renovaterc .renovaterc.json .renovaterc.json5` |
| Run policy  | adaptive — runs in `--fast-only` only when relevant                                                                        |

Verifies `.github/renovate-tracked-deps.json` is up to date by running
Renovate locally and comparing its output against the committed snapshot.
Requires `renovate` in `[tools]`.

With `--fix`, automatically regenerates and commits the snapshot.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

## `ruff`

|             |                     |
| ----------- | ------------------- |
| Description | Lint Python code    |
| Fix         | yes                 |
| Binary      | `ruff`              |
| Scope       | [file](#scope-file) |
| Patterns    | `*.py`              |
| Config      | `ruff.toml`         |

## `ruff-format`

|             |                     |
| ----------- | ------------------- |
| Description | Format Python code  |
| Fix         | yes                 |
| Binary      | `ruff`              |
| Scope       | [file](#scope-file) |
| Patterns    | `*.py`              |
| Config      | `ruff.toml`         |

## `rumdl`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Lint Markdown files for style and consistency |
| Fix         | yes                                           |
| Binary      | `rumdl`                                       |
| Scope       | [file](#scope-file)                           |
| Patterns    | `*.md`                                        |
| Config      | `.rumdl.toml`                                 |

## `ryl`

|             |                                           |
| ----------- | ----------------------------------------- |
| Description | Lint YAML files for style and consistency |
| Fix         | yes                                       |
| Binary      | `ryl`                                     |
| Scope       | [files](#scope-files)                     |
| Patterns    | `*.yml *.yaml`                            |
| Config      | `.yamllint.yml`                           |

## `shellcheck`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint shell scripts for common mistakes |
| Fix         | no                                     |
| Binary      | `shellcheck`                           |
| Scope       | [file](#scope-file)                    |
| Patterns    | `*.sh *.bash *.bats`                   |
| Config      | `.shellcheckrc`                        |

## `shfmt`

|             |                      |
| ----------- | -------------------- |
| Description | Format shell scripts |
| Fix         | yes                  |
| Binary      | `shfmt`              |
| Scope       | [file](#scope-file)  |
| Patterns    | `*.sh *.bash`        |

## `taplo`

|             |                     |
| ----------- | ------------------- |
| Description | Format TOML files   |
| Fix         | yes                 |
| Binary      | `taplo`             |
| Scope       | [file](#scope-file) |
| Patterns    | `*.toml`            |
| Config      | `.taplo.toml`       |

Formats TOML files with [Taplo](https://taplo.tamasfe.dev/).

This check intentionally stays basic: it uses `taplo fmt --check` for
verification and `taplo fmt` for `--fix`. That keeps behavior aligned with
flint's existing formatter-style checks.

Current caveat: Taplo's published docs currently advertise TOML 1.0.0
support, so treat this check as TOML 1.0-oriented for now.

## `xmllint`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Validate XML files are well-formed |
| Fix         | no                                 |
| Binary      | `xmllint`                          |
| Scope       | [files](#scope-files)              |
| Patterns    | `*.xml`                            |

<!-- linter-details-end -->

## Scopes

### Scope: `file`

Invoked once per matched file.

### Scope: `files`

Invoked once with all matched files as args; only changed files are
  passed

### Scope: `project`

Invoked once with no file args; for checks with patterns set (e.g.
`cargo-clippy`), skipped entirely if no matching files changed, but runs on the
whole project when it does run. `golangci-lint` is the exception — it uses
  `--new-from-rev` to scope analysis to changed code even within the project run.

### Scope: `special`

Implemented in-process rather than via a command template. These checks may run
without file arguments or use custom orchestration logic.

Checks use one of three run policies:

- `fast` — always runs, including in `--fast-only`
- `slow` — skipped by `--fast-only`
- `adaptive` — runs in `--fast-only` only when the changed files are relevant

Use `--fast-only` for local/pre-push feedback and the full set in CI.

**`editorconfig-checker` defers to formatters**: `editorconfig-checker` runs on
all files, but automatically skips file types owned by an active formatter. If
none of those formatters are installed, `editorconfig-checker` checks those
files itself.

**Flint writes shared `.editorconfig` carve-outs for known formatter-owned line
length**: today that means `rumdl` for `*.md`, `rustfmt` for `*.rs`, and
`google-java-format` for `*.java`. Those sections use `max_line_length = off` so editors and
`editorconfig-checker` share the same intent instead of relying on
checker-specific JSON excludes.
