# `ktlint`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                            |
| -------- | ------------------------------------------ |
| Project  | [ktlint](https://github.com/ktlint/ktlint) |
| Fix      | yes                                        |
| Binary   | `ktlint`                                   |
| Scope    | [files](../linters.md#scope-files)         |
| Patterns | `*.kt *.kts`                               |

<!-- linter-metadata-end -->

`ktlint` lints and formats Kotlin source and script files. On a normal run,
Flint passes only changed files to `ktlint`. A full run passes the project root
instead, which is useful after changing ktlint rules:

```bash
flint run --full ktlint
flint run --full --fix ktlint
```

The same check both reports style violations and applies ktlint formatting in
fix mode.
