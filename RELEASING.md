# Releasing

Releases are automated via
[Release Please](https://github.com/googleapis/release-please).
When conventional commits land on `main`, Release Please opens
(or updates) a release PR with a changelog.

> **Note:** CI checks don't trigger automatically on release-please
> PRs because they are created with `GITHUB_TOKEN`. To run CI,
> either click **Update branch** or **close and reopen** the PR.

## Post-release: regenerate version mapping

After merging a release that bumps `SUPER_LINTER_VERSION`,
regenerate the native lint tool version mapping:

```bash
mise run setup:update-super-linter-versions
git add super-linter-versions/
git commit -m "chore: regenerate super-linter version mapping"
```

<!-- TODO: automate this via Renovate postUpgradeTasks once
     grafana/grafana-renovate-config supports `mise run` commands
     (see https://github.com/grafana/grafana-renovate-config/pull/65) -->
