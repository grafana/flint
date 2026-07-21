#!/usr/bin/env bash
set -euo pipefail

# This is intentionally a networked compatibility check. The normal Rust
# registry tests stay offline and validate only Flint's metadata contract.
command -v jq >/dev/null || {
	echo "validate-mise-registry: jq is required" >&2
	exit 1
}
command -v mise >/dev/null || {
	echo "validate-mise-registry: mise is required" >&2
	exit 1
}

failures=0
while IFS=$'\t' read -r name install_key; do
	case "$install_key" in
	cargo:* | npm:* | pipx:* | rust | go | dotnet)
		# These are intentionally non-Aqua backends and are covered by their
		# package-manager/toolchain ecosystems rather than the Aqua registry.
		printf 'skipping non-Aqua backend: %s (%s)\n' "$name" "$install_key"
		;;
	*)
		if ! mise ls-remote "$install_key" >/dev/null 2>&1; then
			printf 'mise registry lookup failed: %s (%s)\n' "$name" "$install_key" >&2
			failures=$((failures + 1))
		fi
		;;
	esac
done < <(
	cargo run -q --bin flint -- linters --json |
		jq -r '.[] | select(.install_key != null and .binary != "(built-in)") |
      [.name, .install_key] | @tsv'
)

if ((failures > 0)); then
	printf 'validate-mise-registry: %d backend lookup(s) failed\n' "$failures" >&2
	exit 1
fi

echo "validate-mise-registry: all curated external backends resolved"
