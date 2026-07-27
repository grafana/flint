# `google-java-format`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                    |
| -------- | ------------------------------------------------------------------ |
| Project  | [google-java-format](https://github.com/google/google-java-format) |
| Fix      | yes                                                                |
| Binary   | `google-java-format`                                               |
| Scope    | [files](../linters.md#scope-files)                                 |
| Patterns | `*.java`                                                           |

<!-- linter-metadata-end -->

`google-java-format` checks and formats Java code. Flint uses its dry-run mode
on changed files; apply the formatter with:

```bash
flint run --fix google-java-format
```

Flint also disables EditorConfig line-length enforcement for Java files so
google-java-format remains the formatting authority.
