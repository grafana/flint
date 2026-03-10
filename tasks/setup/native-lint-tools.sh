#!/usr/bin/env bash
#MISE description="Install native lint tools matching the pinned super-linter version"

set -euo pipefail

if [ -z "${MISE_PROJECT_ROOT:-}" ]; then
	echo "MISE_PROJECT_ROOT environment variable is not set. Exiting."
	exit 1
fi

cd "${MISE_PROJECT_ROOT}"

if [ -z "${SUPER_LINTER_VERSION:-}" ]; then
	echo "SUPER_LINTER_VERSION environment variable is not set. Exiting."
	exit 1
fi

# Extract clean version (strip slim- prefix and @sha256 digest)
VERSION="${SUPER_LINTER_VERSION#slim-}"
VERSION="${VERSION%%@*}"

FLINT_REPO="${FLINT_REPO:-grafana/flint}"

# Derive FLINT_REF from the flint SHA pinned in mise.toml task URLs.
# Consuming repos pin flint tasks to a specific commit SHA, e.g.:
#   file = "https://raw.githubusercontent.com/grafana/flint/<sha>/tasks/..."
# This ensures the version mapping matches the flint version in use.
# Falls back to "main" for flint itself (where tasks are local file paths).
if [ -z "${FLINT_REF:-}" ]; then
	FLINT_REF=$(grep -oP \
		"raw\\.githubusercontent\\.com/${FLINT_REPO}/\\K[a-f0-9]{40}" \
		mise.toml 2>/dev/null | head -1 || true)
	FLINT_REF="${FLINT_REF:-main}"
fi

ENV_NAME="super-linter-${VERSION}"
LOCAL_TOML=".mise.${ENV_NAME}.toml"

if [ -f "$LOCAL_TOML" ]; then
	echo "Native lint tools already set up for super-linter ${VERSION}"
	mise install -E "$ENV_NAME"
	exit 0
fi

# Clean up old versions
rm -f .mise.super-linter-*.toml

VERSION_FILE="super-linter-versions/${VERSION}.toml"

# Use local file if available (flint itself), otherwise fetch from GitHub
if [ -f "$VERSION_FILE" ]; then
	cp "$VERSION_FILE" "$LOCAL_TOML"
else
	echo "Fetching tool versions for super-linter ${VERSION}..."
	REMOTE_URL="https://raw.githubusercontent.com/${FLINT_REPO}/${FLINT_REF}/${VERSION_FILE}"

	if ! curl -fsSL "$REMOTE_URL" -o "$LOCAL_TOML"; then
		echo "Failed to fetch version mapping from ${REMOTE_URL}"
		echo "No version mapping available for super-linter ${VERSION}."
		rm -f "$LOCAL_TOML"
		exit 1
	fi
fi

echo "Installing native lint tools..."
mise trust "$LOCAL_TOML"
mise install -E "$ENV_NAME"

echo "Native lint tools installed for super-linter ${VERSION}"
