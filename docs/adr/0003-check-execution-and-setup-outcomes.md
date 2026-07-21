# ADR 0003: Check execution and setup outcomes

- **Status:** Accepted
- **Date:** 2026-07-21

## Context

Read-only checks benefit from parallel execution, but concurrent fixers can
corrupt or overwrite each other's changes. Flint setup drift also differs from
ordinary lint failures: some setup changes are safe to apply before continuing,
while migrations can change the active check set.

## Decision

- Normal read-only checks run in parallel and collect deterministic, sorted
  output.
- Fixing checks run serially.
- Non-blocking setup convergence, such as canonical tool ordering, is applied
  before continuing with independent fixes.
- Blocking or fatal setup migrations stop the fix run with a clear reason.
- Fix results remain classified as `clean`, `fixed`, `review`, or `partial`.

## Consequences

- Check mode stays fast.
- Fix mode is deterministic and avoids concurrent writes.
- Setup normalization does not prevent unrelated linter fixes.
- Some setup failures intentionally require a second command or manual review.

## Alternatives considered

- Running all fixers in parallel would be faster but unsafe for overlapping
  files.
- Treating every setup warning as blocking would make harmless ordering drift
  prevent useful fixes.
