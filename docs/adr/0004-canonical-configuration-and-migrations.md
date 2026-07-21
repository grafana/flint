# ADR 0004: Canonical configuration and migrations

- **Status:** Accepted
- **Date:** 2026-07-21

## Context

Tool auto-discovery creates local/CI drift and makes migrations difficult to
reason about. Flint needs a predictable configuration layout while preserving
intentional repository-specific settings.

## Decision

- Each supported check has one canonical config filename and documented config
  location.
- `FLINT_CONFIG_DIR` controls the shared Flint-managed directory where the
  check supports it; documented root-level exceptions remain explicit.
- Repo-wide excludes belong in `flint.toml`; tool-specific rules belong in the
  tool's own config.
- Setup migrations are versioned and idempotent.
- `flint-setup` applies actionable Flint-owned migrations; `flint init` is the
  explicit full-convergence path.
- Migrations preserve meaningful reviewer-facing comments where possible and
  report when manual review is needed.

## Consequences

- Flint fails clearly instead of silently using an unsupported alternate config.
- Repeated setup runs should produce no unrelated churn.
- Migration code must use targeted edits when full-file regeneration would lose
  important context.
- Existing repository-specific configuration may require an explicit migration
  rather than implicit discovery.

## Alternatives considered

- Supporting every upstream discovery filename would make local/CI behavior
  difficult to predict.
- Rewriting all config files would be simpler but would discard useful comments
  and reviewer context.
