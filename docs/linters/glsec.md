# `glsec`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                           |
| -------- | --------------------------------------------------------- |
| Project  | [glsec](https://github.com/glsec/glsec)                   |
| Fix      | no                                                        |
| Binary   | `glsec`                                                   |
| Scope    | [files](../linters.md#scope-files)                        |
| Patterns | `.gitlab-ci.yml *.gitlab-ci.yml`                          |
| Config   | [`.glsec.yml`](https://github.com/glsec/glsec#exit-codes) |

<!-- linter-metadata-end -->

`glsec` audits GitLab CI/CD pipeline files for security issues, such as
mutable image tags, remote scripts piped into a shell, user-controlled
variables in scripts and secrets in `variables:`. Each finding names a rule
with a page at <https://glsec.github.io/rules/>.

Flint passes the changed `.gitlab-ci.yml` and `*.gitlab-ci.yml` files.
glsec follows local child pipelines (`trigger: include:` with a local path)
from those files, but it does not fetch remote, `project:`, `template:` or
`component:` includes.

glsec exits 1 on `error` findings. `warn` findings are printed but don't
fail the check unless `strict: true` is set in `FLINT_CONFIG_DIR/.glsec.yml`.

glsec is not in the mise registry, so it is installed through mise's GitHub
backend:

```toml
[tools]
"github:glsec/glsec" = "1.23.0"
```
