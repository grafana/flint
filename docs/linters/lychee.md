# `lychee`

<!-- linter-metadata-start -->
<!-- Generated. Run `mise run generate` to regenerate. -->

|         |                                      |
| ------- | ------------------------------------ |
| Project | [lychee](https://lychee.cli.rs/)     |
| Fix     | no                                   |
| Binary  | `lychee`                             |
| Scope   | [native](../linters.md#scope-native) |
| Config  | via `[checks.links]` in flint.toml   |

<!-- linter-metadata-end -->

`lychee` checks source and documentation files for broken links. Flint
uses the upstream checker but adds Git-aware scoping, CI safeguards, GitHub URL
remapping, and local caching. The `lychee` binary is still required in
`[tools]`.

## What Flint adds

Flint does not replace lychee's link-checking engine. It orchestrates lychee to
make changed-file runs reliable locally and in CI:

- **Git-aware scope:** a normal run checks every link in changed,
  link-checkable files. A config change or `--full` triggers a full-repository
  check.
- **Repository-wide local-link safeguard:** CI also checks local links and
  fragments across all files, catching links from unchanged documents to files
  changed or removed by the PR. Set `check_all_local = true` to add the same
  safeguard outside CI.
- **PR-aware GitHub remapping:** Flint remaps GitHub blob, tree, line, and text
  fragment URLs so links are checked against the current branch or fork content
  rather than only the base branch.
- **Managed local cache:** outside CI, Flint enables lychee's request cache in
  `.lychee_cache/` by default. It leaves caching to lychee when the selected
  lychee config already sets `cache = true`.
- **CI authentication guardrails:** Flint requires `GITHUB_TOKEN` in CI and
  reports missing PR metadata before running link remaps, instead of allowing
  unauthenticated GitHub requests to fail ambiguously.

## Configuration

Select the upstream lychee config and optional local-link safeguard in
`flint.toml`:

```toml
[checks.links]
config = ".github/config/lychee.toml"
check_all_local = true
```

Set `FLINT_LYCHEE_SKIP_LOCAL_CACHE=true` to disable Flint's local cache. The
global opt-out is:

```bash
mise set --global FLINT_LYCHEE_SKIP_LOCAL_CACHE=true
```

## CI environment

In CI, `lychee` requires `GITHUB_TOKEN` so GitHub link checks can authenticate.
On GitHub Actions PR runs in changed-file mode, link remaps also require
`GITHUB_REPOSITORY`, `GITHUB_BASE_REF`, `GITHUB_HEAD_REF`, and `PR_HEAD_REPO`.
GitHub Actions provides the first three; set `PR_HEAD_REPO` from
`github.event.pull_request.head.repo.full_name`. The CI local-links safeguard
pass and `--full` do not require the PR remap metadata.
