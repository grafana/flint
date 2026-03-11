# Releasing

Releases are automated via
[Release Please](https://github.com/googleapis/release-please).
When conventional commits land on `main`, Release Please opens
(or updates) a release PR with a changelog.

> **Note:** CI checks don't trigger automatically on release-please
> PRs because they are created with `GITHUB_TOKEN`. To run CI,
> either click **Update branch** or **close and reopen** the PR.

## Version mapping regeneration

When Renovate bumps `SUPER_LINTER_VERSION` in `mise.toml`, the
`generate-super-linter-versions` workflow automatically regenerates
the native lint tool version mapping and commits it to the same
Renovate branch.
