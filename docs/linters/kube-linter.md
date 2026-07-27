# `kube-linter`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|          |                                                                                            |
| -------- | ------------------------------------------------------------------------------------------ |
| Project  | [kube-linter](https://github.com/stackrox/kube-linter)                                     |
| Fix      | no                                                                                         |
| Binary   | `kube-linter`                                                                              |
| Scope    | [native](../linters.md#scope-native)                                                       |
| Patterns | `k8s/*.yml k8s/*.yaml kubernetes/*.yml kubernetes/*.yaml manifests/*.yml manifests/*.yaml` |
| Config   | [`kube-linter.yaml`](https://docs.kubelinter.io/)                                          |

<!-- linter-metadata-end -->

Flint's `kube-linter` integration checks Kubernetes manifests for security and
production-readiness issues. It is report-only: Flint reports findings but does
not modify manifests.

## Which files are checked?

With no Flint configuration, the check recursively searches these directories
when they exist:

- `k8s/`
- `kubernetes/`
- `manifests/`

Only `.yaml` and `.yml` files are considered. Flint parses each file and selects
it when at least one YAML document has both top-level `apiVersion` and `kind`
keys. It then passes the whole file to KubeLinter. This avoids treating ordinary
YAML, such as a Compose file or an application's settings, as a Kubernetes
manifest.

To use different locations, add files or directories relative to the repository
root:

```toml
# flint.toml
[checks.kube-linter]
paths = ["deploy/kubernetes", "examples/demo.yaml"]
```

Explicit directories are searched recursively. Explicit `paths` replace the
conventional directories rather than extending them. Paths must stay within the
repository: absolute paths, empty paths, and paths containing `..` are ignored.

## Example configuration

This example checks manifests under `deploy/kubernetes/` and uses a
KubeLinter policy stored in Flint's config directory:

```toml
# flint.toml
[checks.kube-linter]
paths = ["deploy/kubernetes"]
config = "kube-linter.yaml"
```

```yaml
# kube-linter.yaml
checks:
  addAllBuiltIn: true
  exclude:
    - unset-cpu-requirements
    - unset-memory-requirements
```

`config` is relative to `FLINT_CONFIG_DIR`, which defaults to the repository
root. If `config` is omitted, Flint automatically uses `kube-linter.yaml` from
that directory when it exists. Config paths must stay within
`FLINT_CONFIG_DIR`.

Run the check explicitly while setting it up:

```bash
flint run --full kube-linter
```

See the upstream
[KubeLinter configuration guide](https://github.com/stackrox/kube-linter/blob/main/docs/configuring-kubelinter.md)
for built-in checks, per-object exclusions, and custom checks.

## Manifest selection details

- Multi-document YAML is selected when any document has `apiVersion` and
  `kind`; KubeLinter receives the complete file.
- YAML files without those keys are skipped.
- Symlinks are not traversed.
- Missing configured paths produce a clean no-op.
- Helm and Kustomize rendering is not performed by Flint. Render those inputs
  separately when generated manifests need linting.
