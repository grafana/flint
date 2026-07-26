# [`hadolint`](https://github.com/hadolint/hadolint)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                                       |
| ----------- | ------------------------------------------------------------------------------------- |
| Fix         | no                                                                                    |
| Binary      | `hadolint`                                                                            |
| Scope       | [file](../linters.md#scope-file)                                                      |
| Patterns    | `Dockerfile Dockerfile.* *.dockerfile`                                                |
| Config      | [`.hadolint.yaml`](https://github.com/hadolint/hadolint?tab=readme-ov-file#configure) |
| Description | Lint Dockerfiles                                                                      |

<!-- linter-metadata-end -->

Flint runs `hadolint` for changed files named `Dockerfile`, `Dockerfile.*`, or
`*.dockerfile`. For example, it can flag unpinned packages, fragile shell
commands, and Dockerfile instructions that work against container best
practices.

Put repository-specific rules in `$FLINT_CONFIG_DIR/.hadolint.yaml`; Flint
passes that file explicitly when it exists.
