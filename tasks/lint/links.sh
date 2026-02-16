#!/usr/bin/env bash
#MISE description="Check for broken links in changed files + all local links"

set -euo pipefail

#USAGE flag "--full" help="Check all links (local + remote) in all files"
#USAGE flag "--base <base>" help="base branch to compare against (for modified-files mode)"
#USAGE flag "--head <head>" help="head commit to compare against (for modified-files mode)"
#USAGE flag "--lychee-args <args>" help="extra arguments to pass to lychee"
#USAGE arg "<file>" var=#true help="files to check" default="."

LYCHEE_CONFIG="${LYCHEE_CONFIG:-.github/config/lychee.toml}"

eval "lychee_args=(${usage_lychee_args:-})"

run_lychee() {
	local description="$1"
	shift

	echo "==> $description"
	# shellcheck disable=SC2154 # lychee_args is set via eval above
	lychee --config "$LYCHEE_CONFIG" \
		"${lychee_args[@]+"${lychee_args[@]}"}" \
		"$@"
}

get_modified_files() {
	# shellcheck disable=SC2154 # usage_* vars are set by mise
	local base="${usage_base:-origin/${GITHUB_BASE_REF:-main}}"
	local head="${usage_head:-${GITHUB_HEAD_SHA:-HEAD}}"

	# Using lychee's default extension filter here to match when it runs against all files
	# Note: --diff-filter=d filters out deleted files
	# shellcheck disable=SC2086 # intentional: head may expand to empty
	git diff --name-only --merge-base --diff-filter=d "$base" $head |
		grep -E '\.(md|mkd|mdx|mdown|mdwn|mkdn|mkdown|markdown|html|htm|txt)$' |
		tr '\n' ' ' || true
}

is_config_modified() {
	# shellcheck disable=SC2154 # usage_* vars are set by mise
	local base="${usage_base:-origin/${GITHUB_BASE_REF:-main}}"
	local head="${usage_head:-${GITHUB_HEAD_SHA:-HEAD}}"

	# Pattern for detecting config changes that should trigger a full lint.
	# Consuming repos can override this via LYCHEE_CONFIG_CHANGE_PATTERN.
	local config_change_pattern="${LYCHEE_CONFIG_CHANGE_PATTERN:-^(\.github/config/lychee\.toml|\.mise/tasks/lint/.*|mise\.toml)$}"

	local config_modified
	# shellcheck disable=SC2086 # intentional: head may expand to empty
	config_modified=$(git diff --name-only --merge-base "$base" $head |
		grep -E "$config_change_pattern" || true)

	[ -n "$config_modified" ]
}

# shellcheck disable=SC2154 # usage_full is set by mise
if [ "${usage_full:-}" = "true" ] || is_config_modified; then
	if is_config_modified && [ "${usage_full:-}" != "true" ]; then
		echo "Config changes detected, falling back to full check."
	fi
	# shellcheck disable=SC2154,SC2086 # usage_file is set by mise; intentional word splitting
	run_lychee "Checking all links in all files" -- $usage_file
else
	modified_files=$(get_modified_files)

	if [ -n "$modified_files" ]; then
		# shellcheck disable=SC2086 # intentional word splitting for file list
		run_lychee "Checking all links in modified files" -- $modified_files
	else
		echo "No modified files to check for all links."
	fi

	# shellcheck disable=SC2154,SC2086 # usage_file is set by mise; intentional word splitting
	run_lychee "Checking local links in all files" --scheme file --include-fragments -- $usage_file
fi
