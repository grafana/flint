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

# Build --remap args to redirect base-branch GitHub blob URLs to
# raw.githubusercontent.com on the PR branch. This avoids GitHub's
# API rate limiting (429) and ensures links like /blob/main/README.md
# resolve on the PR branch.
#
# Blob URLs are remapped to raw.githubusercontent.com which serves
# files without rate limiting and lets lychee verify fragments against
# raw content (workaround for lycheeverse/lychee#1729).
#
# Tree URLs stay on github.com (raw doesn't serve directories).
#
# Lychee uses first-match-wins for remaps, so order matters:
#   1. Line-number anchors      → strip fragment, remap to raw
#   2. Scroll to Text Fragments → strip fragment, remap to raw
#   3. All other /blob/ URLs    → remap to raw
#   4. /tree/ line anchors      → strip fragment, remap to head branch
#   5. /tree/ URLs              → remap to head branch
#
# Set LYCHEE_SKIP_GITHUB_REMAPS=true to skip the GitHub-specific remaps
# emitted by this function (escape hatch if they cause unexpected behavior;
# does not affect remaps defined in the lychee config file).
build_remap_args() {
	[ "${LYCHEE_SKIP_GITHUB_REMAPS:-}" != "true" ] || return 0

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
	local raw_head="https://raw.githubusercontent.com/${head_repo}/${head_ref}"

	# /blob/ URLs — remap to raw.githubusercontent.com on the head branch.
	# Lychee applies remaps in a single pass (no chaining), so we must
	# target raw directly here rather than relying on the global rules.

	# 1. Line-number anchors (#L123, #L10-L20): strip fragment, remap to raw
	echo "--remap"
	echo "^${base_url}/blob/${base_ref}/(.*?)#L[0-9]+.*\$ ${raw_head}/\$1"

	# 2. Scroll to Text Fragment anchors (#:~:text=...): strip fragment, remap to raw
	echo "--remap"
	echo "^${base_url}/blob/${base_ref}/(.*?)#:~:text=.*\$ ${raw_head}/\$1"

	# 3. All other /blob/ URLs (with or without fragments): remap to raw
	echo "--remap"
	echo "^${base_url}/blob/${base_ref}/(.*)\$ ${raw_head}/\$1"

	# /tree/ URLs — can't use raw (no directory listing), keep on github.com
	local head_url="https://github.com/${head_repo}"

	# 1. Line-number anchors: strip fragment, remap to head branch
	echo "--remap"
	echo "^${base_url}/tree/${base_ref}/(.*?)#L[0-9]+.*\$ ${head_url}/tree/${head_ref}/\$1"

	# 2. Non-fragment URLs: branch-remap only
	echo "--remap"
	echo "^${base_url}/tree/${base_ref}/(.*)\$ ${head_url}/tree/${head_ref}/\$1"
}

# Build global --remap args for GitHub URLs.
#
# Blob URLs are remapped to raw.githubusercontent.com to avoid
# GitHub's API rate limiting (429). raw.githubusercontent.com
# serves files without rate limiting and returns 404 for missing
# files, so link validity is still verified.
#
# Fragment handling (first-match-wins, so order matters):
#   - Line-number anchors (#L123, #L10-L20): JS-rendered, strip
#     and remap to raw
#   - Scroll to Text Fragment (#:~:text=...): browser-only, strip
#     and remap to raw
#   - Other fragments (#section): keep fragment, remap to raw
#     (lychee can verify fragments in raw content)
#   - No fragment: remap to raw
#
# Issue/PR comment anchors are stripped separately (these can't
# be remapped to raw).
#
# We use --remap (not --exclude) because CLI --exclude overrides
# config file excludes in lychee, rather than merging with them.
#
# Set LYCHEE_SKIP_GITHUB_REMAPS=true to skip these (same escape hatch
# as for the repo-specific remaps above).
build_global_github_args() {
	[ "${LYCHEE_SKIP_GITHUB_REMAPS:-}" != "true" ] || return 0

	# /blob/ URLs → raw.githubusercontent.com (avoids GitHub rate limiting)

	# 1. Line-number anchors (#L123, #L10-L20): strip fragment, remap to raw
	echo "--remap"
	# shellcheck disable=SC2016 # single quotes are intentional: these are regex capture groups, not shell vars
	echo '^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#L[0-9]+.*$ https://raw.githubusercontent.com/$1/$2/$3'

	# 2. Scroll to Text Fragment anchors: strip fragment, remap to raw
	echo "--remap"
	# shellcheck disable=SC2016 # single quotes are intentional
	echo '^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*?)#:~:text=.*$ https://raw.githubusercontent.com/$1/$2/$3'

	# 3. Other fragments (#section): keep fragment, remap to raw
	echo "--remap"
	# shellcheck disable=SC2016 # single quotes are intentional
	echo '^https://github.com/([^/]+/[^/]+)/blob/([^/]+)/(.*)$ https://raw.githubusercontent.com/$1/$2/$3'

	# 4. No fragment: remap to raw (caught by rule 3 above, but kept
	#    explicit for clarity — lychee uses first-match-wins)

	# Issue/PR comment anchors (JS-rendered, can't use raw for these).
	# Strip the fragment so the issue/PR page itself is still checked.
	echo "--remap"
	# shellcheck disable=SC2016 # single quotes are intentional
	echo '^https://github.com/([^/]+/[^/]+)/(issues|pull)/([0-9]+)#issuecomment-.*$ https://github.com/$1/$2/$3'
}

run_lychee() {
	local description="$1"
	shift

	local extra_args=()
	while IFS= read -r line; do
		[ -n "$line" ] && extra_args+=("$line")
	done < <(build_remap_args)
	while IFS= read -r line; do
		[ -n "$line" ] && extra_args+=("$line")
	done < <(build_global_github_args)

	echo "==> $description"
	# shellcheck disable=SC2154 # lychee_args is set via eval above
	lychee --config "$LYCHEE_CONFIG" \
		"${lychee_args[@]+"${lychee_args[@]}"}" \
		"${extra_args[@]+"${extra_args[@]}"}" \
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
	# Note: mise.toml is handled separately below to avoid false positives
	# from unrelated tool version bumps.
	local default_pattern='^(\.github/config/lychee\.toml|\.mise/tasks/lint/.*)$'
	local config_change_pattern="${LYCHEE_CONFIG_CHANGE_PATTERN:-$default_pattern}"

	local config_modified
	# shellcheck disable=SC2086 # intentional: head may expand to empty
	config_modified=$(git diff --name-only --merge-base "$base" $head |
		grep -E "$config_change_pattern" || true)

	if [ -n "$config_modified" ]; then
		return 0
	fi

	# Skip the mise.toml content check when the consumer overrides
	# LYCHEE_CONFIG_CHANGE_PATTERN — they've taken full control.
	if [ -n "${LYCHEE_CONFIG_CHANGE_PATTERN:-}" ]; then
		return 1
	fi

	# For mise.toml, only trigger on lychee-related changes (version or task config),
	# not on unrelated tool version bumps.
	local lychee_changes
	# shellcheck disable=SC2086 # intentional: head may expand to empty
	lychee_changes=$(git diff --merge-base "$base" $head -- mise.toml |
		grep -iE '^\+.*lychee|^-.*lychee' || true)

	[ -n "$lychee_changes" ]
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
