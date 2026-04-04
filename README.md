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

mise-native linter runner. Parallel, cross-platform, AI-friendly, local == CI.
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
"npm:markdownlint-cli" = "0.47.0"
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
run = "flint"

[tasks."lint:pre-commit"]
description = "Fast auto-fix lint pass — for pre-push hooks and agentic pipelines"
run = "flint --fix --fast"

[tasks."lint:fix"]
description = "Auto-fix lint issues"
run = "flint --fix"
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
flint [OPTIONS] [LINTERS...]
flint list
```

| Flag             | Description                                                              |
| ---------------- | ------------------------------------------------------------------------ |
| `--fix`          | Fix what's fixable, report what still needs review; exit 1 if anything changed or needs review |
| `--full`         | Lint all files instead of only changed files                             |
| `--fast`         | Skip slow checks (e.g. `renovate-deps`)                                  |
| `--short`        | Compact summary output, no per-check noise                               |
| `--verbose`      | Show all linter output, not just failures                                |
| `--from-ref REF` | Diff base (default: merge base with base branch)                         |
| `--to-ref REF`   | Diff head (default: HEAD)                                                |

Every flag has an env var equivalent: `FLINT_FIX`, `FLINT_FULL`, `FLINT_FAST`,
`FLINT_VERBOSE`, `FLINT_SHORT`, `FLINT_FROM_REF`, `FLINT_TO_REF`.

#### Intended use by context

| Context                      | Command                   | Why                                                               |
| ---------------------------- | ------------------------- | ----------------------------------------------------------------- |
| Interactive development      | `flint` or `flint --fast` | Full output so you can read the details                           |
| Human wanting a summary      | `flint --short`           | Compact output, no per-check noise                                |
| Pre-push hook (CC / agentic) | `flint --fix --fast`      | Fixes what it can silently, surfaces only what needs human review |
| CI                           | `flint`                   | Full output for humans reading CI logs                            |

**`--short` output** — failed checks partitioned by fixability, fixable ones
expressed as the exact command to run:

```text
flint: 2 checks failed — flint --fix prettier cargo-fmt | review: shellcheck
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
flint shellcheck shfmt        # run only shellcheck and shfmt
flint --fix prettier          # fix only prettier
```

`flint list` shows every check with its status:

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
exclude = "CHANGELOG\\.md|vendor/.*"          # regex — exclude matching files

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

**Note:** `ec`'s config file (`.editorconfig-checker.json`) controls ec's own settings,
not `.editorconfig` itself — editorconfig discovery always walks up from the file
being linted and cannot be redirected via a flag.

### Built-in linter registry

<!-- editorconfig-checker-disable -->

| Name            | Binary          | Patterns                                           | Fix | Scope   | Config file                    |
| --------------- | --------------- | -------------------------------------------------- | --- | ------- | ------------------------------ |
| `shellcheck`    | `shellcheck`    | `*.sh *.bash *.bats`                               | no  | file    | `.shellcheckrc`                |
| `shfmt`         | `shfmt`         | `*.sh *.bash`                                      | yes | file    | —                              |
| `markdownlint`  | `markdownlint`  | `*.md`                                             | yes | file    | `.markdownlint.json`           |
| `prettier`      | `prettier`      | `*.md *.yml *.yaml`                                | yes | files   | `.prettierrc`                  |
| `actionlint`    | `actionlint`    | `.github/workflows/*.yml .github/workflows/*.yaml` | no  | file    | `actionlint.yml`               |
| `hadolint`      | `hadolint`      | `Dockerfile Dockerfile.* *.dockerfile`             | no  | file    | `.hadolint.yaml`               |
| `codespell`     | `codespell`     | `*`                                                | yes | files   | `.codespellrc`                 |
| `ec`            | `ec`            | `*`                                                | no  | files   | `.editorconfig-checker.json`   |
| `golangci-lint` | `golangci-lint` | `*.go`                                             | no  | project | `.golangci.yml`                |
| `ruff`          | `ruff`          | `*.py`                                             | yes | file    | `ruff.toml`                    |
| `ruff-format`   | `ruff`          | `*.py`                                             | yes | file    | `ruff.toml`                    |
| `biome`         | `biome`         | `*.json *.jsonc *.js *.ts *.jsx *.tsx`             | yes | file    | `biome.json` ¹                 |
| `biome-format`  | `biome`         | `*.json *.jsonc *.js *.ts *.jsx *.tsx`             | yes | file    | `biome.json` ¹                 |
| `cargo-clippy`  | `cargo-clippy`  | `*.rs`                                             | yes | project | —                              |
| `cargo-fmt`     | `cargo-fmt`     | `*.rs`                                             | yes | project | —                              |
| `links`         | `lychee`        | (all files)                                        | no  | special | via `[checks.links]` in flint.toml |
| `renovate-deps` | `renovate`      | (all files)                                        | yes | special | —                              |

¹ Not yet implemented. Biome's flag (`--config-path`) takes a directory, not a
file path — requires a directory-injection variant of the config mechanism.

<!-- editorconfig-checker-enable -->

**Scopes:**

- `file` — invoked once per matched file
- `files` — invoked once with all matched files as args
- `project` — invoked once with no file args; for checks with patterns set
  (e.g. `cargo-clippy`), skipped entirely if no matching files changed

**Slow checks** (`renovate-deps`) are skipped by `--fast`. Use `--fast` for
local/pre-push feedback and the full set in CI.

**`ec` deference**: `ec` (editorconfig-checker) runs on all files, but
automatically skips file types owned by an active line-length-enforcing
formatter. When `cargo-fmt`, `ruff-format`, `biome-format`, or `prettier`
are active, their file types are excluded from `ec` — those formatters
already enforce line length and would conflict with `ec`'s
`max_line_length` editorconfig check. If none of those formatters are
installed, `ec` checks those files itself.

### Special checks

#### links

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires
`lychee` in `[tools]`.

Default behavior: checks all links in changed files. When `check_all_local = true`
in `flint.toml`, adds a second pass over local links in all files — useful when
broken internal links from unchanged files also matter.

Configure via `flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```

#### renovate-deps

Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate
locally and comparing its output against the committed snapshot. Same purpose as
the v1 `lint:renovate-deps` task. Requires `renovate` in `[tools]`.

Tagged `slow = true` — skipped by `--fast`. With `--fix`, automatically regenerates
and commits the snapshot.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

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

### Why not MegaLinter / super-linter?

Container-based linters (super-linter, MegaLinter) ship their own tool versions,
independent of what the repo pins in `mise.toml`. This breaks the "declare once,
use everywhere" promise of mise. Container startup also adds latency to every run.

## Principles

1. **mise-based** — `flint` distributed via mise. Tools managed by the consuming
   repo's `mise.toml`. No separate tool installation step.

2. **Fast** — native execution only (no Docker). Linters run in parallel.
   Designed to be the default `mise run lint`, not a slow fallback.
   Slow checks (e.g. `renovate-deps`) can be skipped with `--fast`.

3. **Cross-platform** — runs on Linux, macOS, and Windows. The built-in
   registry accounts for platform differences (e.g. binary names, path quoting).

4. **Local same as CI** — one binary, one config, identical behavior.
   No "native mode subset" distinction. If it passes locally, it passes in CI.

5. **AI-friendly** — `--fix` fixes what's fixable silently, prints output
   only for issues needing review, and exits with a structured summary:
   ```
   [shellcheck]
   ...
   flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
   ```
   Only unfixable issues surface for review — no reasoning step required.
   Also runnable containerised — no host tool dependencies required.

6. **Opt-in via tool install** — checks auto-enable when their tool is declared
   in `mise.toml`. `flint.toml` adds detail (config paths, exclusions) but is
   not required to activate anything.

7. **Changed files by default** — git-aware diff detection. `--from-ref`/`--to-ref`
   for CI. `--full` to check everything. Falls back to all files when no merge
   base is found.

8. **Autofix where possible** — `--fix` checks first, fixes what's fixable,
   reports what needs review. Fix mode runs serially to avoid concurrent writes.
   Pass specific linter names to limit which fixers run (`flint --fix prettier shfmt`).

## Versioning

This project uses [Semantic Versioning](https://semver.org/).
Breaking changes will be documented in [CHANGELOG.md](CHANGELOG.md)
and will result in a major version bump.

## Releasing

See [RELEASING.md](RELEASING.md).
