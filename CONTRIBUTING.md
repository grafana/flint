# Contributing to Flint

Pull request titles should follow the
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/#summary) format:

```text
<type>[optional scope]: <description>
```

For example: `feat(format): support Java formatting`. Common types include `feat`, `fix`,
`docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, and `revert`.

## Local development

If you are working on Flint itself, the normal local workflow is:

```bash
cargo test
mise run lint:fix
```

To run a narrower fixture subset while iterating:

```bash
FLINT_CASES=shellcheck/clean cargo test cases
```

If you have Flint checked out locally, prefer running it directly with Cargo:

```bash
cargo run -- run
cargo run -- run --fix
```

## Testing an unreleased branch in a consumer repo

If you do **not** have Flint checked out locally and want to test an unreleased
Flint branch in a consumer repo, pin a cargo git dependency in that repo's
`mise.toml`:

```toml
[tools]
"cargo:https://github.com/grafana/flint" = "rev:<git-ref>"
```

Replace `<git-ref>` with the branch, tag, or commit you want to test.

## Submit a pull request

Effective 2026-06-22, all Grafana Labs repositories [require signed commits][signed-commits].
To learn more about Git commit verification, refer to [About commit signature verification][signing-commits]
and [Checking your commit signature verification status][verifying-commits].

> [!NOTE]
> Pull requests containing any unsigned commits cannot be merged until all commits are signed.

[signed-commits]: https://docs.github.com/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches#require-signed-commits
[signing-commits]: https://docs.github.com/authentication/managing-commit-signature-verification/about-commit-signature-verification
[verifying-commits]: https://docs.github.com/authentication/troubleshooting-commit-signature-verification/checking-your-commit-and-tag-signature-verification-status
