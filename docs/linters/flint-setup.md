# `flint-setup`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->
|          |                                      |
| -------- | ------------------------------------ |
| Fix      | yes                                  |
| Binary   | (built-in)                           |
| Scope    | [native](../linters.md#scope-native) |
| Patterns | `mise.toml`                          |

Keep Flint setup current and mise.toml lint tooling canonical
<!-- linter-metadata-end -->

Checks the repo's Flint-managed setup state and `mise.toml` layout.

This verifies and fixes Flint-managed setup:
- apply versioned Flint setup migrations
- replace obsolete lint tool keys with their supported successors
- reject unsupported legacy lint tools that need repo migrations
- sort `[tools]` entries into Flint's canonical order
- keep lint-managed tool entries under the `# Linters` header
- keep runtime, SDK, and unknown tool entries above that header

With `--fix`, rewrites Flint-managed config in place and applies any
currently actionable setup migration.
