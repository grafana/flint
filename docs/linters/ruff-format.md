# [`ruff-format`](https://docs.astral.sh/ruff/)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                           |
| ----------- | --------------------------------------------------------- |
| Fix         | yes                                                       |
| Binary      | `ruff`                                                    |
| Scope       | [file](../linters.md#scope-file)                          |
| Patterns    | `*.py`                                                    |
| Config      | [`ruff.toml`](https://docs.astral.sh/ruff/configuration/) |
| Description | Format Python code                                        |

<!-- linter-metadata-end -->

`ruff-format` checks changed Python files with Ruff's formatter. It does not
write during a normal run; use fix mode to format them:

```bash
flint run --fix ruff-format
```

It shares `$FLINT_CONFIG_DIR/ruff.toml` with [`ruff`](ruff.md). When both checks
need fixes, Flint applies lint fixes before formatting.
