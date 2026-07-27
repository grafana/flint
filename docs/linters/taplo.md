# `taplo`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                    |
| -------- | ------------------------------------------------------------------ |
| Project  | [taplo](https://taplo.tamasfe.dev/)                                |
| Fix      | yes                                                                |
| Binary   | `taplo`                                                            |
| Scope    | [file](../linters.md#scope-file)                                   |
| Patterns | `*.toml`                                                           |
| Config   | [`.taplo.toml`](https://taplo.tamasfe.dev/configuration/file.html) |

<!-- linter-metadata-end -->

`taplo` checks and formats TOML files.

This check intentionally stays basic: it uses `taplo fmt --check` for
verification and `taplo fmt` for `--fix`. That keeps behavior aligned with
flint's existing formatter-style checks.

Current caveat: Taplo's published docs currently advertise TOML 1.0.0
support, so treat this check as TOML 1.0-oriented for now.
