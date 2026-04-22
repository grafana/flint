# Key Design Decisions

1. **Activation via `mise.toml`**: A check is active when
   its tool (or `mise_tool_name` override) is declared in
   the consuming repo's `mise.toml`. No PATH probing —
   mise guarantees declared tools are on PATH.

2. **`editorconfig-checker` deference**: `editorconfig-checker`
   (binary: `ec`) runs on all files but skips file types owned
   by active line-length-enforcing formatters (`cargo-fmt`,
   `ruff-format`, `biome-format`, `rumdl`, `yaml-lint`). Implemented
   via `.defer_to_formatters()` on the `editorconfig-checker`
   entry. This avoids its `max_line_length` check conflicting
   with formatter output.

3. **Rust-native docs/config stack**: Markdown is owned by
   `rumdl`, YAML by `yaml-lint`, and JS/TS/JSON by `biome`.
   This keeps ownership boundaries explicit and avoids the
   old markdownlint/prettier overlap on `*.md`.

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
   generated file that should never be linted by `rumdl`,
   ec, etc.). Add entries here — not in user-facing `exclude`
   docs — when a file is managed by flint itself.
