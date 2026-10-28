"""Tests for the standalone Renovate snapshot task's credential fallback."""

import importlib.util
import os
import subprocess
import unittest
from pathlib import Path
from unittest.mock import Mock

os.environ.setdefault("MISE_PROJECT_ROOT", str(Path(__file__).resolve().parents[1]))
_SCRIPT = Path(__file__).resolve().parents[1] / "tasks/lint/renovate-deps.py"
_SPEC = importlib.util.spec_from_file_location("renovate_deps", _SCRIPT)
renovate_deps = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(renovate_deps)


class GithubTokenTests(unittest.TestCase):
    def test_explicit_tokens_take_precedence(self):
        runner = Mock()
        self.assertEqual(
            renovate_deps.github_token(
                {"GITHUB_COM_TOKEN": "com", "GITHUB_TOKEN": "ci"}, runner
            ),
            "com",
        )
        self.assertEqual(
            renovate_deps.github_token(
                {"GITHUB_COM_TOKEN": " ", "GITHUB_TOKEN": "ci"}, runner
            ),
            "ci",
        )
        runner.assert_not_called()

    def test_cli_fallback_uses_matching_host_and_does_not_mutate_env(self):
        env = {"GITHUB_SERVER_URL": "https://git.example.test:8443"}
        runner = Mock(
            return_value=subprocess.CompletedProcess([], 0, " gh-token\n", "")
        )
        self.assertEqual(renovate_deps.github_token(env, runner), "gh-token")
        self.assertEqual(
            runner.call_args.args[0],
            ["gh", "auth", "token", "--hostname", "git.example.test:8443"],
        )
        self.assertEqual(env, {"GITHUB_SERVER_URL": "https://git.example.test:8443"})

    def test_server_url_is_used_instead_of_api_url(self):
        runner = Mock(return_value=subprocess.CompletedProcess([], 0, "token", ""))
        renovate_deps.github_token(
            {
                "GITHUB_SERVER_URL": "https://github.com/",
                "GITHUB_API_URL": "https://api.other.test/",
            },
            runner,
        )
        self.assertEqual(runner.call_args.args[0][-1], "github.com")

    def test_invalid_url_and_missing_cli_are_graceful(self):
        runner = Mock()
        self.assertIsNone(
            renovate_deps.github_token({"GITHUB_SERVER_URL": "not a url"}, runner)
        )
        runner.assert_not_called()
        self.assertIsNone(
            renovate_deps.github_token({}, Mock(side_effect=FileNotFoundError("gh")))
        )


if __name__ == "__main__":
    unittest.main()
