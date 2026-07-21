# mise/Aqua compatibility

Flint's built-in registry is intentionally curated and compiled into the
binary. The Rust registry tests validate metadata and tool-key structure
without network access.

The networked `.mise/tasks/validate-mise-registry` task separately resolves
every Aqua, GitHub-release, and Ubi backend through the current mise registry.
It skips explicit non-Aqua exceptions (cargo, npm, pipx, and language
toolchains). A new backend must either resolve successfully or be added to that
small, documented exception list with a reason.

Run it locally with:

    mise run validate-mise-registry
