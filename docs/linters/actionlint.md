# `actionlint`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                                  |
| -------- | -------------------------------------------------------------------------------- |
| Project  | [actionlint](https://github.com/rhysd/actionlint)                                |
| Fix      | no                                                                               |
| Binary   | `actionlint`                                                                     |
| Scope    | [file](../linters.md#scope-file)                                                 |
| Patterns | `.github/workflows/*.yml .github/workflows/*.yaml`                               |
| Config   | [`actionlint.yml`](https://github.com/rhysd/actionlint/blob/main/docs/config.md) |

<!-- linter-metadata-end -->

`actionlint` lints GitHub Actions workflows for syntax errors, invalid
expressions, and common mistakes. Flint runs it on changed workflow files under
`.github/workflows/` before the workflow reaches GitHub.

Put project-specific actionlint settings in
`$FLINT_CONFIG_DIR/actionlint.yml`; Flint passes that file explicitly when it
exists. For example:

```yaml
self-hosted-runner:
  labels:
    - linux-arm64
```
