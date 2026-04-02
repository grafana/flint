# flint v2

A single Rust binary that replaces the bash task scripts.
Discovers linting tools from PATH, runs them against changed files in parallel,
and produces identical output locally and in CI.

> **Status**: in development on the `feat/flint-v2` branch.
> The bash task scripts (v1) remain the stable option until v2 is released.

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

3. **Local same as CI** — one binary, one config, identical behavior.
   No "native mode subset" distinction. If it passes locally, it passes in CI.

4. **AI-friendly** — `--short` suppresses per-check output and emits a single
   structured summary line (`flint --fix prettier | review: shellcheck`) for
   token-efficient AI consumption. Fixable checks are expressed as the exact
   command to run — no reasoning step required. Also runnable containerised —
   no host tool dependencies required.

5. **Opt-in via tool install** — checks auto-enable when their binary is in PATH.
   Installing a tool in `mise.toml` is the opt-in. `flint.toml` adds detail
   (config paths, exclusions) but is not required to activate anything.

6. **Changed files by default** — git-aware diff detection. `--from-ref`/`--to-ref`
   for CI. `--full` to check everything. Falls back to all files when no merge
   base is found.

7. **Autofix where possible** — `--fix` flag (or `AUTOFIX=true`). Fix mode runs
   serially to avoid concurrent writes to the same file. Pass specific linter
   names to limit which fixers run (`flint --fix prettier shfmt`).

## Installation

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

## Usage

```text
flint [OPTIONS] [LINTERS...]
flint list
```

**Options:**

| Flag             | Description                                        |
| ---------------- | -------------------------------------------------- |
| `--fix`          | Auto-fix issues instead of checking                |
| `--auto`         | Fix what's fixable, report what still needs review |
| `--full`         | Lint all files instead of only changed files       |
| `--fast`         | Skip slow checks (e.g. `renovate-deps`)            |
| `--short`        | Compact summary output, no per-check noise         |
| `--verbose`      | Show all linter output, not just failures          |
| `--from-ref REF` | Diff base (default: merge base with base branch)   |
| `--to-ref REF`   | Diff head (default: HEAD)                          |

Env var equivalents: `AUTOFIX=true` for `--fix`, `FLINT_SHORT=true` for `--short`.

### Intended use by context

| Context                      | Command                   | Why                                                               |
| ---------------------------- | ------------------------- | ----------------------------------------------------------------- |
| Interactive development      | `flint` or `flint --fast` | Full output so you can read the details                           |
| Human wanting a summary      | `flint --short`           | Compact output, no per-check noise                                |
| Pre-push hook (CC / agentic) | `flint --auto --fast`     | Fixes what it can silently, surfaces only what needs human review |
| CI                           | `flint`                   | Full output for humans reading CI logs                            |

**`--short` output** — failed checks partitioned by fixability, fixable ones
expressed as the exact command to run:

```text
flint: 2 checks failed — flint --fix prettier cargo-fmt | review: shellcheck
```

**`--auto` output** — fixes what's fixable, reports the outcome. Exits 1 if
anything was fixed (so the caller commits the fixes before pushing) or if
anything still needs review. Exits 0 only if everything was already clean:

```text
flint: fixed: prettier cargo-fmt — commit before pushing | review: shellcheck
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

## Config (`flint.toml`)

Optional. Place in the repo root. All settings have defaults.

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

## mise.toml wiring

```toml
[tools]
flint = "0.x.y"

[tasks.lint]
description = "Run all lints"
run = "flint"

[tasks."native-lint"]
description = "Run fast lints (skip slow checks)"
run = "flint --fast"

[tasks.fix]
description = "Auto-fix lint issues"
run = "flint --fix"
```

## Built-in linter registry

Checks auto-enable when their binary is found in PATH. Install tools via `mise.toml`.

<!-- editorconfig-checker-disable -->

| Name            | Binary          | Patterns                                           | Fix | Scope   |
| --------------- | --------------- | -------------------------------------------------- | --- | ------- |
| `shellcheck`    | `shellcheck`    | `*.sh *.bash *.bats`                               | no  | file    |
| `shfmt`         | `shfmt`         | `*.sh *.bash`                                      | yes | file    |
| `markdownlint`  | `markdownlint`  | `*.md`                                             | yes | file    |
| `prettier`      | `prettier`      | `*.md *.yml *.yaml`                                | yes | files   |
| `actionlint`    | `actionlint`    | `.github/workflows/*.yml .github/workflows/*.yaml` | no  | file    |
| `hadolint`      | `hadolint`      | `Dockerfile Dockerfile.* *.dockerfile`             | no  | file    |
| `codespell`     | `codespell`     | `*`                                                | yes | files   |
| `ec`            | `ec`            | `*`                                                | no  | files   |
| `golangci-lint` | `golangci-lint` | `*.go`                                             | no  | project |
| `ruff`          | `ruff`          | `*.py`                                             | yes | file    |
| `ruff-format`   | `ruff`          | `*.py`                                             | yes | file    |
| `biome`         | `biome`         | `*.json *.jsonc *.js *.ts *.jsx *.tsx`             | yes | file    |
| `biome-format`  | `biome`         | `*.json *.jsonc *.js *.ts *.jsx *.tsx`             | yes | file    |
| `cargo-clippy`  | `cargo-clippy`  | `*.rs`                                             | yes | project |
| `cargo-fmt`     | `cargo-fmt`     | `*.rs`                                             | yes | project |
| `links`         | `lychee`        | (all files)                                        | no  | special |
| `renovate-deps` | `renovate`      | (all files)                                        | yes | special |

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

## Special checks

### links

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires
`lychee` in PATH (install via `mise.toml`).

Default behavior: checks all links in changed files. When `check_all_local = true`
in `flint.toml`, adds a second pass over local links in all files — useful when
broken internal links from unchanged files also matter.

Configure via `flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```

### renovate-deps

Verifies `.github/renovate-tracked-deps.json` is up to date by running Renovate
locally and comparing its output against the committed snapshot. Same purpose as
the v1 `lint:renovate-deps` task. Requires `renovate` in PATH (install via `mise.toml`).

Tagged `slow = true` — skipped by `--fast`. With `--fix`, automatically regenerates
and commits the snapshot.

Configure via `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners"]
```

## CI example

```yaml
- name: Install tools
  run: mise install

- name: Lint
  run: mise run lint # or: flint --from-ref origin/main --to-ref HEAD
```

`--from-ref`/`--to-ref` is optional in CI — flint detects the merge base
automatically when running in a PR context.
