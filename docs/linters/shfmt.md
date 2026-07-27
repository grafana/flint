# `shfmt`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                      |
| -------- | ------------------------------------ |
| Project  | [shfmt](https://github.com/mvdan/sh) |
| Fix      | yes                                  |
| Binary   | `shfmt`                              |
| Scope    | [file](../linters.md#scope-file)     |
| Patterns | `*.sh *.bash`                        |

<!-- linter-metadata-end -->

`shfmt` checks and formats shell scripts. Flint checks each changed script by
asking `shfmt` for a diff; fix mode writes the canonical formatting to the
file:

```bash
flint run --fix shfmt
```

Use [`shellcheck`](shellcheck.md) alongside it when you also want semantic shell
linting; `shfmt` is responsible only for formatting.
