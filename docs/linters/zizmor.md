# `zizmor`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                       |
| -------- | ----------------------------------------------------- |
| Project  | [zizmor](https://github.com/zizmorcore/zizmor)        |
| Fix      | yes                                                   |
| Binary   | `zizmor`                                              |
| Scope    | [files](../linters.md#scope-files)                    |
| Patterns | `.github/workflows/*.yml .github/workflows/*.yaml`    |
| Config   | [`zizmor.yml`](https://docs.zizmor.sh/configuration/) |

<!-- linter-metadata-end -->

`zizmor` audits GitHub Actions workflows for security issues.

zizmor can drift without file changes: its `ref-version-mismatch`
audit resolves pinned action hashes against GitHub's tag API at
run-time. When a maintainer moves a mutable tag (e.g. `v6` advances
to a new patch), workflows pinned to the old commit but commented
`# v6` become inconsistent without any local file change. Flint
scans only files changed in the PR, so drift in untouched workflows
stays invisible until something edits them. Run `flint run --full`
periodically (e.g. weekly `schedule:` workflow) to catch this.
