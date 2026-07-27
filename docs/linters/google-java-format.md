# `google-java-format`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                    |
| -------- | ------------------------------------------------------------------ |
| Project  | [google-java-format](https://github.com/google/google-java-format) |
| Fix      | yes                                                                |
| Binary   | `google-java-format`                                               |
| Scope    | [native](../linters.md#scope-native)                               |
| Patterns | `*.java`                                                           |

<!-- linter-metadata-end -->

`google-java-format` checks and formats Java code. Flint can limit ownership to
configured paths and preserve regions delimited by repository-specific
formatter-off markers.

Configure google-java-format options and file ownership in `flint.toml`:

```toml
[checks.google-java-format]
patterns = ["src/**/*.java"]
exclude = ["src/generated/**"]
skip_reflowing_long_strings = true
skip_sorting_imports = false
skip_removing_unused_imports = false
skip_javadoc_formatting = false
aosp = false
```

`patterns` defaults to `["*.java"]`. Both `patterns` and `exclude` use the same
glob syntax as `[settings].exclude`.

`skip_reflowing_long_strings` defaults to `true` in Flint to match the
established Spotless behavior. Set it to `false` to use google-java-format's
upstream default. The other google-java-format options default to `false`.

## Preserving formatter-off regions

For formatter-off directives that google-java-format does not understand,
configure one or more marker pairs:

```toml
[checks.google-java-format]
off_on_markers = [
  { off = "// spotless:off", on = "// spotless:on" },
]
```

Flint formats a temporary copy of a marked file, restores the protected
regions, and then compares or writes the result. Marker lines and everything
between them remain unchanged.

Each marker pair must be balanced. Flint reports a configuration error when it
finds an `on` marker without a preceding `off` marker or an `off` marker without
a following `on` marker.

Run the check explicitly while setting it up:

```bash
flint run --full google-java-format
flint run --full --fix google-java-format
```
