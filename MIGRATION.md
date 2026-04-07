# Migration Guide

## Migrating from flint v1 (bash tasks) to flint v2 (binary)

flint v2 replaces the HTTP remote tasks with a single `flint` binary that
discovers linters from your `mise.toml` and runs them against changed files.

### 1. Add `flint` as a tool

```toml
[tools]
"ubi:grafana/flint" = "0.20.0-alpha.1"
```

### 2. Run `flint init`

After installing flint (`mise install`), run `flint init`. It automatically:

- removes v1 HTTP task entries from `[tasks]`
- removes `RENOVATE_TRACKED_DEPS_EXCLUDE` from `[env]` and migrates the manager list to `flint.toml` (when a v1 renovate-deps task is present)
- replaces `npm:markdownlint-cli` with `npm:markdownlint-cli2` in `[tools]`
- adds the missing linters to `[tools]` based on your tracked files
- adds `[env] FLINT_CONFIG_DIR` and standard `lint*` / `setup:pre-commit-hook` tasks
- writes a `flint.toml` skeleton in your chosen config dir
- generates `.github/workflows/lint.yml`
- patches `renovate.json5` to add the flint preset

Then run `mise install` to install the new tools and
`mise run setup:pre-commit-hook` to install the git hook.

### 3. Verify active linters

Run `flint linters` to confirm flint detects all the tools declared in your
`mise.toml`. Any tool listed as `missing` is not declared and will be skipped.
