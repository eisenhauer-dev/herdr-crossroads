"""Execute the trusted inline policy; textual checks are not an Actions validator."""

import ast
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/pr-source-policy.yml"


class PrSourcePolicyTests(unittest.TestCase):
    def setUp(self):
        self.workflow = WORKFLOW.read_text() if WORKFLOW.is_file() else ""

    def event(self, base="main", head_id=123, branch="develop"):
        return {
            "repository": {"id": 123, "private": False},
            "pull_request": {
                "base": {"ref": base, "repo": {"id": 123}},
                "head": {"ref": branch, "repo": {"id": head_id}},
            },
        }

    def check_policy(self, event, allowed):
        script = textwrap.dedent(self.workflow.split("        run: |\n", 1)[1])
        with tempfile.TemporaryDirectory(prefix="pr-source-policy-", dir="/tmp/opencode") as tmp:
            path = Path(tmp) / "event.json"
            path.write_text(json.dumps(event))
            result = subprocess.run(
                ["bash", "--noprofile", "--norc", "-eu", "-o", "pipefail", "-c", script],
                cwd=tmp,
                env={"PATH": os.defpath, "GITHUB_EVENT_PATH": str(path)},
                text=True, capture_output=True, timeout=10,
            )
            self.assertEqual(result.returncode == 0, allowed, result.stdout + result.stderr)
            if allowed:
                self.assertIn("PR source policy: allowed", result.stdout)
            self.assertEqual(sorted(p.name for p in Path(tmp).iterdir()), ["event.json"])

    def test_trusted_event_and_least_privilege_public_job(self):
        self.assertEqual(self.workflow.split("    steps:\n", 1)[0], """name: PR source policy

on:
  pull_request_target:
    branches: [main, develop]
    types: [opened, synchronize, reopened, edited]

permissions: {}

jobs:
  source-policy:
    name: PR source policy
    if: github.event.repository.private == false
    runs-on: ubuntu-24.04
    timeout-minutes: 5
""")
        self.assertEqual(re.findall(r"^      - name: (.*)$", self.workflow, re.M), ["Validate source repository metadata"])
        self.assertEqual(self.workflow.count("        run: |\n"), 1)
        for forbidden in ["uses:", "checkout", "secrets", "${{", "curl", "wget", "cache", "continue-on-error", "GITHUB_TOKEN"]:
            self.assertNotIn(forbidden, self.workflow)
        self.assertIn("python3 -I - <<'PY'\n", self.workflow)
        self.assertIn('os.environ["GITHUB_EVENT_PATH"]', self.workflow)
        script = textwrap.dedent(self.workflow.split("        run: |\n", 1)[1])
        inline = re.fullmatch(r"python3 -I - <<'PY'\n(.*)\nPY\n", script, re.S)
        self.assertIsNotNone(inline, "only the inline Python policy may execute")
        imports = [node for node in ast.walk(ast.parse(inline[1]))
                   if isinstance(node, (ast.Import, ast.ImportFrom))]
        self.assertTrue(all(isinstance(node, ast.Import) for node in imports))
        self.assertEqual([alias.name for node in imports for alias in node.names], ["json", "os", "sys"])

    def test_internal_develop_to_main_is_allowed(self):
        self.check_policy(self.event(), True)

    def test_other_internal_branches_to_main_are_denied(self):
        for branch in ["feature/change", "main", "release/next", "Develop", "develop/next"]:
            with self.subTest(branch=branch):
                self.check_policy(self.event(branch=branch), False)

    def test_main_rejects_missing_null_and_malformed_head_ref(self):
        for branch in [None, "", True, 123, [], {}, ["develop"], {"ref": "develop"}]:
            with self.subTest(branch=branch):
                self.check_policy(self.event(branch=branch), False)
        event = self.event()
        del event["pull_request"]["head"]["ref"]
        self.check_policy(event, False)

    def test_fork_to_main_fails_even_with_same_branch_or_owner_name(self):
        for branch in ["develop", "main", "feature/change"]:
            with self.subTest(branch=branch):
                event = self.event(head_id=456, branch=branch)
                event["pull_request"]["head"]["repo"]["full_name"] = "eisenhauer-dev/herdr-crossroads"
                self.check_policy(event, False)

    def test_develop_accepts_internal_and_fork_sources(self):
        for head_id in [123, 456, None]:
            for branch in ["develop", "feature/change", "main"]:
                with self.subTest(head_id=head_id, branch=branch):
                    self.check_policy(self.event(base="develop", head_id=head_id, branch=branch), True)

    def test_main_rejects_missing_null_and_malformed_repository_identities(self):
        for location in ["head", "base", "repository"]:
            for repo in [None, {}, [], "123", {"id": None}, {"id": "123"},
                         {"id": True}, {"id": 123.0}, {"id": 0}, {"id": -1}]:
                with self.subTest(location=location, repo=repo):
                    event = self.event()
                    if location == "repository":
                        event[location] = repo
                    else:
                        event["pull_request"][location]["repo"] = repo
                    self.check_policy(event, False)
            event = self.event()
            if location == "repository":
                del event[location]
            else:
                del event["pull_request"][location]["repo"]
            self.check_policy(event, False)

    def test_main_requires_base_repository_to_match_event_repository(self):
        event = self.event(head_id=456)
        event["pull_request"]["base"]["repo"]["id"] = 456
        self.check_policy(event, False)

    def test_missing_or_unexpected_target_and_malformed_payload_fail_closed(self):
        for event in [None, [], {}, self.event(base="other")]:
            with self.subTest(event=event):
                self.check_policy(event, False)
        for location in ["base", "head"]:
            event = self.event()
            del event["pull_request"][location]
            self.check_policy(event, False)

    def test_untrusted_strings_remain_data_not_shell_or_python(self):
        event = self.event(branch="$(touch injected); ' PY\nraise SystemExit(0)")
        event["pull_request"]["title"] = "${{ secrets.TOKEN }}; touch injected"
        self.check_policy(event, False)
        event["pull_request"]["base"]["ref"] = "develop"
        self.check_policy(event, True)
        event["pull_request"]["base"]["ref"] = "main"
        event["pull_request"]["head"]["ref"] = "develop"
        event["pull_request"]["head"]["repo"]["id"] = "$(touch injected)"
        self.check_policy(event, False)


if __name__ == "__main__":
    unittest.main()
