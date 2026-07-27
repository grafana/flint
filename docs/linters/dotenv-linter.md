# `dotenv-linter`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                 |
| -------- | --------------------------------------------------------------- |
| Project  | [dotenv-linter](https://github.com/dotenv-linter/dotenv-linter) |
| Fix      | yes                                                             |
| Binary   | `dotenv-linter`                                                 |
| Scope    | [files](../linters.md#scope-files)                              |
| Patterns | `.env .env.* *.env`                                             |

<!-- linter-metadata-end -->

`dotenv-linter` checks and safely formats dotenv environment files without
printing their values.

> [!WARNING]
> Do not commit secret-bearing `.env` files.

Flint checks only explicit `.env`-style files: `.env`, `.env.*`, and files
ending in `.env`. It passes those file paths rather than a directory, so
unrelated YAML, Compose, and application configuration files are never scanned.

Both check and fix mode disable dotenv-linter's update check, avoiding
unexpected network access. Fix mode also uses `--no-backup`, so it does not
leave secret-bearing backup files behind:

```bash
flint run --fix dotenv-linter
```

The fixer is serialized with other Flint formatters that may own the same file.
