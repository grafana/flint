# `checkstyle`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                        |
| -------- | ------------------------------------------------------ |
| Project  | [checkstyle](https://github.com/checkstyle/checkstyle) |
| Fix      | no                                                     |
| Binary   | `checkstyle`                                           |
| Scope    | [files](../linters.md#scope-files)                     |
| Patterns | `*.java`                                               |
| Config   | [`checkstyle.xml`](https://checkstyle.org/config.html) |

<!-- linter-metadata-end -->

`checkstyle` checks Java source against a repository-owned coding standard. It
is report-only; use a formatter such as
[`google-java-format`](google-java-format.md) for safe formatting fixes.

Flint runs the standalone Checkstyle CLI against selected Java files. A Java
runtime must be available on `PATH` because Checkstyle is distributed as a JAR.
Flint resolves that JAR from the direct `checkstyle` entry in `mise.toml` and
invokes it with `java -jar` on every platform.

The repository must provide `checkstyle.xml` at its root. A root-level
`checkstyle-suppressions.xml` is also supported through Checkstyle's standard
property default. Flint does not infer Maven or Gradle source roots.
