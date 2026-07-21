# ADR 0001: Flint is a curated registry and orchestrator

- **Status:** Accepted
- **Date:** 2026-07-21

## Context

Flint needs to provide fast, consistent local and CI linting without becoming
another generic task runner. Repositories already have `mise` for installing
tools and composing arbitrary tasks.

## Decision

Flint uses a built-in, compiled registry of curated checks. The registry owns
the check's execution model, file selection, configuration integration, fix
behavior, setup hooks, and documentation metadata.

Flint is not a generic command wrapper, a repo-local plugin loader, or a
replacement for a build system. New built-in checks must be implemented in the
Flint codebase and reviewed as part of the registry.

## Consequences

- Every check can have consistent local/CI behavior and cross-platform handling.
- Registry entries need complete metadata and tests.
- A repository cannot add an arbitrary Flint check only through `flint.toml`.
- Tools that require build-graph or compiler-plugin integration may need a new
  explicit check abstraction instead of a normal template entry.

## Alternatives considered

- A generic command list in `flint.toml` would duplicate `mise` and weaken
  Flint's consistency guarantees.
- A plugin system would add versioning, security, and cross-platform complexity
  that is outside Flint's current scope.
