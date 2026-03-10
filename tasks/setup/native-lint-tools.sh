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

ENV_NAME="super-linter-${VERSION}"
LOCAL_TOML=".mise.${ENV_NAME}.toml"

if [ -f "$LOCAL_TOML" ]; then
	echo "Native lint tools already set up for super-linter ${VERSION}"
	mise install -E "$ENV_NAME"
	exit 0
fi

# Clean up old versions
rm -f .mise.super-linter-*.toml

FLINT_REPO="${FLINT_REPO:-grafana/flint}"
# Fetching from main is safe: version mappings are keyed by super-linter version
# (e.g. v8.4.0.toml) and their content is stable once committed.
FLINT_REF="${FLINT_REF:-main}"

echo "Fetching tool versions for super-linter ${VERSION}..."
REMOTE_URL="https://raw.githubusercontent.com/${FLINT_REPO}/${FLINT_REF}/super-linter-versions/${VERSION}.toml"

if ! curl -fsSL "$REMOTE_URL" -o "$LOCAL_TOML"; then
	echo "Failed to fetch version mapping from ${REMOTE_URL}"
	echo "No version mapping available for super-linter ${VERSION}."
	rm -f "$LOCAL_TOML"
	exit 1
fi

echo "Installing native lint tools..."
mise trust "$LOCAL_TOML"
mise install -E "$ENV_NAME"

echo "Native lint tools installed for super-linter ${VERSION}"
