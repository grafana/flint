# [`editorconfig-checker`](https://github.com/editorconfig-checker/editorconfig-checker)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->
|          |                                                                                                                               |
| -------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Fix      | no                                                                                                                            |
| Binary   | `ec`                                                                                                                          |
| Scope    | [files](../linters.md#scope-files)                                                                                            |
| Patterns | `*`                                                                                                                           |
| Config   | [`.editorconfig-checker.json`](https://github.com/editorconfig-checker/editorconfig-checker?tab=readme-ov-file#configuration) |

Check files comply with EditorConfig settings
<!-- linter-metadata-end -->

`editorconfig-checker` defers to formatters: it runs on all files
but automatically skips file types owned by an active formatter. If
none of those formatters are installed, `editorconfig-checker` checks
those files itself.

Flint writes shared `.editorconfig` carve-outs for known
formatter-owned line length: today that means `rumdl` for `*.md`,
`rustfmt` for `*.rs`, and `google-java-format` for `*.java`. Those
sections use `max_line_length = off` so editors and
`editorconfig-checker` share the same intent instead of relying on
checker-specific JSON excludes. If a matching section already
exists, `flint init` rewrites its `max_line_length` to `off`
instead of leaving a formatter-conflicting numeric value in place.
