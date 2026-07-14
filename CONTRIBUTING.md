# Contributing to Flint

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

If you do **not** have Flint checked out locally and want to test a branch in a
consumer repo, pin a cargo git dependency in that repo's `mise.toml`:

```toml
[tools]
"cargo:https://github.com/trask/flint" = { version = "branch:fix-lychee-windows-arg-limit", crate = "flint", bin = "flint" }
```

That `trask/flint` example is just a fork used for branch testing; use your own
fork/branch as needed.
