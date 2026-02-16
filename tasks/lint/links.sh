#!/usr/bin/env bash
#MISE description="Lint links in files"

set -euo pipefail

#USAGE flag "--all-files" help="Check all files, not just modified ones"
#USAGE flag "--include-remote" help="Also check remote links (default checks local file links only)"
#USAGE flag "--base <base>" help="base branch to compare against (for modified-files mode)"
#USAGE flag "--head <head>" help="head commit to compare against (for modified-files mode)"
#USAGE flag "--autofix" help="Ignored (lychee does not support autofix)"
#USAGE flag "--lychee-args <args>" help="extra arguments to pass to lychee"
#USAGE arg "<file>" var=#true help="files to check" default="."

LYCHEE_CONFIG="${LYCHEE_CONFIG:-.github/config/lychee.toml}"

eval "lychee_args=(${usage_lychee_args:-})"

local_only_args=()
# shellcheck disable=SC2154 # usage_include_remote is set by mise
if [ "${usage_include_remote:-}" != "true" ]; then
	local_only_args=(--scheme file --include-fragments)
fi

run_lychee() {
	# shellcheck disable=SC2086 # intentional word splitting for file list
	# shellcheck disable=SC2154 # lychee_args is set via eval above
	lychee --config "$LYCHEE_CONFIG" \
		"${local_only_args[@]+"${local_only_args[@]}"}" \
		"${lychee_args[@]+"${lychee_args[@]}"}" \
		-- $1
}

# shellcheck disable=SC2154 # usage_all_files is set by mise
if [ "${usage_all_files:-}" = "true" ]; then
	# shellcheck disable=SC2154,SC2086 # usage_file is set by mise; intentional word splitting
	run_lychee "$usage_file"
else
	# shellcheck disable=SC2154 # usage_* vars are set by mise
	base="${usage_base:-origin/${GITHUB_BASE_REF:-main}}"
	head="${usage_head:-${GITHUB_HEAD_SHA:-HEAD}}"

	# Pattern for detecting config changes that should trigger a full lint.
	# Consuming repos can override this via LYCHEE_CONFIG_CHANGE_PATTERN.
	config_change_pattern="${LYCHEE_CONFIG_CHANGE_PATTERN:-^(\.github/config/lychee\.toml|\.mise/tasks/lint/.*|mise\.toml)$}"

	# Check if lychee config was modified
	# shellcheck disable=SC2086 # intentional: head may expand to empty
	config_modified=$(git diff --name-only --merge-base "$base" $head |
		grep -E "$config_change_pattern" || true)

	if [ -n "$config_modified" ]; then
		echo "config changes, checking all files."
		run_lychee .
	else
		# Using lychee's default extension filter here to match when it runs against all files
		# Note: --diff-filter=d filters out deleted files
		# shellcheck disable=SC2086 # intentional: head may expand to empty
		modified_files=$(git diff --name-only --merge-base --diff-filter=d "$base" $head |
			grep -E '\.(md|mkd|mdx|mdown|mdwn|mkdn|mkdown|markdown|html|htm|txt)$' |
			tr '\n' ' ' || true)

		if [ -z "$modified_files" ]; then
			echo "No modified files, skipping link linting."
			exit 0
		fi

		run_lychee "$modified_files"
	fi
fi
