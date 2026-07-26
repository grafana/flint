# [`ryl`](https://github.com/owenlamont/ryl)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                                 |
| ----------- | ------------------------------------------------------------------------------- |
| Fix         | yes                                                                             |
| Binary      | `ryl`                                                                           |
| Scope       | [files](../linters.md#scope-files)                                              |
| Patterns    | `*.yml *.yaml`                                                                  |
| Config      | [`.yamllint.yml`](https://yamllint.readthedocs.io/en/stable/configuration.html) |
| Description | Lint YAML files for style and consistency                                       |

<!-- linter-metadata-end -->

`ryl` checks changed YAML files for syntax and style consistency and applies
supported formatting changes in fix mode:

```bash
flint run --fix ryl
```

Flint uses `$FLINT_CONFIG_DIR/.yamllint.yml` for the rule set. The check keeps
the familiar yamllint configuration format while using `ryl` as the runner.
