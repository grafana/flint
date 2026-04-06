# Migration Guide

## Migrating from flint v1 (bash tasks) to flint v2 (binary)

flint v2 replaces the HTTP remote tasks with a single `flint` binary that
discovers linters from your `mise.toml` and runs them against changed files.

### 1. Remove the v1 task entries from `mise.toml`

Remove all task entries that reference remote flint task scripts:

```toml
# Remove these:
[tasks."lint:links"]
file = "https://raw.githubusercontent.com/grafana/flint/..."
[tasks."lint:renovate-deps"]
file = "https://raw.githubusercontent.com/grafana/flint/..."
```

Also remove any hand-rolled style lint scripts that delegate to individual
linters (shfmt, prettier, markdownlint, actionlint, codespell,
editorconfig-checker) — flint v2 handles all of these automatically based on
what is declared in `[tools]`.

### 2. Add `flint` as a tool

```toml
[tools]
"ubi:grafana/flint" = "0.20.0-alpha.1"
```

### 3. Replace linting tasks with `flint run`

```toml
[tasks.lint]
run = "flint run"

[tasks."lint:fix"]
run = "flint run --fix"
```

For CI, pass `--short` for compact output suited to AI-assisted review:

```toml
[tasks.ci]
run = "flint run --short"
```

### 4. Add a pre-commit task

flint v2 provides a fast auto-fix pass intended for git hooks:

```toml
[tasks."lint:pre-commit"]
description = "Fast auto-fix lint (skips slow checks) — for pre-commit/pre-push hooks"
run = "flint run --fix --fast-only"

[tasks."setup:pre-commit-hook"]
description = "Install git pre-commit hook"
run = "mise generate git-pre-commit --write --task=lint:pre-commit"
```

Then run `mise run setup:pre-commit-hook` once to install the hook.

### 5. Switch `markdownlint-cli` to `markdownlint-cli2`

flint v2 only supports `markdownlint-cli2`. See the
[section below](#replacing-markdownlint-cli-with-markdownlint-cli2) for
details — config files are compatible, no changes required there.

```toml
# Before:
"npm:markdownlint-cli" = "0.48.0"
# After:
"npm:markdownlint-cli2" = "0.17.2"
```

### 6. Move renovate-deps config to `flint.toml`

If you previously used the `RENOVATE_TRACKED_DEPS_EXCLUDE` env var to exclude
managers, move that to a `flint.toml` at your project root instead:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners", "cargo"]
```

Remove `RENOVATE_TRACKED_DEPS_EXCLUDE` from `[env]` in `mise.toml`.

### 7. Verify active linters

Run `flint linters` to confirm flint detects all the tools declared in your
`mise.toml`. Any tool listed as `missing` is not declared and will be skipped.

## Replacing `markdownlint-cli` with `markdownlint-cli2`

`markdownlint-cli2` is the actively maintained successor to `markdownlint-cli`.
It is faster, supports more configuration options, and is the direction the
markdownlint ecosystem is moving. flint only supports `markdownlint-cli2`.

**Before** (`mise.toml`):

```toml
"npm:markdownlint-cli" = "0.47.0"
```

**After**:

```toml
"npm:markdownlint-cli2" = "0.17.2"
```

Configuration files remain compatible — both tools read `.markdownlint.json`
(and `.markdownlint.yaml`, `.markdownlint.jsonc`). No changes to your config
file are required.

The fix command changes from `markdownlint --fix` to `markdownlint-cli2 --fix`,
but flint handles this automatically.
