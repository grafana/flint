# `golangci-lint`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                   |
| -------- | ----------------------------------------------------------------- |
| Project  | [golangci-lint](https://golangci-lint.run/)                       |
| Fix      | no                                                                |
| Binary   | `golangci-lint`                                                   |
| Scope    | [project](../linters.md#scope-project)                            |
| Patterns | `*.go`                                                            |
| Config   | [`.golangci.yml`](https://golangci-lint.run/usage/configuration/) |

<!-- linter-metadata-end -->

`golangci-lint` lints Go code with a configurable suite of analyzers. It is
project-scoped because many analyzers need package-wide context. Flint still
focuses the result on new code by passing the Git merge base through
`--new-from-rev`.

Put the supported `.golangci.yml` in `$FLINT_CONFIG_DIR`; Flint passes it with
`--config`. Use a full run when validating a new configuration or auditing
existing findings:

```bash
flint run --full golangci-lint
```
