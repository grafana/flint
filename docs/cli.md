# CLI reference

```text
flint run [OPTIONS] [LINTERS...]
flint init [OPTIONS]
flint hook install
flint linters
flint version
```

Commands and flags follow
[golangci-lint](https://golangci-lint.run/) conventions. Teams already using
it do not need to re-learn the interface.

## Output

Flint is built to be quiet so AI agents (and humans) don't have to read pages
of linter output to find the actionable bit:

- **Clean run** — no output. Same under `--fix` when nothing needed fixing.
- **`--fix`** — silently fixes what it can, prints review-required output, and
  ends with a one-line summary of the non-clean checks and their state
  (`fixed`, `review`, `partial`). Fully-clean `--fix` runs print nothing.

Example `--fix` output:

```text
[shellcheck]

In bad.sh line 2:
echo $1
     ^-- SC2086 (info): Double quote to prevent globbing and word splitting.
...
flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
```

## `flint run` flags

<!-- run-flags-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

| Flag                 | Env var              | Description                                                                                                                                                                |
| -------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--fix`              | `FLINT_FIX`          | Fix what's fixable, report what still needs review. Exits 1 if anything was fixed (uncommitted) or needs review; 0 if already clean. Only 0 vs non-0 is stable for callers |
| `--allow-fixed`      | `FLINT_ALLOW_FIXED`  | In --fix mode, exit 0 when all reported issues were fixed successfully. Still exits non-zero when any check is partial or needs review                                     |
| `--full`             | `FLINT_FULL`         | Lint all files instead of only changed files                                                                                                                               |
| `--verbose`          | `FLINT_VERBOSE`      | Show all linter output, not just failures                                                                                                                                  |
| `--short`            | `FLINT_SHORT`        | Compact summary output — no per-check noise (human) or read-only AI review                                                                                                 |
| `--new-from-rev` REV | `FLINT_NEW_FROM_REV` | Show only new issues created after git revision REV (default: merge base with base branch)                                                                                 |
| `--to-ref` REF       | `FLINT_TO_REF`       | Compare changed files to this ref (default: HEAD)                                                                                                                          |
| `--time`             | `FLINT_TIME`         | Show how long each linter took to run                                                                                                                                      |

<!-- run-flags-end -->

All `flint run` flags above have env var equivalents.

## Intended use by context

| Context                      | Command                                | Why                                                               |
| ---------------------------- | -------------------------------------- | ----------------------------------------------------------------- |
| Interactive development      | `flint run`                            | Full output so you can read the details                           |
| Human wanting a summary      | `flint run --short`                    | Compact output, no per-check noise                                |
| Pre-push hook (CC / agentic) | `flint run --fix`                      | Fixes what it can silently, surfaces only what needs human review |
| CI                           | `flint run`                            | Full output for humans reading CI logs                            |

## Changed-file and baseline runs

By default, local `flint run` checks tracked files triggered by changes
relative to the merge base. In CI, `flint run` activates the full linter set
while still keeping diff-aware scoping where each linter supports it. Use
`--full` to check every matching tracked file explicitly.

Flint skips files marked `linguist-generated` in `.gitattributes`. Prefer that
over Flint-only `settings.exclude` entries when the file is generated, because
GitHub and other tools can reuse the same metadata.

Some changed-file runs intentionally expand one or more affected checks to all
matching files. This establishes a baseline when lint coverage changes, while
leaving unrelated checks scoped to changed files.

A check runs against all matching files when:

- the check is newly active because its tool was added to `mise.toml`
- the check's tool version changed in `mise.toml`
- the pinned Flint tool changed in `mise.toml`, either released
  `aqua:grafana/flint` or a cargo-backed prerelease revision, which expands
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

## Adaptive runs

Some linters are expensive enough that running them on every local
`flint run` would slow the inner loop. For those, `flint run` skips the
linter when none of the changed files could plausibly affect its result.
CI is unaffected — it always runs the full set.

Affected linters:

| Linter                                                              | Skipped locally when…                                           |
| ------------------------------------------------------------------- | --------------------------------------------------------------- |
| [`renovate-deps`](linters/renovate-deps.md#when-does-this-run)      | No change to Renovate config, the snapshot, or any tracked file |

To force a local run of a skipped linter:

- `flint run --full` — runs every active linter
- `flint run <linter>` — runs just that one

### Canonical config filenames

When Flint passes config paths explicitly, it supports one canonical config
filename per linter. If an active linter has a known alternate upstream config
file, Flint fails before running the linter instead of silently ignoring or
partially auto-discovering that config.

Move the config to the Flint-managed filename under `FLINT_CONFIG_DIR`, or
remove the alternate file.

> [!NOTE]
> Biome is the exception: its canonical config is root `biome.jsonc`, not a
> file under `FLINT_CONFIG_DIR`.

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

To add or refresh only selected checks without removing unrelated Flint setup,
use `--only`:

```bash
flint init --only rumdl
flint init --only checkstyle dotenv
```

Focused init preserves unrelated tools and configuration, rejects unknown
check names, and remains idempotent. The existing no-argument and profile-based
forms continue to discover and reconcile the repository's complete Flint setup.

`flint init` pins Flint itself in `mise.toml` so every contributor uses the same
lint binary. For unreleased consumer validation, pass an explicit git revision:

```bash
flint init -y --flint-rev <git-rev>
```

That writes a cargo-backed Flint pin. To return to the released Flint backend
after the release is cut, run `flint init` again without `--flint-rev`.

If you want to test an unreleased Flint branch in a consumer repo without
checking Flint out locally, you can also pin it directly in `mise.toml`, for
example:

```toml
[tools]
"cargo:https://github.com/grafana/flint" = "rev:<git-ref>"
```

Replace `<git-ref>` with the branch, tag, or commit you want to test. If you
have Flint checked out locally, prefer `cargo run` / `cargo test` there
instead.
See [CONTRIBUTING.md](../CONTRIBUTING.md) for the broader local development
workflow.

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

Use `--json` for complete registry and setup metadata, including the declared
version, canonical install key, config locations, upstream links, fix behavior,
formatter relationships, and baseline triggers.
