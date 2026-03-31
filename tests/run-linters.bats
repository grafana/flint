#!/usr/bin/env bats
bats_require_minimum_version 1.5.0

SCRIPT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../tasks/lint" && pwd)/run-linters.sh"

setup() {
	TMPDIR="$(mktemp -d)"

	# Bare repo acts as "origin"
	git init --bare "$TMPDIR/origin.git" -q

	# Working repo
	git init "$TMPDIR/repo" -q
	git -C "$TMPDIR/repo" config user.email "test@test.com"
	git -C "$TMPDIR/repo" config user.name "Test"
	git -C "$TMPDIR/repo" remote add origin "$TMPDIR/origin.git"

	# Initial commit → becomes the merge base (simulates the main branch state)
	printf 'baseline\n' >"$TMPDIR/repo/baseline.txt"
	printf 'will change\n' >"$TMPDIR/repo/changing.txt"
	git -C "$TMPDIR/repo" add baseline.txt changing.txt
	git -C "$TMPDIR/repo" commit -m "initial" -q
	git -C "$TMPDIR/repo" branch -M main
	git -C "$TMPDIR/repo" push origin main -q

	# PR changes: modify one existing file, add one new file
	printf 'changed\n' >"$TMPDIR/repo/changing.txt"
	printf 'new file\n' >"$TMPDIR/repo/new.txt"
	git -C "$TMPDIR/repo" add changing.txt new.txt
	git -C "$TMPDIR/repo" commit -m "pr change" -q

	export MISE_PROJECT_ROOT="$TMPDIR/repo"
	export DEFAULT_BRANCH=main

	# Mock tools
	MOCK_BIN="$TMPDIR/bin"
	mkdir -p "$MOCK_BIN"
	export MOCK_LOG="$TMPDIR/mock.log"

	# Logs "check:<file>" per argument; exits 0
	cat >"$MOCK_BIN/mock-check" <<'EOF'
#!/usr/bin/env bash
for f in "$@"; do printf 'check:%s\n' "$f"; done >> "$MOCK_LOG"
EOF

	# Logs "fix:<file>" per argument; exits 0
	cat >"$MOCK_BIN/mock-fix" <<'EOF'
#!/usr/bin/env bash
for f in "$@"; do printf 'fix:%s\n' "$f"; done >> "$MOCK_LOG"
EOF

	# Always exits 1
	cat >"$MOCK_BIN/mock-fail" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF

	chmod +x "$MOCK_BIN"/mock-check "$MOCK_BIN"/mock-fix "$MOCK_BIN"/mock-fail

	# Extra registry injected via env var
	cat >"$TMPDIR/extra-registry.sh" <<'EOF'
_register mock           "mock-check {FILES}"  "mock-fix {FILES}"  "*.txt"
_register mock-fail      "mock-fail {FILES}"   ""                  "*.txt"
_register mock-yaml      "mock-check {FILES}"  ""                  "*.yaml"
_register missing-tool   "no-such-binary-xyz {FILES}" ""           "*.txt"
EOF

	export PATH="$MOCK_BIN:$PATH"
	export RUN_LINTERS_EXTRA_REGISTRY="$TMPDIR/extra-registry.sh"
}

teardown() {
	rm -rf "$TMPDIR"
}

@test "lints only changed files by default" {
	run bash "$SCRIPT" mock
	[ "$status" -eq 0 ]
	grep -q "check:changing.txt" "$MOCK_LOG"
	grep -q "check:new.txt" "$MOCK_LOG"
	run ! grep -q "check:baseline.txt" "$MOCK_LOG"
}

@test "--full lints all tracked files" {
	run bash "$SCRIPT" --full mock
	[ "$status" -eq 0 ]
	grep -q "check:baseline.txt" "$MOCK_LOG"
	grep -q "check:changing.txt" "$MOCK_LOG"
	grep -q "check:new.txt" "$MOCK_LOG"
}

@test "--autofix uses fix command" {
	run bash "$SCRIPT" --autofix mock
	[ "$status" -eq 0 ]
	grep -q "fix:changing.txt" "$MOCK_LOG"
	run ! grep -q "check:" "$MOCK_LOG"
}

@test "propagates linter failure" {
	run bash "$SCRIPT" mock-fail
	[ "$status" -ne 0 ]
}

@test "exits non-zero for unknown linter name" {
	run bash "$SCRIPT" not-registered
	[ "$status" -ne 0 ]
}

@test "exits non-zero when tool binary is missing" {
	run bash "$SCRIPT" missing-tool
	[ "$status" -ne 0 ]
}

@test "no changed files matching pattern exits zero without calling linter" {
	# No .yaml files were changed in the PR, so mock-yaml should not run
	run bash "$SCRIPT" mock-yaml
	[ "$status" -eq 0 ]
	[ ! -f "$MOCK_LOG" ]
}
