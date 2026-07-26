# [`gofmt`](https://pkg.go.dev/cmd/gofmt)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                  |
| ----------- | -------------------------------- |
| Fix         | yes                              |
| Binary      | `gofmt`                          |
| Scope       | [file](../linters.md#scope-file) |
| Patterns    | `*.go`                           |
| Description | Format Go code                   |

<!-- linter-metadata-end -->

Flint runs `gofmt` on each changed Go file. The check prints a formatting diff
without modifying the file; fix mode writes the canonical Go formatting:

```bash
flint run --fix gofmt
```

The `go` tool entry in `mise.toml` provides `gofmt`, so no separate formatter
tool is required.
