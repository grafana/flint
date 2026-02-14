#!/usr/bin/env python3
# pylint: disable=invalid-name,duplicate-code
# [MISE] description="Verify renovate-tracked-deps.json is up to date"
"""Verify renovate-tracked-deps.json is up to date."""

import difflib
import json
import os
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

_repo_root_env = os.environ.get("MISE_PROJECT_ROOT")
if _repo_root_env is None:
    print(
        "ERROR: MISE_PROJECT_ROOT is not set. Run this script via 'mise run'.",
        file=sys.stderr,
    )
    sys.exit(1)
REPO_ROOT = Path(_repo_root_env)
COMMITTED = REPO_ROOT / ".github" / "renovate-tracked-deps.json"

EXCLUDED_MANAGERS = {m.strip() for m in os.environ.get("RENOVATE_TRACKED_DEPS_EXCLUDE", "").split(",") if m.strip()}  # pylint: disable=line-too-long  # noqa: E501


def run_renovate(tmpdir):
    """Run Renovate locally and return the log path."""
    config_path = str(REPO_ROOT / ".github" / "renovate.json5")
    log_path = os.path.join(tmpdir, "renovate.log")
    env = {
        **os.environ,
        "LOG_LEVEL": "debug",
        "LOG_FORMAT": "json",
        "RENOVATE_CONFIG_FILE": config_path,
    }
    with open(log_path, "w", encoding="utf-8") as log_file:
        result = subprocess.run(
            [
                "renovate",
                "--platform=local",
                "--require-config=ignored",
            ],
            env=env,
            stdout=log_file,
            stderr=subprocess.STDOUT,
            check=False,
            cwd=REPO_ROOT,
        )
    if result.returncode != 0:
        print(
            f"ERROR: Renovate failed (exit {result.returncode}). See log: {log_path}",
            file=sys.stderr,
        )
        sys.exit(result.returncode)
    return log_path


def extract_deps(log_path):
    """Parse Renovate log and return deps grouped by file and manager."""
    config = None
    with open(log_path, encoding="utf-8") as f:
        for line in f:
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            if entry.get("msg") == "packageFiles with updates":
                config = entry.get("config", {})

    if config is None:
        print(
            "ERROR: 'packageFiles with updates' message not found in Renovate log.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Skip reasons that mean "not a real dep"
    skip_reasons_to_exclude = {
        "contains-variable",
        "invalid-value",
        "invalid-version",
    }

    # {file_path: {manager: set(dep_names)}}
    deps_by_file = defaultdict(lambda: defaultdict(set))
    for manager, manager_files in config.items():
        if manager in EXCLUDED_MANAGERS:
            continue
        for pkg_file in manager_files:
            file_path = pkg_file.get("packageFile", "")
            for dep in pkg_file.get("deps", []):
                if dep.get("skipReason") in skip_reasons_to_exclude:
                    continue
                dep_name = dep.get("depName")
                if dep_name:
                    deps_by_file[file_path][manager].add(dep_name)

    result = {}
    for file_path in sorted(deps_by_file):
        managers = deps_by_file[file_path]
        result[file_path] = {m: sorted(managers[m]) for m in sorted(managers)}
    return result


def main():
    """Verify renovate-tracked-deps.json is up to date."""
    autofix = os.environ.get("AUTOFIX", "").lower() == "true"

    with tempfile.TemporaryDirectory() as tmpdir:
        log_path = run_renovate(tmpdir)
        generated_data = extract_deps(log_path)

        if not COMMITTED.exists():
            if autofix:
                print("AUTOFIX=true: Creating renovate-tracked-deps.json...")
                with open(COMMITTED, "w", encoding="utf-8") as f:
                    json.dump(generated_data, f, indent=2)
                    f.write("\n")
                print("renovate-tracked-deps.json has been created.")
                committed_data = generated_data
            else:
                print(f"ERROR: {COMMITTED} does not exist.", file=sys.stderr)
                print(
                    "Run 'mise run lint:renovate-deps' with AUTOFIX=true to create it.",
                    file=sys.stderr,
                )
                sys.exit(1)
        else:
            committed_data = json.loads(COMMITTED.read_text())

        if committed_data == generated_data:
            print("renovate-tracked-deps.json is up to date.")
        else:

            def normalize(d):
                return json.dumps(d, indent=2, sort_keys=True) + "\n"

            diff = difflib.unified_diff(
                normalize(committed_data).splitlines(keepends=True),
                normalize(generated_data).splitlines(keepends=True),
                fromfile=str(COMMITTED),
                tofile="generated",
            )
            print("".join(diff))

            if autofix:
                print("AUTOFIX=true: Updating renovate-tracked-deps.json...")
                with open(COMMITTED, "w", encoding="utf-8") as f:
                    json.dump(generated_data, f, indent=2)
                    f.write("\n")
                print("renovate-tracked-deps.json has been updated.")
            else:
                print("ERROR: renovate-tracked-deps.json is out of date.", file=sys.stderr)
                print(
                    "Run 'mise run lint:renovate-deps' with AUTOFIX=true to update.",
                    file=sys.stderr,
                )
                sys.exit(1)


if __name__ == "__main__":
    main()
