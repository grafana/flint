# [`biome`](https://biomejs.dev/)

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

`biome` runs Biome's code checks on changed JavaScript, TypeScript, JSX, TSX,
JSON, and JSONC files. Use `flint run --fix biome` to apply the safe fixes that
Biome exposes through `biome check --fix`.

Flint deliberately uses only the repository-root `biome.jsonc`; it reports
`biome.json` as unsupported so local and CI runs cannot silently discover a
different configuration. Formatting is handled separately by
[`biome-format`](biome-format.md).
