# [`shfmt`](https://github.com/mvdan/sh)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                  |
| ----------- | -------------------------------- |
| Fix         | yes                              |
| Binary      | `shfmt`                          |
| Scope       | [file](../linters.md#scope-file) |
| Patterns    | `*.sh *.bash`                    |
| Description | Format shell scripts             |

<!-- linter-metadata-end -->

Flint checks each changed shell script by asking `shfmt` for a diff. Fix mode
writes shfmt's canonical formatting to the file:

```bash
flint run --fix shfmt
```

Use [`shellcheck`](shellcheck.md) alongside it when you also want semantic shell
linting; `shfmt` is responsible only for formatting.
