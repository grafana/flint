# [`google-java-format`](https://github.com/google/google-java-format)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                    |
| -------- | ---------------------------------- |
| Fix      | yes                                |
| Binary   | `google-java-format`               |
| Scope    | [files](../linters.md#scope-files) |
| Patterns | `*.java`                           |

<!-- linter-metadata-end -->

Flint checks changed Java files with google-java-format's dry-run mode. Apply
the formatter with:

```bash
flint run --fix google-java-format
```

Flint also disables EditorConfig line-length enforcement for Java files so
google-java-format remains the formatting authority.
