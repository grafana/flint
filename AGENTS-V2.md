# AGENTS-V2.md

Guidance for working on flint v2 — the Rust binary.
For v1 (bash task scripts), see [AGENTS-V1.md](AGENTS-V1.md).

## Repository Overview

v2 is a single Rust binary (`flint`) that discovers linting
tools from the consuming repo's `mise.toml`, runs them
against changed files in parallel, and produces identical
output locally and in CI.

See [README.md](README.md) for usage documentation.

## Architecture

### Module Map

- **`src/registry.rs`**: Static linter registry. Defines
  `Check` (builder pattern) and `builtin()` which returns
  the full list of built-in checks. This is where new
  linters are added.
- **`src/runner.rs`**: Executes checks against a file list.
  Handles parallel execution (check mode) and serial
  execution (fix mode, to avoid concurrent writes).
- **`src/config.rs`**: Loads `flint.toml` from the project
  root. All fields have defaults — the file is optional.
- **`src/files.rs`**: Git-aware file discovery. Returns
  changed files relative to the merge base, or all files
  with `--full`.
- **`src/linters/`**: Custom logic for special checks that
  can't be expressed as a simple command template:
  - `lychee.rs`: Link checking orchestration
  - `renovate_deps.rs`: Renovate snapshot verification
- **`src/main.rs`**: CLI parsing (clap), orchestration,
  output formatting.
- **`tests/e2e.rs`**: End-to-end tests. Spin up a temp git
  repo, write files, run the flint binary, assert on
  stdout/stderr and exit code.

### Check Kinds

A `Check` is either a `Template` (a command string with
`{FILE}`, `{FILES}`, or `{MERGE_BASE}` placeholders) or a
`Special` (custom Rust logic in `src/linters/`).

Template scopes:

- `File` — invoked once per matched file (`{FILE}`)
- `Files` — invoked once with all matched files (`{FILES}`)
- `Project` — invoked once with no file args; skipped
  entirely if no matching files changed

### Adding a New Linter

Add an entry to `builtin()` in `src/registry.rs` using the
builder pattern:

```rust
// File scope — invoked per file
Check::file("mytool", "mytool --check {FILE}", &["*.ext"])
    .fix("mytool --fix {FILE}"),

// Files scope — invoked once with all matched files
Check::files("mytool", "mytool {FILES}", &["*.ext"])
    .fix("mytool --fix {FILES}"),

// Project scope — invoked once, skipped if no *.ext changed
Check::project("mytool", "mytool run", &["*.ext"]),
```

Available builder modifiers:

| Method | Purpose |
|---|---|
| `.fix(cmd)` | Enable `--fix` mode with this command |
| `.bin(name)` | Override binary name (when check name ≠ binary) |
| `.mise_tool(name)` | Look up availability under a different mise key (e.g. `rust` for `cargo-fmt`) |
| `.version_req(range)` | Restrict to a semver range (e.g. `">=1.0.0"`) |
| `.excludes(names)` | Skip files already owned by these active checks |
| `.slow()` | Mark as slow — skipped by `--fast` |
| `.linter_config(file, flag)` | Inject a config flag when `FLINT_CONFIG_DIR/<file>` exists (see below) |

#### Config file injection (`.linter_config`)

Use `.linter_config(filename, flag)` when the tool supports an explicit config
file path via a CLI flag. At runtime, if `FLINT_CONFIG_DIR/<filename>` exists,
flint injects `flag <abs-path>` right after the binary name in the command.
If the file is absent the flag is silently omitted — native config discovery
remains in effect.

```rust
// Example: markdownlint accepts --config <path>
Check::file("markdownlint", "markdownlint {FILE}", &["*.md"])
    .fix("markdownlint --fix {FILE}")
    .linter_config(".markdownlint.json", "--config"),
// → markdownlint --config /repo/.github/config/.markdownlint.json <file>
```

**When NOT to use it:**
- The tool has no explicit `--config`/`--rcfile`/equivalent flag (e.g. `shfmt`)
- The flag accepts a **directory** rather than a file (e.g. biome's
  `--config-path <dir>`) — a different injection shape is needed. For biome,
  check for `biome.json` existence but pass `config_dir` itself as the arg:
  `biome --config-path <config_dir> check <file>`. This requires a variant of
  `.linter_config` that injects the directory rather than the full file path
  (not yet implemented)
- The tool is project-scoped and its config must live at the project root to
  function (e.g. `cargo-fmt` reads `rustfmt.toml` via Cargo, not a direct flag)

Look up the tool's `--help` or man page for the config flag name and expected
argument type before adding `.linter_config`.

For checks that need custom logic (not a simple command
template), add a module under `src/linters/` and use
`CheckKind::Special`.

### Key Design Decisions

1. **Activation via `mise.toml`**: A check is active when
   its tool (or `mise_tool_name` override) is declared in
   the consuming repo's `mise.toml`. No PATH probing —
   mise guarantees declared tools are on PATH.

2. **`ec` deference**: `ec` (editorconfig-checker) runs on
   all files but skips file types owned by active
   line-length-enforcing formatters (`cargo-fmt`,
   `ruff-format`, `biome-format`, `prettier`). Implemented
   via `.excludes(&[...])` on the `ec` entry. This avoids
   `ec`'s `max_line_length` check conflicting with
   formatter output.

3. **markdownlint + prettier on `*.md`**: Both checkers are
   active when their tools are installed. They cover
   different concerns (markdownlint: structural rules;
   prettier: formatting). To avoid MD013 (line length)
   conflicting with prettier's line wrapping, consuming
   repos must disable MD013 in `.markdownlint.json`:
   ```json
   { "MD013": false }
   ```

4. **Fix mode runs serially**: `runner.rs` runs checks in
   parallel in check mode, but serially in fix mode to
   avoid concurrent writes to the same file.

5. **Version ranges**: When a `bin_name` has any
   `version_range` entries, every entry for that binary
   must have one (enforced by a registry unit test). This
   prevents ambiguous activation when ranges don't cover
   all versions.

6. **Special checks**: `links` and `renovate-deps` have
   custom orchestration logic that doesn't fit the command
   template model. Their implementations live in
   `src/linters/`.

7. **Built-in file exclusions**: `src/files.rs` has a
   `BUILTIN_EXCLUDES` slice of paths that are always removed
   from the file list before any linter sees it. Currently
   contains `.github/renovate-tracked-deps.json` (a
   generated file that should never be linted by prettier,
   ec, etc.). Add entries here — not in user-facing `exclude`
   docs — when a file is managed by flint itself.

## Testing

Run all tests with:

```bash
cargo test
```

### Unit tests

In-module `#[cfg(test)]` blocks in `src/`. Notable:
- `src/registry.rs`: enforces version-range consistency
- `src/runner.rs`: config injection, scope filtering
- `src/linters/renovate_deps.rs`: log parsing, snapshot
  read/write, diff output

### Fixture-based e2e tests

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
