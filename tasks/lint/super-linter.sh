#!/usr/bin/env bash
#MISE description="Run Super-Linter on the repository"

set -euo pipefail

#USAGE flag "--autofix" help="Enable autofix mode (enables FIX_* vars from the env file)"
#USAGE flag "--native" help="Run linters natively instead of via container"
#USAGE flag "--full" help="Lint all files instead of only changed files"

# shellcheck disable=SC2154 # usage_autofix is set by mise
if [ "${usage_autofix:-}" = "true" ]; then
	AUTOFIX=true
fi

# shellcheck disable=SC2154 # usage_native is set by mise
if [ "${usage_native:-}" = "true" ]; then
	NATIVE=true
fi
NATIVE="${NATIVE:-false}"

# shellcheck disable=SC2154 # usage_full is set by mise
LINT_ALL="${usage_full:-false}"

_LINTER_RAN=false

_on_exit() {
	local ec=$?
	if [ -n "${_FILTERED_ENV_FILE:-}" ]; then
		rm -f -- "$_FILTERED_ENV_FILE"
	fi
	if [ $ec -ne 0 ] && [ "$_LINTER_RAN" = "true" ] && [ "${AUTOFIX:-}" != "true" ]; then
		# shellcheck disable=SC2016 # backticks are intentional: literal formatting, not command substitution
		printf '\n💡 Try `mise run fix` to auto-fix lint issues, then re-run `mise run lint` to verify.\n'
	fi
	exit $ec
}
trap _on_exit EXIT

if [ "$NATIVE" != "true" ]; then
	# check for required env vars, otherwise exit with error
	if [ -z "${SUPER_LINTER_VERSION:-}" ]; then
		echo "SUPER_LINTER_VERSION environment variable is not set. Exiting."
		exit 1
	fi
fi

if [ -z "${MISE_PROJECT_ROOT:-}" ]; then
	echo "MISE_PROJECT_ROOT environment variable is not set. Exiting."
	exit 1
fi

cd "${MISE_PROJECT_ROOT}"

ENV_FILE="${SUPER_LINTER_ENV_FILE:-.github/config/super-linter.env}"

# --- Native mode ---
if [ "$NATIVE" = "true" ]; then
	# Activate the mise environment created by setup:native-lint-tools so that
	# installed tools (shfmt, actionlint, codespell, etc.) are on PATH.
	_SL_ENV_TOML=$(compgen -G ".mise.super-linter-*.toml" | head -1 || true)
	if [ -n "$_SL_ENV_TOML" ]; then
		_SL_ENV_NAME="${_SL_ENV_TOML#.mise.}"
		_SL_ENV_NAME="${_SL_ENV_NAME%.toml}"
		# Allow failure so the script falls through to the "Missing native lint tools"
		# message instead of exiting with a confusing mise error.
		eval "$(mise env -E "$_SL_ENV_NAME" 2>/dev/null)" || true
	fi

	# Native mode expects linter configs at the project root (standard tool locations).
	# Super-linter's .github/linters/ convention is not supported.
	LINTER_RULES_PATH="${LINTER_RULES_PATH:-.github/linters}"
	if [ -d "$LINTER_RULES_PATH" ]; then
		echo "Error: native mode does not support linter configs in ${LINTER_RULES_PATH}/."
		echo "Move config files to the project root (standard tool locations) instead."
		exit 1
	fi

	# Source env file to get VALIDATE_*, FIX_*, and FILTER_REGEX_EXCLUDE
	declare -A _env_vars
	while IFS='=' read -r key value; do
		[[ -z "$key" || "$key" =~ ^[[:space:]]*# ]] && continue
		# Trim whitespace
		key="${key%"${key##*[![:space:]]}"}"
		value="${value%"${value##*[![:space:]]}"}"
		_env_vars["$key"]="$value"
	done <"$ENV_FILE"

	FILTER_REGEX_EXCLUDE="${_env_vars[FILTER_REGEX_EXCLUDE]:-}"

	# Determine which linters are enabled using Super Linter's default logic:
	# If ANY VALIDATE_*=true is explicitly set, only those run.
	# Otherwise, all linters are enabled (unless explicitly VALIDATE_*=false).
	_has_explicit_true=false
	for key in "${!_env_vars[@]}"; do
		if [[ "$key" == VALIDATE_* && "${_env_vars[$key]}" == "true" ]]; then
			_has_explicit_true=true
			break
		fi
	done

	_is_enabled() {
		local flag="$1"
		local val="${_env_vars[$flag]:-}"
		if [ "$_has_explicit_true" = "true" ]; then
			# Explicit mode: only VALIDATE_*=true are enabled
			[[ "$val" == "true" ]]
		else
			# Default mode: all enabled unless explicitly false
			[[ "$val" != "false" ]]
		fi
	}

	_filter_files() {
		if [ -n "$FILTER_REGEX_EXCLUDE" ]; then
			grep -vE "$FILTER_REGEX_EXCLUDE" || true
		else
			cat
		fi
	}

	# Compute merge base once for changed-file detection and golangci-lint diff mode
	_BASE_BRANCH="${DEFAULT_BRANCH:-main}"
	_MERGE_BASE=$(git merge-base "origin/${_BASE_BRANCH}" HEAD 2>/dev/null || echo "")

	_list_files() {
		if [ "$LINT_ALL" = "true" ]; then
			git ls-files
		else
			if [ -n "$_MERGE_BASE" ]; then
				# Files changed in the PR + uncommitted (staged and unstaged) changes
				{
					git diff --name-only --diff-filter=d "$_MERGE_BASE"...HEAD
					git diff --name-only --diff-filter=d
					git diff --name-only --diff-filter=d --cached
				} | sort -u
			else
				# No merge base found (e.g. shallow clone), fall back to all files
				git ls-files
			fi
		fi
	}

	# Cache the file list once (avoids re-running git commands per linter)
	mapfile -t _CACHED_FILES < <(_list_files | _filter_files)

	_find_files() {
		local -a patterns=("$@")
		[ ${#_CACHED_FILES[@]} -eq 0 ] && return
		for file in "${_CACHED_FILES[@]}"; do
			for pattern in "${patterns[@]}"; do
				# shellcheck disable=SC2254 # glob pattern matching is intentional
				case "$file" in
				$pattern) echo "$file" ;;
				*/$pattern) echo "$file" ;;
				esac
			done
		done | sort -u
	}

	# Linter definitions: validate_flag|tool_binary|check_cmd_template|fix_cmd_template|file_patterns
	# {FILE} = per-file invocation (one run per file)
	# {FILES} = all matching files passed as arguments in one invocation
	# "SELF" = tool handles its own file discovery (no file args)
	# Config files: linters use their standard config discovery from the project root.
	declare -a LINTER_DEFS=(
		"VALIDATE_BASH|shellcheck|shellcheck {FILE}||*.sh *.bash"
		"VALIDATE_SHELL_SHFMT|shfmt|shfmt -d {FILE}|shfmt -w {FILE}|*.sh *.bash"
		"VALIDATE_MARKDOWN|markdownlint|markdownlint {FILE}|markdownlint --fix {FILE}|*.md"
		"VALIDATE_MARKDOWN_PRETTIER|prettier|prettier --check {FILE}|prettier --write {FILE}|*.md"
		"VALIDATE_YAML_PRETTIER|prettier|prettier --check {FILE}|prettier --write {FILE}|*.yaml *.yml"
		"VALIDATE_JSON_PRETTIER|prettier|prettier --check {FILE}|prettier --write {FILE}|*.json"
		"VALIDATE_EDITORCONFIG|editorconfig-checker|editorconfig-checker {FILES}||*"
		"VALIDATE_GITHUB_ACTIONS|actionlint|actionlint {FILE}||.github/workflows/*.yml .github/workflows/*.yaml"
		"VALIDATE_DOCKERFILE_HADOLINT|hadolint|hadolint {FILE}||Dockerfile Dockerfile.* *.dockerfile"
		"VALIDATE_GO_GOLANGCI_LINT|golangci-lint|golangci-lint run||SELF"
		"VALIDATE_PYTHON_RUFF|ruff|ruff check {FILE}|ruff check --fix {FILE}|*.py"
		"VALIDATE_PYTHON_RUFF_FORMAT|ruff|ruff format --check {FILE}|ruff format {FILE}|*.py"
		"VALIDATE_SPELL_CODESPELL|codespell|codespell {FILES}|codespell --write-changes {FILES}|*"
		"VALIDATE_JSONC|biome|biome check {FILE}|biome check --fix {FILE}|*.json *.jsonc *.js *.ts *.jsx *.tsx"
		"VALIDATE_BIOME_FORMAT|biome|biome format {FILE}|biome format --write {FILE}|*.json *.jsonc *.js *.ts *.jsx *.tsx"
	)

	# Track which VALIDATE_* flags from env are not supported
	declare -A _supported_flags
	for def in "${LINTER_DEFS[@]}"; do
		IFS='|' read -r flag _ _ _ _ <<<"$def"
		_supported_flags["$flag"]=1
	done

	_unsupported=()
	for key in "${!_env_vars[@]}"; do
		if [[ "$key" == VALIDATE_* && "${_env_vars[$key]}" == "true" && -z "${_supported_flags[$key]:-}" ]]; then
			_unsupported+=("$key")
		fi
	done

	if [ ${#_unsupported[@]} -gt 0 ]; then
		printf '⚠️  Not supported in native mode: %s\n' "${_unsupported[*]}"
	fi

	_LINTER_RAN=true
	_failed=()
	_skipped_flags=()
	_skipped_tools=()

	for def in "${LINTER_DEFS[@]}"; do
		IFS='|' read -r flag tool check_cmd fix_cmd patterns <<<"$def"

		if ! _is_enabled "$flag"; then
			continue
		fi

		if ! command -v "$tool" >/dev/null 2>&1; then
			_skipped_flags+=("$flag")
			_skipped_tools+=("$tool")
			continue
		fi

		# Determine which command to run
		if [ "${AUTOFIX:-}" = "true" ] && [ -n "$fix_cmd" ]; then
			# Check if FIX_ counterpart is enabled
			fix_flag="FIX_${flag#VALIDATE_}"
			if [ "${_env_vars[$fix_flag]:-}" = "true" ]; then
				cmd_template="$fix_cmd"
			else
				cmd_template="$check_cmd"
			fi
		else
			cmd_template="$check_cmd"
		fi

		linter_failed=false

		if [ "$patterns" = "SELF" ]; then
			# Tool handles its own file discovery; add diff flags when not linting all files
			if [ "$LINT_ALL" != "true" ] && [ -n "$_MERGE_BASE" ]; then
				if [[ "$cmd_template" == golangci-lint* ]]; then
					cmd_template+=" --new-from-rev=$_MERGE_BASE"
				fi
			fi
			if ! eval "$cmd_template"; then
				linter_failed=true
			fi
		else
			# Find matching files
			read -ra pattern_arr <<<"$patterns"
			mapfile -t files < <(_find_files "${pattern_arr[@]}")

			# mapfile produces a single empty element when input is empty
			if [ ${#files[@]} -eq 0 ] || [[ ${#files[@]} -eq 1 && -z "${files[0]}" ]]; then
				continue
			fi

			if [[ "$cmd_template" == *"{FILES}"* ]]; then
				# Build quoted file list for single invocation
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
				# Per-file invocation
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
			_failed+=("$flag")
		fi
	done

	if [ ${#_skipped_tools[@]} -gt 0 ]; then
		printf '\n❌ Missing native lint tools: %s\n' "${_skipped_tools[*]}"
		# shellcheck disable=SC2016 # backticks are intentional: literal formatting, not command substitution
		printf '   Run `mise run setup:native-lint-tools` to install them.\n'
		_failed+=("${_skipped_flags[@]}")
	fi

	if [ ${#_failed[@]} -gt 0 ]; then
		printf '\n❌ Native lint failed: %s\n' "${_failed[*]}"
		exit 1
	fi

	exit 0
fi

# --- Container mode ---
if [ "${AUTOFIX:-}" != "true" ]; then
	# Filter out FIX_* and comment lines when not auto-fixing
	_FILTERED_ENV_FILE=$(mktemp)
	grep -v '^#' "$ENV_FILE" | grep -v '^FIX_' >"$_FILTERED_ENV_FILE"
	ENV_FILE="$_FILTERED_ENV_FILE"
fi

if command -v podman >/dev/null 2>&1; then
	RUNTIME=podman
	# Fedora, by default, runs with SELinux on. We require the "z" option for bind mounts.
	# See: https://docs.docker.com/engine/storage/bind-mounts/#configure-the-selinux-label
	# See: https://docs.podman.io/en/stable/markdown/podman-run.1.html section "Labeling Volume Mounts"
	MOUNT_OPTS="rw,z"
elif command -v docker >/dev/null 2>&1; then
	RUNTIME=docker
	MOUNT_OPTS=rw
else
	echo "Unable to find a suitable container runtime such as Podman or Docker. Exiting."
	exit 1
fi

$RUNTIME image pull -q --platform linux/amd64 "ghcr.io/super-linter/super-linter:${SUPER_LINTER_VERSION}" >/dev/null

VALIDATE_ALL_CODEBASE="false"
if [ "$LINT_ALL" = "true" ]; then
	VALIDATE_ALL_CODEBASE="true"
fi

_LINTER_RAN=true
$RUNTIME container run --rm --platform linux/amd64 \
	-e RUN_LOCAL=true \
	-e DEFAULT_BRANCH=main \
	-e VALIDATE_ALL_CODEBASE="$VALIDATE_ALL_CODEBASE" \
	--env-file "$ENV_FILE" \
	-v "$(pwd)":/tmp/lint:"${MOUNT_OPTS}" \
	"ghcr.io/super-linter/super-linter:${SUPER_LINTER_VERSION}"
