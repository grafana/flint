# Why / Principles

## Why

The bash task scripts (v1) have two problems:

**Local ≠ CI**: `--native` runs a subset of linters; CI runs full super-linter
in Docker. Different tools, different behavior. Passing locally does not mean
passing in CI.

**Bash has limits**: the registry pattern was already at the edge of what bash
does cleanly. Adding built-in checks (links, renovate) would make it worse.

### Why not pre-commit?

pre-commit adds a parallel tool management system on top of mise. Consuming repos
already declare their tools in `mise.toml` — pre-commit would require maintaining
a second inventory of the same tools in `.pre-commit-config.yaml`, with its own
versioning and install lifecycle. That's friction without benefit for repos that
are already mise-first.

### Why not Husky?

Husky manages git hooks for Node.js projects and requires `npm install` to activate.
Repos that aren't Node-first still need a `package.json` and a dev dependency just to
run hooks. `flint hook install` writes a single shell script directly to `.git/hooks/`
with no install step and no language runtime dependency.

### Why not Spotless (or other Maven formatter plugins)?

Spotless runs `google-java-format` as a Maven build phase, which means format
failures block compilation and test runs — that's the wrong place for a style
check. flint's `google-java-format` check runs as a separate lint step, only on
changed files, and is fast.

To migrate: remove `spotless-maven-plugin` from your `pom.xml` (and any
`spotless.skip` properties), add `"github:google/google-java-format"` to
`[tools]` in `mise.toml`, and run `flint run --fix` once to confirm the repo is
clean.

### Why not MegaLinter / super-linter?

Container-based linters (super-linter, MegaLinter) ship their own tool versions,
independent of what the repo pins in `mise.toml`. This breaks the "declare once,
use everywhere" promise of mise. Container startup also adds latency to every run.

## Principles

1. **Fast** — the primary goal; everything else serves it:
   - Native execution only (no Docker); linters run in parallel (Rust binary, short startup)
   - Small binary, cached by mise — fast install, near-zero overhead between runs
   - Diff-aware: only changed files are linted by default; `--full` to check everything
   - Opt-in via `mise.toml`: undeclared tools are skipped entirely
   - Checks can be tagged slow in the registry and skipped via `--fast-only`

2. **Local same as CI** — one binary, one config, identical behavior.
   No "native mode subset" distinction. If it passes locally, it passes in CI.

3. **AI-friendly** — `--fix` fixes what's fixable silently, prints output
   only for issues needing review, and exits with a structured summary:

   ```text
   [shellcheck]
   ...
   flint: fixed: cargo-fmt — commit before pushing | review: shellcheck
   ```

   Only unfixable issues surface for review — no reasoning step required.

4. **Cross-platform** — runs on Linux, macOS, and Windows. The built-in
   registry accounts for platform differences (e.g. binary names, path quoting).

5. **Autofix where possible** — `--fix` checks first, fixes what's fixable,
   reports what needs review. Fix mode runs serially to avoid concurrent writes.
   Pass specific linter names to limit which fixers run (`flint run --fix rumdl shfmt`).
