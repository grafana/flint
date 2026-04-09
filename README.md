<!-- editorconfig-checker-disable -->
<!-- markdownlint-disable MD033 MD041 -->
<p align="center">
  <img src=".idea/icon.svg" width="128" height="128" alt="flint logo">
</p>

<h1 align="center">flint — fast lint</h1>

<p align="center">
  <a href="https://github.com/grafana/flint/actions/workflows/lint.yml"><img src="https://github.com/grafana/flint/actions/workflows/lint.yml/badge.svg" alt="Lint"></a>
  <a href="https://github.com/grafana/flint/releases"><img src="https://img.shields.io/github/v/release/grafana/flint" alt="GitHub Release"></a>
</p>
<!-- markdownlint-enable MD033 MD041 -->
<!-- editorconfig-checker-enable -->

Linter runner built for speed and consistency:

- **Fast** — native execution (no Docker), parallel, diff-aware (changed files only), opt-in (undeclared tools don't run), small binary cached by mise
- **Local == CI** — one binary, one config, identical behavior
- **AI-friendly** — fix silently, surface only what needs review
- **Cross-platform** — Linux, macOS, Windows
- **Autofix** — `--fix` fixes what's fixable; reports what still needs review

See [Why / Principles](#why) for background.

> **Legacy v1** (bash task scripts): see [README-V1.md](README-V1.md).

---

## Getting Started

### Installation

Add `flint` to your repo's `mise.toml` (once published):

```toml
[tools]
flint = "0.x.y"
```

Until the first release, build from source:

```bash
git clone https://github.com/grafana/flint
cd flint
cargo build --release
# Binary at target/release/flint
```

### mise.toml setup

Flint reads your `[tools]` section to discover which linters to run — declaring
a tool is the opt-in. No separate configuration needed to activate a check: if
`shellcheck` is in `[tools]`, flint runs shellcheck; if it isn't, that check is
skipped. `mise install` puts all declared tools on PATH; flint picks up whatever
is there.

Add the linting tools your project needs alongside the `flint` binary itself:

```toml
[tools]
flint   = "0.x.y"

# Add whichever linters apply to your repo:
shellcheck  = "v0.11.0"
shfmt       = "v3.12.0"
actionlint  = "1.7.10"
"npm:markdownlint-cli2" = "0.47.0"
"npm:prettier"         = "3.5.0"
rust        = "1.87.0"   # activates cargo-fmt + cargo-clippy
go          = "1.24.0"   # activates gofmt
lychee      = "0.18.0"   # activates links check
"npm:renovate" = "39.0.0" # activates renovate-deps check (slow)
```

Then wire up lint tasks:

```toml
[tasks.lint]
description = "Run all lints"
run = "flint run"

[tasks."lint:pre-commit"]
description = "Fast auto-fix lint pass — for pre-push hooks and agentic pipelines"
run = "flint run --fix --fast-only"

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

```text
flint run [OPTIONS] [LINTERS...]
flint linters
flint version
```

Commands and flags follow [golangci-lint](https://golangci-lint.run/) conventions — teams already using it don't need to re-learn the interface.

`flint run` flags:

| Flag                 | Description                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------------- |
| `--fix`              | Fix what's fixable, report what still needs review; exit 1 if anything changed or needs review |
| `--full`             | Lint all files instead of only changed files                                                   |
| `--fast-only`        | Skip slow checks (e.g. `renovate-deps`). Overridden by explicit linter names.                  |
| `--short`            | Compact summary output, no per-check noise                                                     |
| `--verbose`          | Show all linter output, not just failures                                                      |
| `--new-from-rev REV` | Diff base (default: merge base with base branch)                                               |
| `--to-ref REF`       | Diff head (default: HEAD)                                                                      |

Every flag has an env var equivalent: `FLINT_FIX`, `FLINT_FULL`, `FLINT_FAST_ONLY`,
`FLINT_VERBOSE`, `FLINT_SHORT`, `FLINT_NEW_FROM_REV`, `FLINT_TO_REF`.

#### Intended use by context

| Context                      | Command                                | Why                                                               |
| ---------------------------- | -------------------------------------- | ----------------------------------------------------------------- |
| Interactive development      | `flint run` or `flint run --fast-only` | Full output so you can read the details                           |
| Human wanting a summary      | `flint run --short`                    | Compact output, no per-check noise                                |
| Pre-push hook (CC / agentic) | `flint run --fix --fast-only`          | Fixes what it can silently, surfaces only what needs human review |
| CI                           | `flint run`                            | Full output for humans reading CI logs                            |

**`--short` output** — failed checks partitioned by fixability, fixable ones
expressed as the exact command to run:

```text
flint: 2 checks failed — flint run --fix prettier cargo-fmt | review: shellcheck
```

**`--fix` output** — fixes what's fixable, then prints the full output of
any checks that still need review, followed by a summary line. Exits 1 if
anything was fixed (so the caller commits the fixes before pushing) or if
anything still needs review. Exits 0 only if everything was already clean:

```text
[shellcheck]

In bad.sh line 2:
echo $1
     ^-- SC2086 (info): Double quote to prevent globbing and word splitting.
...
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
```

Pass one or more linter names to run only those:

```bash
flint run shellcheck shfmt        # run only shellcheck and shfmt
flint run --fix prettier          # fix only prettier
```

`flint linters` shows every check with its status:

```text
NAME            BINARY          STATUS     SPEED  PATTERNS
-------------------------------------------------------------------
shellcheck      shellcheck      installed  fast   *.sh *.bash *.bats
cargo-fmt       cargo-fmt       missing    fast   *.rs
renovate-deps   renovate        installed  slow
...
```

### Config (`flint.toml`)

Optional. Place in the repo root (or in `FLINT_CONFIG_DIR` — see below). All settings have defaults.

```toml
[settings]
base_branch = "main"                           # branch to diff against
exclude = ["CHANGELOG.md", "vendor/**"]       # glob patterns — exclude matching files

[checks.links]
config = ".github/config/lychee.toml"         # lychee config path
check_all_local = true                         # second pass: local links in all files

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

When set, `flint.toml` is loaded from that directory, and each linter that supports
an explicit config file path via a CLI flag will have it injected automatically when
the corresponding file exists there (see the "Config file" column in the table below).
Files that are absent are silently skipped — existing project-root configs remain in
effect.

**Note:** `editorconfig-checker`'s config file (`.editorconfig-checker.json`) controls its own settings,
not `.editorconfig` itself — editorconfig discovery always walks up from the file
being linted and cannot be redirected via a flag.

### Built-in linter registry

<!-- editorconfig-checker-disable -->
<!-- registry-table-start -->
<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->

| Name                   | Description                                                         | Fix |
| ---------------------- | ------------------------------------------------------------------- | --- |
| `shellcheck`           | Lint shell scripts for common mistakes                              | —   |
| `shfmt`                | Format shell scripts                                                | yes |
| `markdownlint-cli2`    | Lint Markdown files for style and consistency                       | yes |
| `prettier`             | Format Markdown and YAML files                                      | yes |
| `actionlint`           | Lint GitHub Actions workflow files                                  | —   |
| `hadolint`             | Lint Dockerfiles                                                    | —   |
| `xmllint`              | Validate XML files are well-formed                                  | —   |
| `codespell`            | Check for common spelling mistakes                                  | yes |
| `editorconfig-checker` | Check files comply with EditorConfig settings                       | —   |
| `golangci-lint`        | Lint Go code; uses --new-from-rev to scope analysis to changed code | —   |
| `ruff`                 | Lint Python code                                                    | yes |
| `ruff-format`          | Format Python code                                                  | yes |
| `biome`                | Lint JS/TS/JSON files                                               | yes |
| `biome-format`         | Format JS/TS/JSON files                                             | yes |
| `cargo-clippy`         | Lint Rust code; runs on all .rs files, not just changed             | yes |
| `cargo-fmt`            | Format Rust code; runs on all .rs files, not just changed           | yes |
| `gofmt`                | Format Go code                                                      | yes |
| `google-java-format`   | Format Java code                                                    | yes |
| `ktlint`               | Lint and format Kotlin code                                         | yes |
| `dotnet-format`        | Format C# code                                                      | yes |
| `lychee`               | Check for broken links                                              | —   |
| `renovate-deps`        | Verify Renovate dependency snapshot is up to date                   | yes |
| `license-header`       | Check source files have the required license header                 | —   |

#### `shellcheck`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint shell scripts for common mistakes |
| Fix         | no                                     |
| Binary      | `shellcheck`                           |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.sh *.bash *.bats`                   |
| Config      | `.shellcheckrc`                        |

#### `shfmt`

|             |                      |
| ----------- | -------------------- |
| Description | Format shell scripts |
| Fix         | yes                  |
| Binary      | `shfmt`              |
| Scope       | [file](#scopes)      |
| Patterns    | `*.sh *.bash`        |

#### `markdownlint-cli2`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Lint Markdown files for style and consistency |
| Fix         | yes                                           |
| Binary      | `markdownlint-cli2`                           |
| Scope       | [file](#scopes)                               |
| Patterns    | `*.md`                                        |
| Config      | `.markdownlint.jsonc`                         |

#### `prettier`

|             |                                |
| ----------- | ------------------------------ |
| Description | Format Markdown and YAML files |
| Fix         | yes                            |
| Binary      | `prettier`                     |
| Scope       | [files](#scopes)               |
| Patterns    | `*.md *.yml *.yaml`            |
| Config      | `.prettierrc`                  |

#### `actionlint`

|             |                                                    |
| ----------- | -------------------------------------------------- |
| Description | Lint GitHub Actions workflow files                 |
| Fix         | no                                                 |
| Binary      | `actionlint`                                       |
| Scope       | [file](#scopes)                                    |
| Patterns    | `.github/workflows/*.yml .github/workflows/*.yaml` |
| Config      | `actionlint.yml`                                   |

#### `hadolint`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint Dockerfiles                       |
| Fix         | no                                     |
| Binary      | `hadolint`                             |
| Scope       | [file](#scopes)                        |
| Patterns    | `Dockerfile Dockerfile.* *.dockerfile` |
| Config      | `.hadolint.yaml`                       |

#### `xmllint`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Validate XML files are well-formed |
| Fix         | no                                 |
| Binary      | `xmllint`                          |
| Scope       | [files](#scopes)                   |
| Patterns    | `*.xml`                            |

#### `codespell`

|             |                                    |
| ----------- | ---------------------------------- |
| Description | Check for common spelling mistakes |
| Fix         | yes                                |
| Binary      | `codespell`                        |
| Scope       | [files](#scopes)                   |
| Patterns    | `*`                                |
| Config      | `.codespellrc`                     |

#### `editorconfig-checker`

|             |                                               |
| ----------- | --------------------------------------------- |
| Description | Check files comply with EditorConfig settings |
| Fix         | no                                            |
| Binary      | `ec`                                          |
| Scope       | [files](#scopes)                              |
| Patterns    | `*`                                           |
| Config      | `.editorconfig-checker.json`                  |

#### `golangci-lint`

|             |                                                                     |
| ----------- | ------------------------------------------------------------------- |
| Description | Lint Go code; uses --new-from-rev to scope analysis to changed code |
| Fix         | no                                                                  |
| Binary      | `golangci-lint`                                                     |
| Scope       | [project](#scopes)                                                  |
| Patterns    | `*.go`                                                              |
| Config      | `.golangci.yml`                                                     |

#### `ruff`

|             |                  |
| ----------- | ---------------- |
| Description | Lint Python code |
| Fix         | yes              |
| Binary      | `ruff`           |
| Scope       | [file](#scopes)  |
| Patterns    | `*.py`           |
| Config      | `ruff.toml`      |

#### `ruff-format`

|             |                    |
| ----------- | ------------------ |
| Description | Format Python code |
| Fix         | yes                |
| Binary      | `ruff`             |
| Scope       | [file](#scopes)    |
| Patterns    | `*.py`             |
| Config      | `ruff.toml`        |

#### `biome`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Lint JS/TS/JSON files                  |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

#### `biome-format`

|             |                                        |
| ----------- | -------------------------------------- |
| Description | Format JS/TS/JSON files                |
| Fix         | yes                                    |
| Binary      | `biome`                                |
| Scope       | [file](#scopes)                        |
| Patterns    | `*.json *.jsonc *.js *.ts *.jsx *.tsx` |

#### `cargo-clippy`

|             |                                                         |
| ----------- | ------------------------------------------------------- |
| Description | Lint Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                     |
| Binary      | `cargo-clippy`                                          |
| Scope       | [project](#scopes)                                      |
| Patterns    | `*.rs`                                                  |

#### `cargo-fmt`

|             |                                                           |
| ----------- | --------------------------------------------------------- |
| Description | Format Rust code; runs on all .rs files, not just changed |
| Fix         | yes                                                       |
| Binary      | `rustfmt`                                                 |
| Scope       | [project](#scopes)                                        |
| Patterns    | `*.rs`                                                    |

#### `gofmt`

|             |                 |
| ----------- | --------------- |
| Description | Format Go code  |
| Fix         | yes             |
| Binary      | `gofmt`         |
| Scope       | [file](#scopes) |
| Patterns    | `*.go`          |

#### `google-java-format`

|             |                      |
| ----------- | -------------------- |
| Description | Format Java code     |
| Fix         | yes                  |
| Binary      | `google-java-format` |
| Scope       | [files](#scopes)     |
| Patterns    | `*.java`             |

#### `ktlint`

|             |                             |
| ----------- | --------------------------- |
| Description | Lint and format Kotlin code |
| Fix         | yes                         |
| Binary      | `ktlint`                    |
| Scope       | [files](#scopes)            |
| Patterns    | `*.kt *.kts`                |

#### `dotnet-format`

|             |                  |
| ----------- | ---------------- |
| Description | Format C# code   |
| Fix         | yes              |
| Binary      | `dotnet`         |
| Scope       | [files](#scopes) |
| Patterns    | `*.cs`           |

#### `lychee`

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

#### `renovate-deps`

|             |                                                                                                                            |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| Description | Verify Renovate dependency snapshot is up to date                                                                          |
| Fix         | yes                                                                                                                        |
| Binary      | `renovate`                                                                                                                 |
| Scope       | [special](#scopes)                                                                                                         |
| Patterns    | `renovate.json renovate.json5 .github/renovate.json .github/renovate.json5 .renovaterc .renovaterc.json .renovaterc.json5` |

Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate locally and comparing its output against the committed snapshot. Requires `renovate` in `[tools]`.

With `--fix`, automatically regenerates and commits the snapshot.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

#### `license-header`

|             |                                                     |
| ----------- | --------------------------------------------------- |
| Description | Check source files have the required license header |
| Fix         | no                                                  |
| Binary      | (built-in)                                          |
| Scope       | [special](#scopes)                                  |

<!-- registry-table-end -->
<!-- editorconfig-checker-enable -->

**Note:** Biome's config flag (`--config-path`) takes a directory, not a file path —
config injection for `biome` and `biome-format` is not yet implemented.

#### Scopes

- `file` — invoked once per matched file
- `files` — invoked once with all matched files as args; only changed files are passed
- `project` — invoked once with no file args; for checks with patterns set
  (e.g. `cargo-clippy`), skipped entirely if no matching files changed, but runs on
  the whole project when it does run. `golangci-lint` is the exception — it uses
  `--new-from-rev` to scope analysis to changed code even within the project run.

**Slow checks** (Slow = yes) are skipped by `--fast-only`. Use `--fast-only` for
local/pre-push feedback and the full set in CI.

**`editorconfig-checker` deference**: `editorconfig-checker` runs on all files, but
automatically skips file types owned by an active line-length-enforcing
formatter. When `cargo-fmt`, `ruff-format`, `biome-format`, or `prettier`
are active, their file types are excluded from `editorconfig-checker` — those formatters
already enforce line length and would conflict with `editorconfig-checker`'s
`max_line_length` editorconfig check. If none of those formatters are
installed, `editorconfig-checker` checks those files itself.

## Why

The bash task scripts (v1) have two problems:

**Local ≠ CI**: `--native` runs a subset of linters; CI runs full super-linter
in Docker. Different tools, different behavior. Passing locally does not mean
passing in CI.

**Bash has limits**: the registry pattern was already at the edge of what bash
does cleanly. Adding built-in checks (links, renovate) would make it worse.

### Why not pre-commit?

pre-commit adds a parallel tool management system on top of mise. Consuming repos
already declare their tools in `mise.toml` — pre-commit would require maintaining
a second inventory of the same tools in `.pre-commit-config.yaml`, with its own
versioning and install lifecycle. That's friction without benefit for repos that
are already mise-first.

### Why not Spotless (or other Maven formatter plugins)?

Spotless runs `google-java-format` as a Maven build phase, which means format
failures block compilation and test runs — that's the wrong place for a style
check. flint's `google-java-format` check runs as a separate lint step, only on
changed files, and is fast.

To migrate: remove `spotless-maven-plugin` from your `pom.xml` (and any
`spotless.skip` properties), add `"github:google/google-java-format"` to
`[tools]` in `mise.toml`, and run `flint run --fix` once to confirm the repo is
clean.

### Why not MegaLinter / super-linter?

Container-based linters (super-linter, MegaLinter) ship their own tool versions,
independent of what the repo pins in `mise.toml`. This breaks the "declare once,
use everywhere" promise of mise. Container startup also adds latency to every run.

## Principles

1. **Fast** — the primary goal; everything else serves it:
   - Native execution only (no Docker); linters run in parallel (Rust binary, short startup)
   - Small binary, cached by mise — fast install, near-zero overhead between runs
   - Diff-aware: only changed files are linted by default; `--full` to check everything
   - Opt-in via `mise.toml`: undeclared tools are skipped entirely
   - Slow checks (e.g. `renovate-deps`) tagged and skippable with `--fast-only`

2. **Local same as CI** — one binary, one config, identical behavior.
   No "native mode subset" distinction. If it passes locally, it passes in CI.

3. **AI-friendly** — `--fix` fixes what's fixable silently, prints output
   only for issues needing review, and exits with a structured summary:

   ```text
   [shellcheck]
   ...
   flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
   ```

   Only unfixable issues surface for review — no reasoning step required.

4. **Cross-platform** — runs on Linux, macOS, and Windows. The built-in
   registry accounts for platform differences (e.g. binary names, path quoting).

5. **Autofix where possible** — `--fix` checks first, fixes what's fixable,
   reports what needs review. Fix mode runs serially to avoid concurrent writes.
   Pass specific linter names to limit which fixers run (`flint run --fix prettier shfmt`).

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
