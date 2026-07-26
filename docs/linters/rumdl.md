# [`rumdl`](https://rumdl.dev/)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                       |
| ----------- | --------------------------------------------------------------------- |
| Fix         | yes                                                                   |
| Binary      | `rumdl`                                                               |
| Scope       | [files](../linters.md#scope-files)                                    |
| Patterns    | `*.md`                                                                |
| Config      | [`.rumdl.toml`](https://rumdl.dev/mdformat-comparison/#configuration) |
| Description | Lint Markdown files for style and consistency                         |

<!-- linter-metadata-end -->

`rumdl` checks changed Markdown files for formatting and style issues and can
fix supported findings in place:

```bash
flint run --fix rumdl
```

Put Markdown rules in `$FLINT_CONFIG_DIR/.rumdl.toml`; Flint passes the file
explicitly. Flint also disables EditorConfig line-length enforcement for
Markdown so rumdl remains the authority for wrapping and line length.
