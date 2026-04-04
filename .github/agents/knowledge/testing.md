# Testing

Run all tests with:

```bash
cargo test
```

## Unit Tests

In-module `#[cfg(test)]` blocks in `src/`. Notable:

- `src/registry.rs`: enforces version-range consistency
- `src/runner.rs`: config injection, scope filtering
- `src/linters/renovate_deps.rs`: log parsing, snapshot
  read/write, diff output

## Fixture-based E2E Tests

`tests/cases/` holds one directory per scenario. Each
contains:

- `files/` — files copied verbatim into a temp git repo
  and staged before the run
- `test.toml` — test spec:

```toml
[expected]
args = "--full shellcheck"
exit = 1                    # optional, default 0
stderr = """
...golden output...
"""

[expected.files]            # optional: assert files written by --fix
".github/renovate-tracked-deps.json" = """
{...}
"""

[env]                       # optional extra env vars
FOO = "bar"

[fake_bins]                 # optional fake binaries (Unix only)
renovate = '''
#!/bin/sh
echo '...'
'''
```

The `cases` test in `tests/e2e.rs` runs all of them.
Set `UPDATE_SNAPSHOTS=1` to regenerate `[expected].exit`/
`stderr`/`stdout` in place. `[expected.files]` and `[fake_bins]`
are always preserved by the snapshot writer.

Use fixture cases for any check — including ones that require
fake external binaries (via `[fake_bins]`). The fixture runner
writes each binary into a tempdir and prepends it to `PATH`.

When adding a new check, cover at least: clean pass, failure
with correct diff/output, and fix mode if supported.
