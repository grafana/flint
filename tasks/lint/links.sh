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

# Build --remap args to redirect base-branch GitHub URLs to the PR branch.
# This ensures links like /blob/main/README.md resolve on the PR branch.
build_remap_args() {
	local repo base_ref head_ref head_repo

	# Resolve repo name
	if [ -n "${GITHUB_REPOSITORY:-}" ]; then
		repo="$GITHUB_REPOSITORY"
	else
		local remote_url
		remote_url=$(git config --get remote.origin.url 2>/dev/null) || return 0
		# Extract owner/repo from HTTPS or SSH URLs
		repo=$(echo "$remote_url" | sed -n 's|.*github\.com[:/]\(.*\)\.git$|\1|p; s|.*github\.com[:/]\(.*\)$|\1|p' | head -1)
		[ -n "$repo" ] || return 0
	fi

	# Resolve base branch
	if [ -n "${GITHUB_BASE_REF:-}" ]; then
		base_ref="$GITHUB_BASE_REF"
	else
		base_ref=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's|.*/||') || true
		base_ref="${base_ref:-main}"
	fi

	# Resolve head branch
	if [ -n "${GITHUB_HEAD_REF:-}" ]; then
		head_ref="$GITHUB_HEAD_REF"
	else
		head_ref=$(git rev-parse --abbrev-ref HEAD 2>/dev/null) || return 0
	fi

	# Skip if on the base branch (no point remapping main → main)
	[ "$head_ref" != "$base_ref" ] || return 0

	# Resolve head repo (for forks; only matters in CI)
	head_repo="${PR_HEAD_REPO:-$repo}"

	local base_url="https://github.com/${repo}"
	local head_url="https://github.com/${head_repo}"

	echo "--remap"
	echo "^${base_url}/blob/${base_ref}/(.*)$ ${head_url}/blob/${head_ref}/\$1"
	echo "--remap"
	echo "^${base_url}/tree/${base_ref}/(.*)$ ${head_url}/tree/${head_ref}/\$1"
}

run_lychee() {
	local description="$1"
	shift

	local remap_args=()
	while IFS= read -r line; do
		[ -n "$line" ] && remap_args+=("$line")
	done < <(build_remap_args)

	echo "==> $description"
	# shellcheck disable=SC2154 # lychee_args is set via eval above
	lychee --config "$LYCHEE_CONFIG" \
		"${lychee_args[@]+"${lychee_args[@]}"}" \
		"${remap_args[@]+"${remap_args[@]}"}" \
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
	local default_pattern='^(\.github/config/lychee\.toml|\.mise/tasks/lint/.*|mise\.toml)$'
	local config_change_pattern="${LYCHEE_CONFIG_CHANGE_PATTERN:-$default_pattern}"

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
