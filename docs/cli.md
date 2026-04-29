# CLI reference

```text
flint run [OPTIONS] [LINTERS...]
flint init
flint hook install
flint linters
flint version
```

Commands and flags follow
[golangci-lint](https://golangci-lint.run/) conventions. Teams already using
it do not need to re-learn the interface.

## `flint run` flags

| Flag                 | Description                                                                                                          |
| -------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `--fix`              | Fix what's fixable, report `clean` / `fixed` / `partial` / `review` outcomes; exit non-zero if anything needs action |
| `--full`             | Lint all files instead of only changed files                                                                         |
| `--fast-only`        | Skip checks tagged as slow in the registry. Overridden by explicit linter names.                                     |
| `--short`            | Compact summary output, no per-check noise                                                                           |
| `--verbose`          | Show all linter output, not just failures                                                                            |
| `--new-from-rev REV` | Diff base (default: merge base with base branch)                                                                     |
| `--to-ref REF`       | Diff head (default: HEAD)                                                                                            |

Every flag has an env var equivalent: `FLINT_FIX`, `FLINT_FULL`, `FLINT_FAST_ONLY`,
`FLINT_VERBOSE`, `FLINT_SHORT`, `FLINT_NEW_FROM_REV`, `FLINT_TO_REF`.

## Intended use by context

| Context                      | Command                                | Why                                                               |
| ---------------------------- | -------------------------------------- | ----------------------------------------------------------------- |
| Interactive development      | `flint run` or `flint run --fast-only` | Full output so you can read the details                           |
| Human wanting a summary      | `flint run --short`                    | Compact output, no per-check noise                                |
| Pre-push hook (CC / agentic) | `flint run --fix --fast-only`          | Fixes what it can silently, surfaces only what needs human review |
| CI                           | `flint run`                            | Full output for humans reading CI logs                            |

## Changed-file and baseline runs

By default, `flint run` checks only files changed relative to the merge base.
Use `--full` to check every matching file explicitly.

Some changed-file runs intentionally expand one or more affected checks to all
matching files. This establishes a baseline when lint coverage changes, while
leaving unrelated checks scoped to changed files.

A check runs against all matching files when:

- the check is newly active because its tool was added to `mise.toml`
- the check's tool version changed in `mise.toml`
- the pinned Flint tool changed in `mise.toml`, either released
  `github:grafana/flint` or a cargo-backed prerelease revision, which expands
  all active checks
- the check's flint-managed config file changed, such as `.shellcheckrc` or
  `.yamllint.yml` in `FLINT_CONFIG_DIR`
- another supported baseline config for the check changed, such as
  `.editorconfig` for `editorconfig-checker`
- `flint.toml` changed under `[settings]`
- `flint.toml` changed the check-specific config for a native check, such as
  `[checks.links]` or `[checks.renovate-deps]`

`--full` is still the explicit whole-repo mode. The automatic baseline behavior
only applies in changed-file mode, and only to checks whose lint coverage may
have changed. Config-file triggers are detected from the raw git change list, so
they still apply when the config path itself is excluded from ordinary lint file
selection.

Flint intentionally supports one canonical config filename per linter when it
passes config paths explicitly. If an active linter has a known alternate
upstream config file, Flint fails before running the linter instead of silently
ignoring or partially auto-discovering that config. Move the config to the
Flint-managed filename under `FLINT_CONFIG_DIR`, or remove the alternate file.
Biome is the exception: its canonical config is root `biome.jsonc`.

**`--short` output** — failed checks partitioned by fixability, fixable ones
expressed as the exact command to run:

```text
flint: 2 checks failed — flint run --fix rumdl cargo-fmt | review: shellcheck
```

**`--fix` output** — fixes what's fixable, then prints the full output of
any checks that still need review, followed by a summary line. The internal
outcome model distinguishes `clean`, `fixed`, `review`, and `partial`:

- `clean` — the fixer ran and found nothing to change
- `fixed` — the fixer resolved the issue; commit before pushing
- `review` — no fixer was applied; human review is required
- `partial` — a fixer ran but the check still failed and needs review

Exit status remains intentionally coarse: `0` only when everything was already
clean, non-zero when anything still needs action. Callers should rely only on
`0` vs non-`0`, not on specific non-zero codes:

```text
[shellcheck]

In bad.sh line 2:
echo $1
     ^-- SC2086 (info): Double quote to prevent globbing and word splitting.
...
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
```

More `--fix` summary examples:

```text
flint: fixed: gofmt — commit before pushing
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
flint: fixed: gofmt — commit before pushing | partial: cargo-clippy
flint: partial: cargo-clippy | review: shellcheck
flint: partial: cargo-clippy
flint: review: shellcheck
```

Pass one or more linter names to run only those:

```bash
flint run shellcheck shfmt        # run only shellcheck and shfmt
flint run --fix rumdl             # fix only Markdown issues
```

## `flint init`

`flint init` pins Flint itself in `mise.toml` so every contributor uses the same
lint binary. For unreleased consumer validation, pass an explicit git revision:

```bash
flint init -y --flint-rev <git-rev>
```

That writes a cargo-backed Flint pin. To return to the released Flint backend
after the release is cut, run `flint init` again without `--flint-rev`.

`flint init` is also the explicit way to reconcile a repo with the latest Flint
setup defaults. Routine lint runs use `flint-setup` and only fail when an
actionable setup migration applies to the repo.

To check setup drift without applying changes, run:

```bash
flint run flint-setup
```

To apply setup migrations and canonicalize `mise.toml`, run:

```bash
flint run --fix flint-setup
```

## `flint linters`

`flint linters` shows every check with its status:

```text
NAME            BINARY          STATUS     SPEED  PATTERNS
-------------------------------------------------------------------
shellcheck      shellcheck      installed  fast   *.sh *.bash *.bats
cargo-fmt       cargo-fmt       missing    fast   *.rs
renovate-deps   renovate        installed  fast
...
```
