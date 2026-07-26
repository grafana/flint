# [`cargo-fmt`](https://github.com/rust-lang/rustfmt)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                                                                               |
| ----------- | --------------------------------------------------------------------------------------------- |
| Fix         | yes                                                                                           |
| Binary      | `rustfmt`                                                                                     |
| Scope       | [project](../linters.md#scope-project)                                                        |
| Patterns    | `*.rs`                                                                                        |
| Config      | [`rustfmt.toml`](https://github.com/rust-lang/rustfmt?tab=readme-ov-file#configuring-rustfmt) |
| Description | Format Rust code; runs on all .rs files, not just changed                                     |

<!-- linter-metadata-end -->

`cargo-fmt` starts when Rust files change and checks the whole Cargo project,
because `cargo fmt` operates on crates rather than a changed-file list. Use fix
mode to write rustfmt's result:

```bash
flint run --fix cargo-fmt
```

Flint reads `rustfmt.toml` from `$FLINT_CONFIG_DIR` and passes it explicitly.
It also disables EditorConfig line-length enforcement for Rust files so
rustfmt remains the formatting authority.
