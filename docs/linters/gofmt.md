# `gofmt`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                       |
| -------- | ------------------------------------- |
| Project  | [gofmt](https://pkg.go.dev/cmd/gofmt) |
| Fix      | yes                                   |
| Binary   | `gofmt`                               |
| Scope    | [file](../linters.md#scope-file)      |
| Patterns | `*.go`                                |

<!-- linter-metadata-end -->

`gofmt` checks and formats Go code. Flint runs it on each changed Go file,
printing a formatting diff without modifying the file; fix mode writes the
canonical Go formatting:

```bash
flint run --fix gofmt
```

The `go` tool entry in `mise.toml` provides `gofmt`, so no separate formatter
tool is required.
