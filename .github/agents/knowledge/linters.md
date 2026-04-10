# Adding a New Linter

Add an entry to `builtin()` in `src/registry.rs` using the
builder pattern:

```rust
// File scope — invoked per file
Check::file("mytool", "mytool --check {FILE}", &["*.ext"])
    .fix("mytool --fix {FILE}"),

// Files scope — invoked once with all matched files (absolute paths)
Check::files("mytool", "mytool {FILES}", &["*.ext"])
    .fix("mytool --fix {FILES}"),

// Files scope — invoked once with all matched files (relative to project root)
// Use {RELFILES} when the tool requires paths relative to the project root
// (e.g. dotnet format --include).
Check::files("mytool", "mytool --include {RELFILES}", &["*.ext"])
    .fix("mytool --fix --include {RELFILES}"),

// Project scope — invoked once, skipped if no *.ext changed
Check::project("mytool", "mytool run", &["*.ext"]),
```

Available builder modifiers:

| Method                       | Purpose                                                                       |
| ---------------------------- | ----------------------------------------------------------------------------- |
| `.fix(cmd)`                  | Enable `--fix` mode with this command                                         |
| `.bin(name)`                 | Override binary name (when check name ≠ binary)                               |
| `.mise_tool(name)`           | Look up availability under a different mise key (e.g. `rust` for `cargo-fmt`) |
| `.version_req(range)`        | Restrict to a semver range (e.g. `">=1.0.0"`)                                 |
| `.excludes(names)`           | Skip files already owned by these active checks                               |
| `.slow()`                    | Mark as slow — skipped by `--fast-only`                                       |
| `.linter_config(file, flag)` | Inject a config flag when `FLINT_CONFIG_DIR/<file>` exists (see below)        |

## Config File Injection (`.linter_config`)

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
  function (no explicit `--config` flag exists)

Look up the tool's `--help` or man page for the config flag name and expected
argument type before adding `.linter_config`.

For checks that need custom logic (not a simple command template), add a module
under `src/linters/` and use `CheckKind::Special`.

## Changed-files scoping

Most linters use `file` or `files` scope, so they naturally receive only changed
files as arguments. `golangci-lint` uses `project` scope but scopes internally via
`--new-from-rev={MERGE_BASE}`.

**`cargo-clippy` cannot scope to changed files.** Cargo has no git-aware flag
equivalent to `--new-from-rev`. It still skips entirely when no `*.rs` files
changed, but when it does run it checks the whole project. Workspace support
(`-p <pkg> --no-deps` per changed package) would be a future improvement.
