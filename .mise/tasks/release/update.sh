#!/usr/bin/env bash
#MISE description="Update release-plz state locally"

set -euo pipefail

if [ -z "${MISE_PROJECT_ROOT:-}" ]; then
	echo "MISE_PROJECT_ROOT environment variable is not set. Exiting."
	exit 1
fi

cd "${MISE_PROJECT_ROOT}"

release-plz update
mise run release:docs-sync
