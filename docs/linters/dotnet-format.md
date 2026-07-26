# [`dotnet-format`](https://learn.microsoft.com/dotnet/core/tools/dotnet-format)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                    |
| ----------- | ---------------------------------- |
| Fix         | yes                                |
| Binary      | `dotnet`                           |
| Scope       | [files](../linters.md#scope-files) |
| Patterns    | `*.cs`                             |
| Description | Format C# code                     |

<!-- linter-metadata-end -->

For a normal changed-files run, Flint passes the changed C# paths to
`dotnet format --include`. The paths are relative to the project root, as
required by the .NET CLI. A full run omits `--include` and checks the entire
solution or project:

```bash
flint run --full dotnet-format
flint run --full --fix dotnet-format
```
