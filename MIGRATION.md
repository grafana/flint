# Migration Guide

## Replacing `markdownlint-cli` with `markdownlint-cli2`

`markdownlint-cli2` is the actively maintained successor to `markdownlint-cli`.
It is faster, supports more configuration options, and is the direction the
markdownlint ecosystem is moving. flint only supports `markdownlint-cli2`.

**Before** (`mise.toml`):

```toml
"npm:markdownlint-cli" = "0.47.0"
```

**After**:

```toml
"npm:markdownlint-cli2" = "0.17.2"
```

Configuration files remain compatible — both tools read `.markdownlint.json`
(and `.markdownlint.yaml`, `.markdownlint.jsonc`). No changes to your config
file are required.

The fix command changes from `markdownlint --fix` to `markdownlint-cli2 --fix`,
but flint handles this automatically.
