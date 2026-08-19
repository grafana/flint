# Changed-file discovery

`flint changed-files` exposes Flint's Git-aware file selection for repository
tasks that need to orchestrate tools other than Flint's built-in checks. It is
also useful for hooks and editor integrations that should select exactly the
same files as `flint run`.

```bash
flint changed-files
```

## Semantics

Without `--full`, Flint selects the union of:

- committed changes in the three-dot range from the merge base with the
  configured base branch to `HEAD`;
- unstaged worktree changes; and
- staged index changes.

Untracked files are not included until they are staged. `--full` lists tracked
files only.

`--new-from-rev REV` replaces the configured base branch when resolving the
merge base. `--to-ref REF` replaces `HEAD` as the end of the committed range.
These options have the same meaning as their `flint run` counterparts. If a
merge base cannot be resolved, Flint falls back to full tracked-file
selection, as it does for a lint run.

The revision options also accept `FLINT_NEW_FROM_REV` and `FLINT_TO_REF`, which
is useful when a shared task forwards the same revision context to both
`flint run` and `flint changed-files`. Explicit command-line options take
precedence over environment variables.

`--full` explicitly selects all tracked files. In either mode, Flint excludes
paths that:

- do not exist in the worktree, including deleted files;
- are marked `linguist-generated` in `.gitattributes`;
- are built-in Flint-managed paths; or
- match `settings.exclude` in `flint.toml`.

The remaining paths are repository-relative, sorted deterministically, and
printed without diagnostic output on stdout. Errors are reported on stderr.
Flint currently represents Git paths as UTF-8 strings. A path containing
invalid UTF-8 bytes is lossily converted; `--null` makes separators safe for
whitespace and newlines, but does not preserve arbitrary non-UTF-8 bytes.

## Output formats

The default output has one path per line:

```text
src/main.rs
src/runner.rs
```

For scripts, use `--null`. It terminates every path with a NUL byte, so spaces,
tabs, quotes, and newlines in a filename cannot be mistaken for separators:

```bash
flint changed-files --null | python -c '
import subprocess, sys
raw = sys.stdin.buffer.read().rstrip(b"\0")
files = raw.split(b"\0") if raw else []
if files:
    raise SystemExit(subprocess.run(["some-formatter", *files]).returncode)
'
```

Use a NUL-aware reader in other languages, for example Python:

```python
import subprocess

paths = subprocess.check_output(["flint", "changed-files", "--null"])
payload = paths.rstrip(b"\0")
files = payload.split(b"\0") if payload else []
```

## Tool-specific filtering

Flint deliberately does not provide formatter-specific flags. Callers should
filter this generic list according to their own ownership rules—for example,
selecting only Scala and Groovy files before invoking Spotless. Keeping that
policy outside Flint allows the same command to serve different repositories,
formatters, hooks, and orchestration tasks while preserving one canonical
definition of changed files.

See the [CLI reference](cli.md#flint-changed-files) for the complete command
overview.
