# [`xmllint`](https://github.com/jonwiggins/xmloxide)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|             |                                    |
| ----------- | ---------------------------------- |
| Fix         | no                                 |
| Binary      | `xmllint`                          |
| Scope       | [files](../linters.md#scope-files) |
| Patterns    | `*.xml`                            |
| Description | Validate XML files are well-formed |

<!-- linter-metadata-end -->

Flint uses the `xmllint` command provided by `xmloxide` to parse changed XML
files without producing output. A clean run means every selected file is
well-formed; malformed elements, attributes, or document structure fail the
check with a parser diagnostic.

This is a validation-only check. It does not format XML or validate documents
against a schema.
