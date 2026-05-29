# How Flint runs checks

This page explains Flint's execution model: how checks become active, what kind
of checks Flint supports, and where Flint stops.

## Mental model

Flint has a **built-in check registry** compiled into the binary.

Each check in that registry describes:

- its name
- how it becomes active
- which files it cares about
- whether it supports fix mode
- how Flint should run it

At runtime, Flint combines that built-in registry with the consuming repo's:

- `mise.toml`
- `flint.toml`
- tracked files and changed-file set
- environment (for example local vs CI)

Flint is **not** a generic per-repo command runner or plugin loader. That is an
intentional boundary, not a temporary limitation: `mise` already provides the
generic task-runner layer, while Flint focuses on curated lint and check
orchestration. The extension point is the built-in registry plus repo
configuration, not arbitrary commands declared in `flint.toml`.

## How a check becomes active

Most checks are **opt-in via `mise.toml`**.

If a repo declares the tool a check expects, Flint considers that check active.
If the tool is not declared, Flint skips the check.

Examples:

- declare `shellcheck` in `mise.toml` -> `shellcheck` becomes active
- declare `aqua:owenlamont/ryl` -> `ryl` becomes active

This is why Flint can stay quiet and fast: the repo chooses the toolchain, and
Flint only runs checks the repo has explicitly installed.

Some checks are exceptions:

- **native config-driven checks** may be active based on config rather than an
  external binary alone
- **setup checks** such as `flint-setup` are special-purpose and may run even
  though they are not ordinary linter binaries

## Two execution models

Flint supports two main kinds of checks.

## Template checks

Template checks run an external command from a command template.

Typical examples:

- `shellcheck`
- `shfmt`
- `rumdl`
- `zizmor`

Template checks declare:

- a command for check mode
- optionally a command for fix mode
- a scope:
  - **file**: one invocation per file
  - **files**: one invocation with a file list
  - **project**: one invocation with no file arguments

This is the simplest model when a tool is already a good CLI and Flint mainly
needs to handle:

- file selection
- config injection
- quiet output
- fix-mode orchestration

## Native checks

Native checks are **implemented in-process** inside Flint.

Typical examples today:

- `renovate-deps`
- `lychee`
- `license-header`
- `flint-setup`

Native checks can still invoke external binaries, but they are not limited to a
single command template. They can do custom preparation and orchestration first.

Use this model when the check needs behavior such as:

- custom orchestration logic
- richer config handling
- multiple execution phases
- custom relevance/baseline logic
- fix behavior that is more nuanced than one command string

In other words:

- **template** = Flint runs a command
- **native** = Flint runs custom Rust logic, which may itself run commands

## Fix mode

Flint has one `--fix` surface, but checks participate in it in two different
ways:

- template checks provide a fix command
- native checks implement fix behavior in code

Internally, Flint distinguishes outcomes such as:

- **clean**
- **fixed**
- **review**
- **partial**

That lets Flint present one consistent user-facing `--fix` workflow even when
the underlying tools have different capabilities.

## What Flint is good at

Flint works best as:

- a fast runner over a curated set of checks
- a consistent local + CI surface
- a changed-file-aware orchestrator
- a unified fix entrypoint

## What Flint is not trying to be

Flint is not intended to be:

- a generic "run whatever commands the repo config says" wrapper
- a plugin system that loads arbitrary repo-local check implementations
- a replacement for every tool's own CLI

That boundary is deliberate: Flint owns the execution model and built-in
registry, while `mise` provides the generic task-runner layer and repos opt
into Flint checks by declaring the toolchain and config they want.

## Implication for new checks

When deciding how to add a new check, use this rule of thumb:

- choose a **template check** when the tool already has a clean CLI and Flint
  mostly needs to handle scope, config, and fix wiring
- choose a **native check** when the tool needs custom orchestration, custom
  state handling, or in-process logic

If the real logic lives elsewhere, the key question is not "can Flint call an
external thing?" — it can — but "does this belong as a template command or as a
native check with custom orchestration?"
