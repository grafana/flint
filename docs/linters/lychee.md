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

## Local vs CI

CI [activates the full linter set](../cli.md#adaptive-runs), but still keeps
lychee's file coverage diff-aware. Flint adds a second, local-links-only pass in
CI rather than checking every remote link in the repository on every PR.

| Behavior                                   | Local default                         | CI changed-file run                   |
| ------------------------------------------ | ------------------------------------- | ------------------------------------- |
| All links in changed, link-checkable files | ✅                                    | ✅                                    |
| Local links and fragments across all files | only with `check_all_local = true`    | ✅                                    |
| Remote links in unchanged files            | only with `--full` or a config change | only with `--full` or a config change |
| GitHub URL and PR-branch remapping         | ✅ when Git context is available      | ✅ with required PR metadata          |
| Flint-managed request cache                | ✅ unless disabled or configured      | ❌                                    |
| Missing `GITHUB_TOKEN`                     | warning                               | error                                 |

### GitHub URL remapping

Vanilla lychee requests a GitHub URL exactly as written. In a PR, a link such
as:

```text
https://github.com/acme/widget/blob/main/docs/setup.md
```

still reads `docs/setup.md` from `main`, not from the commit being checked.
Most importantly, a PR that adds both this link and `docs/setup.md` gets a
false failure because the new file does not exist on `main` yet. The reverse
can also happen: checking the old file on `main` can hide a target renamed or
removed by the PR.

Flint passes `--remap` rules to lychee so repository links follow the content
under test:

| Link written against the base repository | Same-repository PR                 | Fork PR                              |
| ---------------------------------------- | ---------------------------------- | ------------------------------------ |
| GitHub `blob` URL                        | file in the local checkout         | raw file from the fork's head branch |
| GitHub `tree` URL                        | directory in the local checkout    | tree on the fork's head branch       |
| GitHub line or text-fragment URL         | underlying file under test         | underlying raw file on the fork      |

Flint also converts ordinary GitHub `blob` links to raw-content URLs and
normalizes issue or pull-request comment links to their parent issue or PR.
These are lychee command-line remaps only; Flint does not rewrite repository
files.

Set `LYCHEE_SKIP_GITHUB_REMAPS=true` to disable these rules and use vanilla
lychee URL behavior.

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
