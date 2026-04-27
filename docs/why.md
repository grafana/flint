# Why Flint

flint exists to make repository linting fast, predictable, and easy to keep
consistent between local development, hooks, CI, and agentic workflows.

It uses the tools the repo has chosen to install, runs only the checks that
are actually opted in, and keeps behavior aligned across every place the repo
is linted.

For comparisons with other lint runners and hook managers, see
[Alternatives / Comparisons](alternatives.md).

## Fast

This is the primary goal; everything else serves it.

- Native execution only: no Docker startup overhead
- Parallel runs in check mode
- Small binary, cached by mise
- Diff-aware by default: changed files only unless `--full` is requested
- Opt-in activation: undeclared tools are skipped entirely
- Slow checks can be skipped via `--fast-only`

## Local same as CI

One binary, one config model, identical behavior. There is no "native mode
subset" distinction. If it passes locally, it passes in CI.

## Predictable and updatable linter versions

Flint runs pinned linter versions chosen by the repo, so lint behavior does not
suddenly change just because an upstream release landed. When a repo wants a
new `lychee`, `ruff`, or `shellcheck`, it updates that version explicitly and
reviews the result as a normal change. In practice that also works well with
Renovate, because the pinned versions live in `mise.toml`.

## Easy setup, sane defaults

`flint init` bootstraps a repo quickly, the active checks come from
`mise.toml`, and most repos do not need much custom configuration beyond
choosing tools.

## Opinionated where it matters

Flint prefers one canonical config shape per linter to avoid discovery drift,
while still letting repos choose a config directory with `FLINT_CONFIG_DIR`
when the tool supports explicit config injection.

## Separated ownership

Linters and formatters are distinct checks, and overlapping file types have a
clear style owner. `editorconfig-checker` defers where formatter ownership
should win, which avoids contradictory output.

Examples:

- Markdown style is owned by `rumdl`, not split between multiple Markdown tools
- JS/TS/JSON formatting is owned by Biome, with root `biome.jsonc` as the
  canonical config
- `editorconfig-checker` defers to active formatters for file types where the
  formatter should be authoritative

## AI-friendly

`--fix` fixes what's fixable silently, prints output only for issues needing
review, and exits with a structured summary:

```text
[shellcheck]
...
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
```

Only unfixable issues surface for review; no reasoning step is required.

## Cross-platform

Flint runs on Linux, macOS, and Windows. The built-in registry accounts for
platform differences such as binary names and path quoting.

## Autofix where possible

`--fix` checks first, fixes what's fixable, and reports what needs review. Fix
mode runs serially to avoid concurrent writes. Pass specific linter names to
limit which fixers run, for example `flint run --fix rumdl shfmt`.
