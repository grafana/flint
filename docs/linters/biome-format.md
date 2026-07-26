# [`biome-format`](https://biomejs.dev/)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                              |
| -------- | ------------------------------------------------------------ |
| Fix      | yes                                                          |
| Binary   | `biome`                                                      |
| Scope    | [file](../linters.md#scope-file)                             |
| Patterns | `*.json *.jsonc *.js *.ts *.jsx *.tsx`                       |
| Config   | [`biome.jsonc`](https://biomejs.dev/guides/configure-biome/) |

<!-- linter-metadata-end -->

`biome-format` is the formatting half of Flint's Biome integration. It checks
changed JavaScript, TypeScript, JSX, TSX, JSON, and JSONC files without writing
them; use `flint run --fix biome-format` to apply Biome's formatter.

Flint uses the repository-root `biome.jsonc` as the single Biome configuration
for both [`biome`](biome.md) and `biome-format`. When both checks need fixes,
Flint applies lint fixes first and formatting second.
