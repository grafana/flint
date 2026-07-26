# [`actionlint`](https://github.com/rhysd/actionlint)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                                  |
| ----------- | -------------------------------------------------------------------------------- |
| Fix         | no                                                                               |
| Binary      | `actionlint`                                                                     |
| Scope       | [file](../linters.md#scope-file)                                                 |
| Patterns    | `.github/workflows/*.yml .github/workflows/*.yaml`                               |
| Config      | [`actionlint.yml`](https://github.com/rhysd/actionlint/blob/main/docs/config.md) |
| Description | Lint GitHub Actions workflow files                                               |

<!-- linter-metadata-end -->

Flint runs `actionlint` on changed workflow files under `.github/workflows/`.
This catches workflow syntax errors, invalid expressions, and common GitHub
Actions mistakes before the workflow reaches GitHub.

Put project-specific actionlint settings in
`$FLINT_CONFIG_DIR/actionlint.yml`; Flint passes that file explicitly when it
exists. For example:

```yaml
self-hosted-runner:
  labels:
    - linux-arm64
```
