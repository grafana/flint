# [`cargo-clippy`](https://doc.rust-lang.org/clippy/configuration.html)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                        |
| -------- | -------------------------------------- |
| Fix      | yes                                    |
| Binary   | `cargo-clippy`                         |
| Scope    | [project](../linters.md#scope-project) |
| Patterns | `*.rs`                                 |

<!-- linter-metadata-end -->

`cargo-clippy` starts when Rust files change, but Cargo cannot restrict Clippy
to only those files. Flint therefore runs Clippy across all targets in the
project and treats every warning as an error.

Fix mode uses `cargo clippy --fix`, which may resolve only part of a failing
run. Review the remaining diagnostics and the resulting diff before committing:

```bash
flint run --fix cargo-clippy
```
