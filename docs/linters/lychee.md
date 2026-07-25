# [`lychee`](https://lychee.cli.rs/)

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|        |                                      |
| ------ | ------------------------------------ |
| Fix    | no                                   |
| Binary | `lychee`                             |
| Scope  | [native](../linters.md#scope-native) |
| Config | via `[checks.links]` in flint.toml   |

Check for broken links
<!-- linter-metadata-end -->

Orchestrates [lychee](https://lychee.cli.rs/) for link checking. Requires `lychee` in `[tools]`.

Default behavior: checks all links in changed files. In CI, Flint also adds a
full-repository safeguard pass over local links in all files so broken internal
links in unchanged docs still fail the build. Outside that CI safeguard, setting
`check_all_local = true` in `flint.toml` adds the same local-links-only pass
over all files.

Outside CI, flint also enables a local lychee request cache by default to
speed up repeated runs. Flint stores that cache under `.lychee_cache/` and
creates the directory on first use. Set `FLINT_LYCHEE_SKIP_LOCAL_CACHE=true`
to opt out. If your lychee config already sets `cache = true`, flint leaves
caching to lychee instead.

In CI, `lychee` requires `GITHUB_TOKEN` so GitHub link checks can authenticate.
On GitHub Actions PR runs in changed-file mode, link remaps also require
`GITHUB_REPOSITORY`, `GITHUB_BASE_REF`, `GITHUB_HEAD_REF`, and `PR_HEAD_REPO`.
GitHub Actions provides the first three; set `PR_HEAD_REPO` from
`github.event.pull_request.head.repo.full_name`. The CI local-links safeguard
pass and `--full` do not require the PR remap metadata.

Configure via `flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```
