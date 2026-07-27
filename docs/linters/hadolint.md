# `hadolint`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                                       |
| -------- | ------------------------------------------------------------------------------------- |
| Project  | [hadolint](https://github.com/hadolint/hadolint)                                      |
| Fix      | no                                                                                    |
| Binary   | `hadolint`                                                                            |
| Scope    | [file](../linters.md#scope-file)                                                      |
| Patterns | `Dockerfile Dockerfile.* *.dockerfile`                                                |
| Config   | [`.hadolint.yaml`](https://github.com/hadolint/hadolint?tab=readme-ov-file#configure) |

<!-- linter-metadata-end -->

`hadolint` lints Dockerfiles for common mistakes and container best practices.
Flint runs it for changed files named `Dockerfile`, `Dockerfile.*`, or
`*.dockerfile`. For example, it can flag unpinned packages and fragile shell
commands.

Put repository-specific rules in `$FLINT_CONFIG_DIR/.hadolint.yaml`; Flint
passes that file explicitly when it exists.
