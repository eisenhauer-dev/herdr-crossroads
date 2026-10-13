"""Exercise the opt-in hooks with isolated Git repositories and fake Cargo."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SETUP = r"""(
    if git config --get-all core.hooksPath; then
        printf '%s\n' 'Existing hooksPath: leave it unchanged; do not run setup.' >&2
        exit 1
    fi
    git config --local core.hooksPath .githooks
)"""
REMOVE = "git config --local --unset core.hooksPath"


class GitHookTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(
            dir="/tmp/opencode", prefix="crossroads-hooks-"
        )
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repo = self.directory / "repo"
        self.repo.mkdir()
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        self.log = self.directory / "cargo.log"
        self.env = {
            "PATH": f"{self.bin}:/usr/bin:/bin",
            "HOME": str(self.directory),
            "LC_ALL": "C",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "commit.gpgSign",
            "GIT_CONFIG_VALUE_0": "false",
            "GIT_AUTHOR_NAME": "Hook Test",
            "GIT_AUTHOR_EMAIL": "hook-test@example.invalid",
            "GIT_COMMITTER_NAME": "Hook Test",
            "GIT_COMMITTER_EMAIL": "hook-test@example.invalid",
            "CARGO_LOG": str(self.log),
        }
        self.git("init", "--template=", "--quiet")
        hooks = self.repo / ".githooks"
        hooks.mkdir()
        source = ROOT / ".githooks/pre-commit"
        # With no implementation, Git still commits: the RED is domain behavior.
        if source.exists():
            shutil.copy2(source, hooks / "pre-commit")
        source = ROOT / ".githooks/pre-push"
        # Without pre-push, a failing test status still allows the actual push.
        if source.exists():
            shutil.copy2(source, hooks / "pre-push")
        self.run_command(["sh", "-c", SETUP])
        self.file = self.repo / "example.txt"
        self.file.write_text("staged content\n")
        self.git("add", "example.txt")
        self.fake_cargo = self.bin / "cargo"
        self.fake_cargo.write_text(
            '#!/bin/sh\n'
            'printf "%s|%s\\n" "$PWD" "$*" >> "$CARGO_LOG"\n'
            'case "$1" in\n'
            '    fmt) exit "${FMT_STATUS:-0}" ;;\n'
            '    clippy) exit "${CLIPPY_STATUS:-0}" ;;\n'
            '    test) exit "${TEST_STATUS:-0}" ;;\n'
            '    *) exit 99 ;;\n'
            'esac\n'
        )
        self.fake_cargo.chmod(0o755)

    def run_command(self, command, *, check=True, cwd=None):
        return subprocess.run(
            command, cwd=cwd or self.repo, env=self.env,
            text=True, capture_output=True, check=check,
        )

    def git(self, *args, check=True, cwd=None):
        return self.run_command(["git", *args], check=check, cwd=cwd)

    def calls(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def expected_calls(self):
        return [
            f"{self.repo}|fmt --check",
            f"{self.repo}|clippy --locked --all-targets -- -D warnings",
        ]

    def commit(self, cwd=None):
        return self.git("commit", "--quiet", "-m", "Exercise hook", check=False, cwd=cwd)

    def test_success_checks_in_order_from_worktree_root_without_mutation(self):
        self.file.write_text("unstaged content\n")
        before = self.git("write-tree").stdout
        nested = self.repo / "nested"
        nested.mkdir()
        result = self.commit(cwd=nested)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), self.expected_calls())
        self.assertEqual(self.git("write-tree").stdout, before)
        self.assertEqual(self.file.read_text(), "unstaged content\n")
        self.assertEqual(self.git("show", "HEAD:example.txt").stdout, "staged content\n")

    def test_formatter_failure_blocks_commit_and_skips_clippy(self):
        self.env["FMT_STATUS"] = "23"
        before = self.git("write-tree").stdout
        result = self.commit()
        self.assertNotEqual(result.returncode, 0, "formatter failure must block commit")
        self.assertEqual(self.calls(), self.expected_calls()[:1])
        self.assertEqual(self.git("write-tree").stdout, before)
        self.assertEqual(self.file.read_text(), "staged content\n")
        self.assertNotEqual(self.git("rev-parse", "--verify", "HEAD", check=False).returncode, 0)
        direct = self.run_command(["sh", ".githooks/pre-commit"], check=False)
        self.assertEqual(direct.returncode, 23)

    def test_clippy_failure_blocks_commit(self):
        self.env["CLIPPY_STATUS"] = "29"
        result = self.commit()
        self.assertNotEqual(result.returncode, 0, "Clippy failure must block commit")
        self.assertEqual(self.calls(), self.expected_calls())
        self.assertNotEqual(self.git("rev-parse", "--verify", "HEAD", check=False).returncode, 0)
        direct = self.run_command(["sh", ".githooks/pre-commit"], check=False)
        self.assertEqual(direct.returncode, 29)

    def test_missing_cargo_blocks_commit(self):
        self.fake_cargo.unlink()
        # Keep Git available, but no system or user Cargo can leak into this test.
        git_only = self.bin / "git"
        git_only.symlink_to(shutil.which("git"))
        self.env["PATH"] = str(self.bin)
        result = self.commit()
        self.assertNotEqual(result.returncode, 0, "missing Cargo must block commit")
        self.assertIn("cargo: not found", result.stderr)
        self.assertEqual(self.calls(), [])

    def test_removal_restores_unset_configuration_and_normal_commits(self):
        prior_hook = self.repo / ".git/hooks/pre-commit"
        prior_hook.parent.mkdir()
        prior_hook.write_text("#!/bin/sh\nprintf '%s\\n' restored > prior-hook-ran\n")
        prior_hook.chmod(0o755)
        self.env["FMT_STATUS"] = "23"
        self.assertNotEqual(self.commit().returncode, 0)
        self.assertFalse((self.repo / "prior-hook-ran").exists())
        self.log.unlink()
        self.run_command(["sh", "-c", REMOVE])
        self.assertEqual(self.git("config", "--local", "--get", "core.hooksPath", check=False).returncode, 1)
        result = self.commit()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual((self.repo / "prior-hook-ran").read_text(), "restored\n")

    def test_setup_refuses_existing_local_or_inherited_hooks_path(self):
        self.run_command(["sh", "-c", REMOVE])
        for scope in ["--local", "--global"]:
            with self.subTest(scope=scope):
                # The isolated global file is not the user's configuration.
                self.env["GIT_CONFIG_GLOBAL"] = str(self.directory / "gitconfig")
                self.git("config", scope, "core.hooksPath", "prior-hooks")
                result = self.run_command(["sh", "-c", SETUP], check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.git("config", "--get-all", "core.hooksPath").stdout, "prior-hooks\n")
                self.git("config", scope, "--unset", "core.hooksPath")

    def test_hook_is_executable(self):
        self.assertTrue(os.access(ROOT / ".githooks/pre-commit", os.X_OK))

    def prepare_push(self):
        self.remote = self.directory / "remote.git"
        self.git("init", "--bare", "--template=", "--quiet", str(self.remote))
        result = self.commit()
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.remote_before = self.remote_ref()
        self.file.write_text("next commit\n")
        self.git("add", "example.txt")
        result = self.commit()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.log.unlink()

    def push(self, *options, cwd=None):
        return self.git(
            "-c", "protocol.file.allow=always", "-c", "push.gpgSign=false",
            "push", *options, str(self.remote), "HEAD:refs/heads/hook-test",
            check=False, cwd=cwd,
        )

    def remote_ref(self):
        return self.git("--git-dir", str(self.remote), "rev-parse", "refs/heads/hook-test").stdout

    def test_test_failure_blocks_push_without_advancing_remote(self):
        self.prepare_push()
        self.env["TEST_STATUS"] = "31"
        before = self.git("write-tree").stdout
        result = self.push()
        self.assertNotEqual(result.returncode, 0, "Cargo test failure must block push")
        self.assertEqual(self.remote_ref(), self.remote_before)
        self.assertEqual(self.calls(), [f"{self.repo}|test --locked"])
        self.assertEqual(self.git("write-tree").stdout, before)
        self.assertEqual(self.file.read_text(), "next commit\n")
        direct = self.run_command(["sh", ".githooks/pre-push"], check=False)
        self.assertEqual(direct.returncode, 31)

    def test_success_push_checks_worktree_root_without_mutation(self):
        self.prepare_push()
        self.file.write_text("staged after commit\n")
        self.git("add", "example.txt")
        self.file.write_text("unstaged after commit\n")
        before = self.git("write-tree").stdout
        status = self.git("status", "--porcelain", "--untracked-files=all").stdout
        nested = self.repo / "nested"
        nested.mkdir()
        result = self.push(cwd=nested)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [f"{self.repo}|test --locked"])
        self.assertNotEqual(self.remote_ref(), self.remote_before)
        self.assertEqual(self.remote_ref(), self.git("rev-parse", "HEAD").stdout)
        self.assertEqual(self.git("write-tree").stdout, before)
        self.assertEqual(self.file.read_text(), "unstaged after commit\n")
        self.assertEqual(self.git("status", "--porcelain", "--untracked-files=all").stdout, status)

    def test_missing_cargo_blocks_push(self):
        self.prepare_push()
        self.fake_cargo.unlink()
        (self.bin / "git").symlink_to(shutil.which("git"))
        (self.bin / "git-receive-pack").symlink_to(shutil.which("git-receive-pack"))
        self.env["PATH"] = str(self.bin)
        result = self.push()
        self.assertNotEqual(result.returncode, 0, "missing Cargo must block push")
        self.assertIn("cargo: not found", result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual(self.remote_ref(), self.remote_before)

    def test_removal_restores_prior_push_hook(self):
        self.prepare_push()
        prior_hook = self.repo / ".git/hooks/pre-push"
        prior_hook.parent.mkdir()
        prior_hook.write_text("#!/bin/sh\nprintf '%s\\n' restored > prior-push-hook-ran\n")
        prior_hook.chmod(0o755)
        self.env["TEST_STATUS"] = "31"
        self.assertNotEqual(self.push().returncode, 0)
        self.assertEqual(self.remote_ref(), self.remote_before)
        self.assertFalse((self.repo / "prior-push-hook-ran").exists())
        self.log.unlink()
        self.run_command(["sh", "-c", REMOVE])
        self.assertEqual(self.git("config", "--local", "--get", "core.hooksPath", check=False).returncode, 1)
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual(self.remote_ref(), self.git("rev-parse", "HEAD").stdout)
        self.assertEqual((self.repo / "prior-push-hook-ran").read_text(), "restored\n")

    def test_push_no_verify_bypasses_checks(self):
        self.prepare_push()
        self.env["TEST_STATUS"] = "31"
        result = self.push("--no-verify")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [])
        self.assertEqual(self.remote_ref(), self.git("rev-parse", "HEAD").stdout)

    def test_pre_push_is_executable(self):
        self.assertTrue(os.access(ROOT / ".githooks/pre-push", os.X_OK))

    def test_documented_setup_and_removal_match_tested_commands(self):
        documentation = (ROOT / "CONTRIBUTING.md").read_text()
        self.assertIn(f"```sh\n{SETUP}\n```", documentation)
        self.assertIn(f"```sh\n{REMOVE}\n```", documentation)


if __name__ == "__main__":
    unittest.main()
