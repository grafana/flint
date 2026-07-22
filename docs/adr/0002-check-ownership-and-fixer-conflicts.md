# ADR 0002: Check ownership and fixer conflicts

- **Status:** Accepted
- **Date:** 2026-07-21

## Context

Checks often share file extensions without having the same responsibility. For
example, YAML style and Kubernetes policy checks may both inspect a file, while
a formatter and another formatter may both try to rewrite it.

## Decision

Registry metadata distinguishes these relationships:

- **formatter:** owns formatting and may be the explicit deferral target for a
  generic formatting check;
- **generic linter:** validates syntax or general style;
- **semantic linter:** validates domain-specific rules and may complement a
  generic linter;
- **project check:** evaluates repository-wide state rather than individual
  file ownership.

Overlapping read-only checks are allowed when their responsibilities are
documented. Formatter deferral must be explicit in the registry. Two
fix-capable checks must not silently rewrite the same file in an
order-dependent way; they must have disjoint fix ownership, an explicit safe
precedence, or a reported conflict.

## Consequences

- `ryl` and `kube-linter` may both inspect Kubernetes YAML.
- `google-java-format` and a read-only Checkstyle check may both inspect Java.
- `flint run --fix` must preserve deterministic write ordering and conflict
  reporting.
- New registry entries require ownership and fixer-conflict tests.

## Alternatives considered

- A strict one-check-per-file rule would prevent useful complementary checks.
- Implicit last-writer-wins behavior would make fixes fragile and difficult to
  review.
