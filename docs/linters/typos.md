# [`typos`](https://github.com/crate-ci/typos)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                                  |
| ----------- | -------------------------------------------------------------------------------- |
| Fix         | yes                                                                              |
| Binary      | `typos`                                                                          |
| Scope       | [files](../linters.md#scope-files)                                               |
| Patterns    | `*`                                                                              |
| Config      | [`_typos.toml`](https://github.com/crate-ci/typos/blob/master/docs/reference.md) |
| Description | Check for common spelling mistakes                                               |

<!-- linter-metadata-end -->

`typos` scans changed source and text files for likely spelling mistakes. Fix
mode writes corrections that the tool considers unambiguous:

```bash
flint run --fix typos
```

Put accepted words, identifier rules, and file exclusions in
`$FLINT_CONFIG_DIR/_typos.toml`. Flint passes `--force-exclude`, so configured
exclusions are honored even when changed paths are supplied explicitly.
