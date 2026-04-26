#!/usr/bin/env bash
#MISE description="Create git tags and GitHub releases when a release PR was merged"

set -euo pipefail

if [ -z "${MISE_PROJECT_ROOT:-}" ]; then
	echo "MISE_PROJECT_ROOT environment variable is not set. Exiting."
	exit 1
fi

cd "${MISE_PROJECT_ROOT}"

if [ -z "${GITHUB_TOKEN:-}" ]; then
	echo "GITHUB_TOKEN environment variable is not set. Exiting."
	exit 1
fi

tmp_json="$(mktemp)"
trap 'rm -f "${tmp_json}"' EXIT

release-plz release -o json "$@" >"${tmp_json}"

jq -e '.releases and (.releases | type == "array")' "${tmp_json}" >/dev/null

if ! jq -e '.releases | length > 0' "${tmp_json}" >/dev/null; then
	echo "No releases created."
	exit 0
fi

if [ -z "${GH_TOKEN:-}" ]; then
	export GH_TOKEN="${GITHUB_TOKEN}"
fi

tag="$(jq -r '.releases[0].tag' "${tmp_json}")"

gh workflow run release.yml -f "tag=${tag}"
