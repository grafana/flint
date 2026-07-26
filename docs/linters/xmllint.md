# `xmllint`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                   |
| -------- | ------------------------------------------------- |
| Project  | [xmllint](https://github.com/jonwiggins/xmloxide) |
| Fix      | no                                                |
| Binary   | `xmllint`                                         |
| Scope    | [files](../linters.md#scope-files)                |
| Patterns | `*.xml`                                           |

<!-- linter-metadata-end -->

`xmllint` validates that XML files are well-formed. Flint uses the command
provided by `xmloxide` to parse changed files without producing output.
Malformed elements, attributes, or document structure fail the check with a
parser diagnostic.

This is a validation-only check. It does not format XML or validate documents
against a schema.
