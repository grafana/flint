# `license-header`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|        |                                             |
| ------ | ------------------------------------------- |
| Fix    | no                                          |
| Binary | (built-in)                                  |
| Scope  | [native](../linters.md#scope-native)        |
| Config | via `[checks.license-header]` in flint.toml |

Check source files have the required license header
<!-- linter-metadata-end -->

Disabled by default. Configure in `flint.toml`:

```toml
[checks.license-header]
text = "SPDX-License-Identifier: Apache-2.0"
patterns = ["*.java", "*.kt"]
lines_to_check = 5
```

- `text` — required header text to find near the top of each file
- `patterns` — glob patterns selecting which files to check
- `lines_to_check` — how many leading lines to search; defaults to `5`

`text` may be multi-line. Flint joins the first `lines_to_check` lines with
newlines and checks whether that text contains the configured header snippet.
