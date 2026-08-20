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

Untracked files are not included until they are staged. `--full` selects all
eligible tracked files; it does not include untracked files.

`--new-from-rev REV` replaces the configured base branch when resolving the
merge base. `--to-ref REF` replaces `HEAD` as the end of the committed range.
These options have the same meaning as their `flint run` counterparts. If a
merge base cannot be resolved, Flint falls back to full tracked-file
selection, as it does for a lint run.

The revision options also accept values from the `FLINT_NEW_FROM_REV` and
`FLINT_TO_REF` environment variables. This is useful when a shared task
forwards the same revision context to both `flint run` and `flint changed-files`.
An explicitly supplied command-line option takes precedence over its
environment variable.

In either mode, Flint first excludes paths that:

- do not exist in the worktree, including deleted files;
- are marked `linguist-generated` in `.gitattributes`;
- are built-in Flint-managed paths; or
- match `settings.exclude` in `flint.toml`.

The remaining paths are repository-relative, sorted deterministically, and
printed without diagnostic output on stdout. Errors are reported on stderr.
Flint currently represents Git paths as UTF-8 strings. A path containing
invalid UTF-8 bytes is lossily converted; `--null` makes separators safe for
whitespace and newlines, but does not preserve arbitrary non-UTF-8 bytes.

Flint computes the complete deterministic set before emitting it, then writes
the output incrementally. For large or unusual path sets, consume `--null`
directly as a stream rather than using shell command substitution. Pagination
is intentionally not provided: the command emits one complete snapshot.

## Filtering

Use `--include GLOB` and `--exclude GLOB` to select a subset of Flint's
already-eligible paths. Each option may be repeated. Includes are combined with
OR; exclusions are applied afterward and always win. With no includes, all
eligible paths are candidates. An include cannot re-include a generated,
built-in, configured-excluded, deleted, or untracked path.

Patterns match either a repository-relative path or its basename, so `*.scala`
matches Scala files in any directory while `src/**/*.scala` restricts the
selection to `src`. For example, a formatter task can select Scala and Groovy
files while retaining Flint's change semantics:

```bash
flint changed-files --null \
  --include '*.scala' --include '*.groovy' \
  --exclude 'src/generated/**'
```

Filtering is generic rather than formatter-specific. A caller still decides
which patterns belong to Spotless, Flint, or another tool.

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

On Unix, `xargs -0` can batch paths for tools that accept file arguments, but
its batching options and availability vary across platforms. Cross-platform
callers should parse the NUL stream incrementally and choose their own
argument-size limit. If a caller only needs to know whether any matching path
exists, it can stop after the first relevant record instead of collecting the
whole list.

Use a NUL-aware reader in other languages, for example Python:

```python
import subprocess

paths = subprocess.check_output(["flint", "changed-files", "--null"])
payload = paths.rstrip(b"\0")
files = payload.split(b"\0") if payload else []
```

See the [CLI reference](cli.md#flint-changed-files) for the complete command
overview.
