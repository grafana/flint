# ADR 0005: Changed-file relevance and baselines

- **Status:** Accepted
- **Date:** 2026-07-21

## Context

Flint's local default should be fast and diff-aware. CI activates the complete
active check set, including adaptive checks, while retaining changed-file
scoping where each check supports it. Explicit `--full` runs provide complete
file coverage. Some changes affect the meaning of all files, and deleted paths
can still affect a check's relevance.

## Decision

Flint keeps two related inputs:

- the runnable file list, containing existing tracked files that can be passed
  to external tools;
- the changed-path list, retaining created, modified, renamed, and deleted
  paths for relevance and baseline decisions.

A check expands to a full baseline when its activation, version, supported
config, Flint-managed settings, or other declared baseline trigger changes.
Adaptive checks may skip local runs only when their relevance hook says the
changed paths cannot affect the result. CI includes those checks, but is not
equivalent to `--full`: file coverage remains diff-aware where supported.

## Consequences

- Deleting a file can correctly trigger a check without passing a missing path
  to the external tool.
- New checks need explicit relevance and baseline behavior.
- Relevance logic must be tested separately for added, modified, renamed, and
  deleted paths.

## Alternatives considered

- Using only existing runnable files loses deletion-only changes.
- Always running every check locally would preserve coverage but violate
  Flint's fast inner-loop goal.
