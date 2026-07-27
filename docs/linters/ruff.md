# `ruff`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                           |
| -------- | --------------------------------------------------------- |
| Project  | [ruff](https://docs.astral.sh/ruff/)                      |
| Fix      | yes                                                       |
| Binary   | `ruff`                                                    |
| Scope    | [file](../linters.md#scope-file)                          |
| Patterns | `*.py`                                                    |
| Config   | [`ruff.toml`](https://docs.astral.sh/ruff/configuration/) |

<!-- linter-metadata-end -->

`ruff` runs Python lint rules against each changed file. Fix mode applies the
safe edits available through `ruff check --fix`:

```bash
flint run --fix ruff
```

Put the supported configuration in `$FLINT_CONFIG_DIR/ruff.toml`. Formatting
is a separate check, [`ruff-format`](ruff-format.md), so either capability can
be enabled and run independently.
