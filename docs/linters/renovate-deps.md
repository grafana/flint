# `renovate-deps`

`renovate-deps` does two related checks:

1. It verifies that `renovate-tracked-deps.json` next to the active Renovate
   config matches what Renovate currently extracts from the repo.
2. It checks that extracted dependencies which resolve to the same upstream
   package are covered consistently by Renovate package rules.

The second check is there to catch configuration mistakes before they show up as
separate Renovate PRs or README drift.

## When does this run?

CI always runs `renovate-deps`. Locally `flint run` only runs it when the
changed files plausibly affect the snapshot. `--full` or naming the
linter explicitly bypass the skip.

| Change                                        | Local | CI  |
| --------------------------------------------- | ----- | --- |
| Renovate config edited                        | ✅    | ✅  |
| `renovate-tracked-deps.json` snapshot edited  | ✅    | ✅  |
| File already tracked in the snapshot edited   | ✅    | ✅  |
| New tool/action added that is not yet tracked | ❌    | ✅  |
| Unrelated change (docs, source, etc.)         | ❌    | ✅  |

The "new tool not yet tracked" case is the typical reason a CI failure
won't reproduce locally without `--full`.

## What it catches

Goal: `mise.toml` and `README.md` both refer to actionlint, so you want
Renovate to treat them as the same dependency and keep them in the same group.

A setup can fail that goal by extracting different dependency names for the
same upstream package:

```json5
{
  packageRules: [
    {
      groupName: "linters",
      matchDepNames: ["actionlint"],
    },
  ],
  customManagers: [
    {
      customType: "regex",
      managerFilePatterns: ["/^README\\.md$/"],
      datasourceTemplate: "github-releases",
      depNameTemplate: "rhysd/actionlint",
    },
  ],
}
```

Where it fails:

- `mise.toml` extracts `actionlint`
- `README.md` extracts `rhysd/actionlint`
- the `linters` rule matches only `actionlint`

Renovate can now stop grouping those occurrences consistently and update them
separately.

`renovate-deps` reports that mismatch earlier, at config-check time.

## Preferred pattern

When a custom manager needs a different lookup identity than the grouping name,
set both values explicitly:

```json5
{
  customType: "regex",
  datasourceTemplate: "github-releases",
  depNameTemplate: "actionlint",
  packageNameTemplate: "rhysd/actionlint",
}
```

Why:

- `depNameTemplate` controls the extracted dependency name Flint uses for rule
  matching comparisons
- `packageNameTemplate` keeps the datasource lookup pointed at the real upstream
  package

The same pattern applies to entries like:

```json5
depNameTemplate: "github:koalaman/shellcheck",
packageNameTemplate: "koalaman/shellcheck",
```

## Snapshot shape

The committed `renovate-tracked-deps.json` snapshot lives next to the active
Renovate config:

- `.github/renovate-tracked-deps.json` for `.github/renovate.json5`
- `renovate-tracked-deps.json` for root-level configs such as `.renovaterc.json`

It stores only the metadata Flint needs for these checks:

- `files`: extracted dependency names by file and manager
- `meta`: package metadata for deps relevant to rule-coverage validation

This is intentionally narrower than full Renovate output so steady-state
`renovate-deps --fix` stays cheap.

## Fixing failures

If the snapshot is stale:

```bash
flint run --fix renovate-deps
```

Verification (plain `flint run`) uses Renovate's cheap `--dry-run=extract`
plus the committed snapshot's metadata. `--fix` regenerates via
`--dry-run=lookup` so meta is authoritative.

The linter requires every dep referenced by a `packageRule` to have
`packageName`; deps matched via `matchPackageNames` additionally require
`datasource` so Renovate's `(packageName, datasource)` grouping is
deterministic. `matchDepNames` rules don't require datasource — bare-key
mise tools like `biome` don't always surface one even in lookup-mode
output, and Renovate matches them by name regardless.

If rule coverage is inconsistent:

- normalize equivalent deps to one canonical `depNameTemplate`
- keep `packageNameTemplate` explicit when datasource lookup needs a different
  identifier
- make sure the intended `packageRules` matcher covers that canonical dependency name
