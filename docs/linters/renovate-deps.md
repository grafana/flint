# `renovate-deps`

`renovate-deps` does three related checks:

1. It verifies that `renovate-tracked-deps.json` next to the active Renovate
   config matches what Renovate currently extracts from the repo.
2. It checks that extracted dependencies which resolve to the same upstream
   package are covered consistently by Renovate package rules.
3. It checks that any extracted `extractVersion` still turns Renovate's
   resolved `currentVersion` into the tracked `currentValue`.

The second and third checks are there to catch configuration mistakes before
they show up as separate Renovate PRs, stalled updates, or README drift.

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

It also catches stale or overly generic `extractVersion` rules. For example,
if Renovate resolves a current upstream tag like `@biomejs/biome@2.4.12` but
the configured `extractVersion` is still `^v?(?<version>.+)`, Flint will flag
that the regex no longer extracts the tracked `currentValue` (`2.4.12`).

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

## Preset compatibility note

`default.json` now describes Flint's current Renovate behavior only. It does
not include the legacy regex managers some v1-era repos used to update
SHA-pinned `raw.githubusercontent.com/.../<sha>/... # vX.Y.Z` references or
`*_VERSION` variables in `mise.toml`. Repos that still need those updates
should keep repo-local custom managers for them when extending the shared
preset.

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

It stores only the stable metadata Flint needs for these checks:

- `files`: extracted dependency names by file and manager
- `meta`: stable package metadata used for rule-coverage validation

Lookup-only fields such as `currentVersion`, `currentValue`, and
`extractVersion` are used transiently during validation and autofix, but are
stripped before Flint compares or writes the committed snapshot. This keeps
`renovate-tracked-deps.json` stable across routine version changes.

## Fixing failures

If the snapshot is stale:

```bash
flint run --fix renovate-deps
```

Verification (plain `flint run`) uses Renovate's cheap `--dry-run=extract`
plus the committed snapshot's metadata. `--fix` regenerates via
`--dry-run=lookup` so meta is authoritative.

When Flint can infer a better `extractVersion` directly from Renovate's
resolved `currentVersion` and `currentValue`, `--fix` also appends a targeted
`packageRules` override for the affected `depName` and retries the lookup.

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
