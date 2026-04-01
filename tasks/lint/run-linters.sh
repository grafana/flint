#!/usr/bin/env bash
#MISE description="Run native linters with changed-file detection"

set -euo pipefail

#USAGE flag "--autofix" help="Auto-fix issues instead of checking (uses fix command when defined)"
#USAGE flag "--full" help="Lint all files instead of only changed files"
#USAGE arg "[<linter>...]" help="Linters to run (e.g. prettier markdownlint shfmt)"

# Support both direct invocation (parse flags from $@) and mise invocation (usage_* vars).
AUTOFIX="${AUTOFIX:-false}"
LINT_ALL=false
_TOOLS=()

while [[ $# -gt 0 ]]; do
	case "$1" in
	--autofix) AUTOFIX=true && shift ;;
	--full) LINT_ALL=true && shift ;;
	--) shift && _TOOLS+=("$@") && break ;;
	*) _TOOLS+=("$@") && break ;;
	esac
done

[ "${usage_autofix:-}" = "true" ] && AUTOFIX=true
[ "${usage_full:-}" = "true" ] && LINT_ALL=true

# Allow callers to specify tools via env var (useful for mise file tasks where
# positional args can't be passed from a depends list).
if [ ${#_TOOLS[@]} -eq 0 ] && [ -n "${RUN_LINTERS_TOOLS:-}" ]; then
	read -ra _TOOLS <<<"$RUN_LINTERS_TOOLS"
fi

if [ -z "${MISE_PROJECT_ROOT:-}" ]; then
	echo "MISE_PROJECT_ROOT environment variable is not set. Exiting."
	exit 1
fi

cd "${MISE_PROJECT_ROOT}"

# --- Registry ---
# Format: check_cmd|fix_cmd|file_patterns
# Placeholders: {FILE} (per-file), {FILES} (all at once), {MERGE_BASE}, SELF (no file args)
declare -A _CHECK _FIX _PATTERNS

_register() {
	_CHECK["$1"]="$2"
	_FIX["$1"]="$3"
	_PATTERNS["$1"]="$4"
}

_register shellcheck "shellcheck {FILE}" "" "*.sh *.bash *.bats"
_register shfmt "shfmt -d {FILE}" "shfmt -w {FILE}" "*.sh *.bash"
_register markdownlint "markdownlint {FILE}" "markdownlint --fix {FILE}" "*.md"
_register prettier "prettier --check {FILES}" "prettier --write {FILES}" "*.md *.json *.yml *.yaml"
_register actionlint "actionlint {FILE}" "" ".github/workflows/*.yml .github/workflows/*.yaml"
_register hadolint "hadolint {FILE}" "" "Dockerfile Dockerfile.* *.dockerfile"
_register codespell "codespell {FILES}" "codespell --write-changes {FILES}" "*"
_register ec "ec {FILES}" "" "*"
_register golangci-lint "golangci-lint run --new-from-rev={MERGE_BASE}" "" "SELF"
_register ruff "ruff check {FILE}" "ruff check --fix {FILE}" "*.py"
_register ruff-format "ruff format --check {FILE}" "ruff format {FILE}" "*.py"
_register biome "biome check {FILE}" "biome check --fix {FILE}" "*.json *.jsonc *.js *.ts *.jsx *.tsx"
_register biome-format "biome format {FILE}" "biome format --write {FILE}" "*.json *.jsonc *.js *.ts *.jsx *.tsx"

# Allow callers to extend the registry (useful for testing and custom linters).
# The file is sourced after the built-in entries and may call _register freely.
if [ -n "${RUN_LINTERS_EXTRA_REGISTRY:-}" ]; then
	# shellcheck source=/dev/null
	source "$RUN_LINTERS_EXTRA_REGISTRY"
fi

# --- File detection ---

_filter_files() {
	if [ -n "${FILTER_REGEX_EXCLUDE:-}" ]; then
		grep -vE "$FILTER_REGEX_EXCLUDE" || true
	else
		cat
	fi
}

_BASE_BRANCH="${DEFAULT_BRANCH:-main}"
_MERGE_BASE=$(git merge-base "origin/${_BASE_BRANCH}" HEAD 2>/dev/null || echo "")

_list_files() {
	if [ "$LINT_ALL" = "true" ]; then
		git ls-files
	elif [ -n "$_MERGE_BASE" ]; then
		# Files changed in the PR (committed) + uncommitted changes (staged and unstaged)
		{
			git diff --name-only --diff-filter=d "$_MERGE_BASE"...HEAD
			git diff --name-only --diff-filter=d
			git diff --cached --name-only --diff-filter=d
		} | sort -u
	else
		# No merge base found (e.g. shallow clone), fall back to all files
		git ls-files
	fi
}

# Cache the file list once (avoids re-running git commands per linter).
# Filter to files that exist on disk — excludes uncommitted deletions/renames.
mapfile -t _CACHED_FILES < <(
	_list_files | _filter_files | while IFS= read -r f; do
		[ -f "$f" ] && printf '%s\n' "$f"
	done
)

_find_files() {
	local -a globs=("$@")
	[ ${#_CACHED_FILES[@]} -eq 0 ] && return
	for file in "${_CACHED_FILES[@]}"; do
		for glob in "${globs[@]}"; do
			# shellcheck disable=SC2254 # glob pattern matching is intentional
			case "$file" in
			$glob) echo "$file" ;;
			*/$glob) echo "$file" ;;
			esac
		done
	done | sort -u
}

# --- Run linters ---

_LINTER_RAN=false

_on_exit() {
	local ec=$?
	if [ $ec -ne 0 ] && [ "$_LINTER_RAN" = "true" ] && [ "$AUTOFIX" != "true" ]; then
		# shellcheck disable=SC2016 # backticks are intentional: literal formatting, not command substitution
		printf '\n💡 Try `mise run fix` to auto-fix lint issues, then re-run `mise run lint` to verify.\n'
	fi
	exit $ec
}
trap _on_exit EXIT

_LINTER_RAN=true
_failed=()
_skipped=()

for tool in "${_TOOLS[@]}"; do
	if [ -z "${_CHECK[$tool]+set}" ]; then
		printf '❌ Unknown linter: %s\n' "$tool" >&2
		exit 1
	fi

	check_cmd="${_CHECK[$tool]}"
	fix_cmd="${_FIX[$tool]}"
	tool_patterns="${_PATTERNS[$tool]}"
	bin="${check_cmd%% *}"

	if ! command -v "$bin" >/dev/null 2>&1; then
		_skipped+=("$tool")
		_failed+=("$tool")
		continue
	fi

	if [ "$AUTOFIX" = "true" ] && [ -n "$fix_cmd" ]; then
		cmd_template="$fix_cmd"
	else
		cmd_template="$check_cmd"
	fi

	# Substitute {MERGE_BASE}; strip --flag={MERGE_BASE} entirely when no merge base available
	if [ -n "$_MERGE_BASE" ]; then
		cmd_template="${cmd_template//\{MERGE_BASE\}/$_MERGE_BASE}"
	else
		cmd_template=$(printf '%s' "$cmd_template" | sed 's/ \?--[a-zA-Z_-]*={MERGE_BASE}//g')
	fi

	linter_failed=false

	if [ "$tool_patterns" = "SELF" ]; then
		if ! eval "$cmd_template"; then
			linter_failed=true
		fi
	else
		read -ra pattern_arr <<<"$tool_patterns"
		mapfile -t files < <(_find_files "${pattern_arr[@]}")

		# mapfile produces a single empty element when input is empty
		if [ ${#files[@]} -eq 0 ] || [[ ${#files[@]} -eq 1 && -z "${files[0]}" ]]; then
			continue
		fi

		if [[ "$cmd_template" == *"{FILES}"* ]]; then
			quoted_files=""
			for file in "${files[@]}"; do
				# shellcheck disable=SC2016 # single quotes are intentional to prevent expansion
				quoted_files+=" '${file//\'/\'\\\'\'}'"
			done
			cmd="${cmd_template//\{FILES\}/$quoted_files}"
			if ! eval "$cmd"; then
				linter_failed=true
			fi
		else
			for file in "${files[@]}"; do
				# shellcheck disable=SC2016 # single quotes are intentional to prevent expansion
				quoted_file="'${file//\'/\'\\\'\'}'"
				cmd="${cmd_template//\{FILE\}/$quoted_file}"
				if ! eval "$cmd"; then
					linter_failed=true
				fi
			done
		fi
	fi

	if [ "$linter_failed" = "true" ]; then
		_failed+=("$tool")
	fi
done

if [ ${#_skipped[@]} -gt 0 ]; then
	printf '\n❌ Missing lint tools: %s\n' "${_skipped[*]}"
fi

if [ ${#_failed[@]} -gt 0 ]; then
	printf '\n❌ Linting failed: %s\n' "${_failed[*]}"
	exit 1
fi
