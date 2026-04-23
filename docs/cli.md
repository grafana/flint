# CLI reference

```text
flint run [OPTIONS] [LINTERS...]
flint init
flint hook install
flint update
flint linters
flint version
```

Commands and flags follow [golangci-lint](https://golangci-lint.run/) conventions — teams already using it don't need to re-learn the interface.

## `flint run` flags

| Flag                 | Description                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------------- |
| `--fix`              | Fix what's fixable, report what still needs review; exit 1 if anything changed or needs review |
| `--full`             | Lint all files instead of only changed files                                                   |
| `--fast-only`        | Skip checks tagged as slow in the registry. Overridden by explicit linter names.               |
| `--short`            | Compact summary output, no per-check noise                                                     |
| `--verbose`          | Show all linter output, not just failures                                                      |
| `--new-from-rev REV` | Diff base (default: merge base with base branch)                                               |
| `--to-ref REF`       | Diff head (default: HEAD)                                                                      |

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
- the check's flint-managed config file changed, such as `.shellcheckrc` or
  `.yamllint.yml` in `FLINT_CONFIG_DIR`
- `flint.toml` changed under `[settings]`
- `flint.toml` changed the check-specific config for a special check, such as
  `[checks.links]` or `[checks.renovate-deps]`

`--full` is still the explicit whole-repo mode. The automatic baseline behavior
only applies in changed-file mode, and only to checks whose lint coverage may
have changed. Config-file triggers are detected from the raw git change list, so
they still apply when the config path itself is excluded from ordinary lint file
selection.

**`--short` output** — failed checks partitioned by fixability, fixable ones
expressed as the exact command to run:

```text
flint: 2 checks failed — flint run --fix rumdl cargo-fmt | review: shellcheck
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
flint run --fix rumdl             # fix only Markdown issues
```

## `flint update`

`flint update` applies non-interactive migrations to `mise.toml` — replaces obsolete
tool keys with their modern equivalents, preserving the declared version. Run it when
`flint run` reports an obsolete key error:

```text
flint: obsolete tool key in mise.toml: "github:mvdan/sh" (replaced by "shfmt")
  Run `flint update` to apply the migration automatically.
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
