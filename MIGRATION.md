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

### 3. Run `flint init`

After installing flint (`mise install`), run `flint init`. It detects your
languages from tracked files and takes care of:

- adding linters to `[tools]`
- adding `[env] FLINT_CONFIG_DIR` pointing to your chosen config dir
- adding `lint`, `lint:fix`, `lint:pre-commit`, and `setup:pre-commit-hook`
  tasks to `[tasks]`
- writing a `flint.toml` skeleton in your config dir
- generating `.github/workflows/lint.yml`

Then run `mise install` to install the new tools and
`mise run setup:pre-commit-hook` to install the git hook.

### 4. Switch `markdownlint-cli` to `markdownlint-cli2`

flint v2 only supports `markdownlint-cli2`. `flint init` selects it for new
installs, but for an existing repo you need to rename the key manually:

```toml
# Before:
"npm:markdownlint-cli" = "0.48.0"
# After:
"npm:markdownlint-cli2" = "0.17.2"
```

Configuration files remain compatible — both tools read `.markdownlint.json`
(and `.markdownlint.yaml`, `.markdownlint.jsonc`). No changes to your config
file are required.

### 5. Move renovate-deps config to `flint.toml`

If you previously used the `RENOVATE_TRACKED_DEPS_EXCLUDE` env var to exclude
managers, remove it from `[env]` in `mise.toml` and uncomment the
`exclude_managers` line that `flint init` wrote to your `flint.toml`:

```toml
[checks.renovate-deps]
exclude_managers = ["github-actions", "github-runners", "cargo"]
```

### 6. Add the flint renovate preset to `renovate.json5`

Add `"github>grafana/flint#v<version>"` to the `extends` list in your
`renovate.json5`. This lets renovate keep the flint binary version up to date
automatically:

```json5
{
  extends: [
    "config:recommended",
    "github>grafana/flint#v0.20.0",
    // ...
  ],
}
```

Replace `v0.20.0` with the version you pinned in `[tools]`.

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
