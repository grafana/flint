# Built-in linter registry

Every supported check, its config file (when applicable), and its scope. The
[summary table lives in the README](../README.md#built-in-linter-registry).

> [!NOTE]
> Biome is the exception to `FLINT_CONFIG_DIR`: its real CLI does not work
> reliably with a nested managed config, so flint treats root `biome.jsonc` as
> the canonical Biome config. Flint is opinionated here: use JSONC, not
> `biome.json`.

<!-- editorconfig-checker-disable -->
<!-- markdownlint-disable MD013 -->
<!-- linter-details-start -->
<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->
## `actionlint`

|             |                                                    |
| ----------- | -------------------------------------------------- |
| Description | Lint GitHub Actions workflow files                 |
| Fix         | no                                                 |
| Binary      | `actionlint`                                       |
| Scope       | [file](#scopes)                                    |
| Patterns    | `.github/workflows/*.yml .github/workflows/*.yaml` |
| Config      | `actionlint.yml`                                   |

## `biome`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint JS/TS/JSON files                  |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

## `biome-format`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Format JS/TS/JSON files                |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

## `cargo-clippy`

|             |                                                         |
| ----------- | ------------------------------------------------------- |
| Description | Lint Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                     |
| Binary      | `cargo-clippy`                                          |
| Scope       | [project](#scopes)                                      |
| Patterns    | `*.rs`                                                  |

## `cargo-fmt`

|             |                                                           |
| ----------- | --------------------------------------------------------- |
| Description | Format Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                       |
| Binary      | `rustfmt`                                                 |
| Scope       | [project](#scopes)                                        |
| Patterns    | `*.rs`                                                    |
| Config      | `rustfmt.toml`                                            |

## `codespell`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Check for common spelling mistakes |
| Fix         | yes                                |
| Binary      | `codespell`                        |
| Scope       | [files](#scopes)                   |
| Patterns    | `*`                                |
| Config      | `.codespellrc`                     |

## `dotnet-format`

|             |                  |
| ----------- | ---------------- |
| Description | Format C# code   |
| Fix         | yes              |
| Binary      | `dotnet`         |
| Scope       | [files](#scopes) |
| Patterns    | `*.cs`           |

## `editorconfig-checker`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Check files comply with EditorConfig settings |
| Fix         | no                                            |
| Binary      | `ec`                                          |
| Scope       | [files](#scopes)                              |
| Patterns    | `*`                                           |
| Config      | `.editorconfig-checker.json`                  |

## `gofmt`

|             |                 |
| ----------- | --------------- |
| Description | Format Go code  |
| Fix         | yes             |
| Binary      | `gofmt`         |
| Scope       | [file](#scopes) |
| Patterns    | `*.go`          |

## `golangci-lint`

|             |                                                                     |
| ----------- | ------------------------------------------------------------------- |
| Description | Lint Go code; uses --new-from-rev to scope analysis to changed code |
| Fix         | no                                                                  |
| Binary      | `golangci-lint`                                                     |
| Scope       | [project](#scopes)                                                  |
| Patterns    | `*.go`                                                              |
| Config      | `.golangci.yml`                                                     |

## `google-java-format`

|             |                      |
| ----------- | -------------------- |
| Description | Format Java code     |
| Fix         | yes                  |
| Binary      | `google-java-format` |
| Scope       | [files](#scopes)     |
| Patterns    | `*.java`             |

## `hadolint`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint Dockerfiles                       |
| Fix         | no                                     |
| Binary      | `hadolint`                             |
| Scope       | [file](#scopes)                        |
| Patterns    | `Dockerfile Dockerfile.* *.dockerfile` |
| Config      | `.hadolint.yaml`                       |

## `ktlint`

|             |                             |
| ----------- | --------------------------- |
| Description | Lint and format Kotlin code |
| Fix         | yes                         |
| Binary      | `ktlint`                    |
| Scope       | [files](#scopes)            |
| Patterns    | `*.kt *.kts`                |

## `license-header`

|             |                                                     |
| ----------- | --------------------------------------------------- |
| Description | Check source files have the required license header |
| Fix         | no                                                  |
| Binary      | (built-in)                                          |
| Scope       | [special](#scopes)                                  |

## `lychee`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Check for broken links             |
| Fix         | no                                 |
| Binary      | `lychee`                           |
| Scope       | [special](#scopes)                 |
| Config      | via `[checks.links]` in flint.toml |

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires `lychee` in `[tools]`.

Default behavior: checks all links in changed files. When `check_all_local = true` in `flint.toml`, adds a second pass over local links in all files — useful when broken internal links from unchanged files also matter.

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
| Scope       | [special](#scopes)                                                                                                         |
| Patterns    | `renovate.json renovate.json5 .github/renovate.json .github/renovate.json5 .renovaterc .renovaterc.json .renovaterc.json5` |
| Run policy  | adaptive — runs in `--fast-only` only when relevant                                                                        |

Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate locally and comparing its output against the committed snapshot. Requires `renovate` in `[tools]`.

With `--fix`, automatically regenerates and commits the snapshot.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

## `ruff`

|             |                  |
| ----------- | ---------------- |
| Description | Lint Python code |
| Fix         | yes              |
| Binary      | `ruff`           |
| Scope       | [file](#scopes)  |
| Patterns    | `*.py`           |
| Config      | `ruff.toml`      |

## `ruff-format`

|             |                    |
| ----------- | ------------------ |
| Description | Format Python code |
| Fix         | yes                |
| Binary      | `ruff`             |
| Scope       | [file](#scopes)    |
| Patterns    | `*.py`             |
| Config      | `ruff.toml`        |

## `rumdl`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Lint Markdown files for style and consistency |
| Fix         | yes                                           |
| Binary      | `rumdl`                                       |
| Scope       | [file](#scopes)                               |
| Patterns    | `*.md`                                        |
| Config      | `.rumdl.toml`                                 |

## `shellcheck`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint shell scripts for common mistakes |
| Fix         | no                                     |
| Binary      | `shellcheck`                           |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.sh *.bash *.bats`                   |
| Config      | `.shellcheckrc`                        |

## `shfmt`

|             |                      |
| ----------- | -------------------- |
| Description | Format shell scripts |
| Fix         | yes                  |
| Binary      | `shfmt`              |
| Scope       | [file](#scopes)      |
| Patterns    | `*.sh *.bash`        |

## `taplo`

|             |                   |
| ----------- | ----------------- |
| Description | Format TOML files |
| Fix         | yes               |
| Binary      | `taplo`           |
| Scope       | [file](#scopes)   |
| Patterns    | `*.toml`          |
| Config      | `.taplo.toml`     |

Formats TOML files with [Taplo](https://taplo.tamasfe.dev/).

This check intentionally stays basic: it uses `taplo fmt --check` for verification and `taplo fmt` for `--fix`. That keeps behavior aligned with flint's existing formatter-style checks.

Current caveat: Taplo's published docs currently advertise TOML 1.0.0 support, so treat this check as TOML 1.0-oriented for now.

## `xmllint`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Validate XML files are well-formed |
| Fix         | no                                 |
| Binary      | `xmllint`                          |
| Scope       | [files](#scopes)                   |
| Patterns    | `*.xml`                            |

## `yaml-lint`

|             |                                           |
| ----------- | ----------------------------------------- |
| Description | Lint YAML files for style and consistency |
| Fix         | yes                                       |
| Binary      | `ryl`                                     |
| Scope       | [files](#scopes)                          |
| Patterns    | `*.yml *.yaml`                            |
| Config      | `.yamllint.yml`                           |

<!-- linter-details-end -->
<!-- markdownlint-enable MD013 -->
<!-- editorconfig-checker-enable -->

## Scopes

- `file` — invoked once per matched file
- `files` — invoked once with all matched files as args; only changed files are
  passed
- `project` — invoked once with no file args; for checks with patterns set
  (e.g. `cargo-clippy`), skipped entirely if no matching files changed, but
  runs on the whole project when it does run. `golangci-lint` is the
  exception — it uses
  `--new-from-rev` to scope analysis to changed code even within the project run.

Checks use one of three run policies:

- `fast` — always runs, including in `--fast-only`
- `slow` — skipped by `--fast-only`
- `adaptive` — runs in `--fast-only` only when the changed files are relevant

Use `--fast-only` for local/pre-push feedback and the full set in CI.

**`editorconfig-checker` defers to formatters**: `editorconfig-checker` runs on
all files, but automatically skips file types owned by an active formatter. If
none of those formatters are installed, `editorconfig-checker` checks those
files itself.

**`flint init` / `flint update` writes shared `.editorconfig` carve-outs for
known formatter-owned line length**: today that means `rumdl` for `*.md` and
`google-java-format` for `*.java`. Those sections use `max_line_length = off`
so editors and `editorconfig-checker` share the same intent instead of relying
on checker-specific JSON excludes.
