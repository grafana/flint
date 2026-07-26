# [`shellcheck`](https://github.com/koalaman/shellcheck)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                                       |
| -------- | ------------------------------------------------------------------------------------- |
| Fix      | no                                                                                    |
| Binary   | `shellcheck`                                                                          |
| Scope    | [file](../linters.md#scope-file)                                                      |
| Patterns | `*.sh *.bash *.bats`                                                                  |
| Config   | [`.shellcheckrc`](https://github.com/koalaman/shellcheck/blob/master/shellcheck.1.md) |

<!-- linter-metadata-end -->

Flint runs ShellCheck on each changed shell script with external-source
following enabled. `SCRIPTDIR` is added to the source path so a script can
resolve files sourced from its own directory.

Put project-specific rules in `$FLINT_CONFIG_DIR/.shellcheckrc`; Flint passes
that file with `--rcfile`. For example:

```text
shell=bash
disable=SC1091
```
