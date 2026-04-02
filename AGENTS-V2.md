# AGENTS-V2.md

Guidance for working on flint v2 — the Rust binary.
For v1 (bash task scripts), see [AGENTS-V1.md](AGENTS-V1.md).

## Repository Overview

v2 is a single Rust binary (`flint`) that discovers linting
tools from the consuming repo's `mise.toml`, runs them
against changed files in parallel, and produces identical
output locally and in CI.

See [FLINT-V2.md](FLINT-V2.md) for usage documentation.

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

## Testing

### Unit tests

`src/registry.rs` has a unit test that enforces the
version-range consistency invariant. Run with:

```bash
cargo test
```

### End-to-end tests

`tests/e2e.rs` tests the full binary. Each test:

1. Creates a temp directory initialised as a git repo
   (`git_repo()`)
2. Writes a minimal `mise.toml` declaring the tools under
   test (`write_mise_toml()`)
3. Writes and stages test files (`stage()`)
4. Runs `flint` via `Command` and asserts on output/exit

When adding a new linter, add an e2e test that covers at
least: check mode failure output format, and fix mode if
the linter supports it.
