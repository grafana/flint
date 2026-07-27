# `license-header`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|        |                                             |
| ------ | ------------------------------------------- |
| Fix    | no                                          |
| Binary | (built-in)                                  |
| Scope  | [native](../linters.md#scope-native)        |
| Config | via `[checks.license-header]` in flint.toml |

<!-- linter-metadata-end -->

`license-header` checks that selected source files contain the required header
text near the top. It is disabled by default; configure it in `flint.toml`:

```toml
[checks.license-header]
text = "SPDX-License-Identifier: Apache-2.0"
patterns = ["*.java", "*.kt", "*.scala", "*.groovy"]
exclude = ["package-info.java"]
lines_to_check = 5
```

- `text` — required header text to find near the top of each file
- `patterns` — glob patterns selecting which files to check
- `exclude` — glob patterns excluded after `patterns`
- `lines_to_check` — how many leading lines to search; defaults to `5`

`text` may be multi-line. Flint joins the first `lines_to_check` lines with
newlines and checks whether that text contains the configured header snippet.
