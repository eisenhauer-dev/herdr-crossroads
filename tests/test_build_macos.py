"""Packaging contracts with mocked tools; never native execution evidence."""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def helper():
    path = ROOT / "scripts/build-macos.py"
    if not path.exists():
        return None
    loader = importlib.machinery.SourceFileLoader("build_macos", str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


class BuildMacosTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir="/tmp/opencode")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.module = helper()
        self.calls = []
        self.gate_timeouts = []
        environment = patch.dict(os.environ, {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
            (self.root / name).write_text(name)

    def command(self, argv, *, timeout=None):
        self.calls.append(argv)
        if argv[1].startswith("tests/pty_"):
            self.gate_timeouts.append(timeout)
            self.assertEqual(Path(argv[2]).read_bytes(), b"exact native fixture")
        output = ""
        if argv[:2] == ["uname", "-s"]:
            output = "Darwin"
        elif argv[:2] == ["uname", "-m"]:
            output = self.arch
        elif "check-version.py" in argv[1]:
            output = "0.2.0"
        elif "build-provenance.py" in argv[1]:
            binary, stem = map(Path, argv[-2:])
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_bytes(b"exact native fixture")
            binary.chmod(0o755)
            Path(str(stem) + ".link.map").write_text("map")
            Path(str(stem) + ".provenance.json").write_text(json.dumps({
                "binary": {"sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}}))
        return subprocess.CompletedProcess(argv, 0, output, "")

    def build(self, arch="arm64", effect=None):
        self.assertIsNotNone(self.module, "native macOS build/gate/package entry point required")
        self.arch = arch
        with patch.object(self.module, "ROOT", self.root), patch.object(
                self.module, "run", side_effect=effect or self.command):
            return self.module.build()

    def test_both_architectures_package_exact_binary_and_four_files(self):
        for arch, target in [("arm64", "aarch64-apple-darwin"), ("x86_64", "x86_64-apple-darwin")]:
            with self.subTest(arch=arch):
                self.calls.clear()
                self.gate_timeouts.clear()
                archive = self.build(arch)
                self.assertEqual(archive.name, f"herdr-crossroads-0.2.0-{target}.tar.gz")
                gates = [c for c in self.calls if c[1].startswith("tests/pty_")]
                self.assertEqual([c[1] for c in gates], [f"tests/pty_{g}.py" for g in
                                                     ["latency", "render", "keymaps", "hints"]])
                self.assertTrue(all(len(c) == 3 for c in gates))
                self.assertEqual(self.gate_timeouts, [300] * 4)
                self.assertTrue(all(c[2] == str(self.root / f"target/{target}/release/herdr-crossroads")
                                    for c in gates))
                self.assertEqual(sum("build-provenance.py" in c[1] for c in self.calls), 1)
                with tarfile.open(archive) as package:
                    self.assertEqual(package.getnames(), ["herdr-crossroads", "herdr-crossroads.sha256",
                                                          "LICENSE", "THIRD-PARTY-NOTICES.md"])
                    for member in package:
                        self.assertTrue(member.isfile())
                        self.assertEqual((member.uid, member.gid, member.uname, member.gname), (0, 0, "", ""))
                        self.assertEqual(member.mode, 0o755 if member.name == "herdr-crossroads" else 0o644)
                    binary = package.extractfile("herdr-crossroads").read()
                    self.assertEqual(binary, b"exact native fixture")
                    self.assertEqual(package.extractfile("herdr-crossroads.sha256").read().decode(),
                                     hashlib.sha256(binary).hexdigest() + "  herdr-crossroads\n")
                self.assertEqual(Path(str(archive) + ".sha256").read_text(),
                                 hashlib.sha256(archive.read_bytes()).hexdigest() + f"  {archive.name}\n")

    def test_each_failed_gate_removes_current_artifacts(self):
        for gate in ["latency", "render", "keymaps", "hints"]:
            self.build()
            output = self.root / "target/builds"
            (output / "unrelated").write_text("keep")
            def fail(argv, **kwargs):
                if argv[1] == f"tests/pty_{gate}.py":
                    raise subprocess.CalledProcessError(23, argv)
                return self.command(argv, **kwargs)
            with self.assertRaises(subprocess.CalledProcessError):
                self.build(effect=fail)
            self.assertEqual([p.name for p in output.iterdir()], ["unrelated"])

    def test_gate_timings_and_captured_output_are_flushed_before_next_gate(self):
        events = []
        def emit(*args, **kwargs):
            events.append((args, kwargs))
        def command(argv, **kwargs):
            if argv[1].startswith("tests/pty_"):
                gate = Path(argv[1]).stem.removeprefix("pty_")
                self.assertTrue(events, "gate start must be flushed before launching the child")
                self.assertEqual(events[-1], ((f"PTY package {gate}: start (timeout=300s)",),
                                              {"flush": True}))
                if gate != "latency":
                    self.assertIn("passed in 1.25s", events[-2][0][0])
                self.command(argv, **kwargs)
                return subprocess.CompletedProcess(argv, 0, f"{gate} output\n", f"{gate} error\n")
            return self.command(argv, **kwargs)
        with patch("builtins.print", side_effect=emit), patch.object(
                self.module, "time") as clock:
            clock.monotonic.side_effect = [value for start in range(4) for value in (start * 10, start * 10 + 1.25)]
            self.build(effect=command)
        for gate in ["latency", "render", "keymaps", "hints"]:
            self.assertIn(((f"PTY package {gate}: passed in 1.25s",), {"flush": True}), events)
            self.assertIn(((f"{gate} output\n",), {"end": "", "flush": True}), events)
            self.assertIn(((f"{gate} error\n",), {"end": "", "file": sys.stderr, "flush": True}), events)

    def test_failed_interrupted_and_timed_out_gates_flush_elapsed_and_stop(self):
        for error in [subprocess.CalledProcessError(23, ["gate"]),
                      RuntimeError("PTY command timed out after 300s"), KeyboardInterrupt()]:
            with self.subTest(error=type(error).__name__):
                self.calls.clear()
                def fail(argv, **kwargs):
                    result = self.command(argv, **kwargs)
                    if argv[1] == "tests/pty_latency.py":
                        raise error
                    return result
                with patch("builtins.print") as output, patch.object(
                        self.module, "time") as clock:
                    clock.monotonic.side_effect = [10.0, 12.5]
                    with self.assertRaises(type(error)) as failure:
                        self.build(effect=fail)
                    self.assertIs(failure.exception, error)
                    output.assert_any_call("PTY package latency: start (timeout=300s)", flush=True)
                    output.assert_any_call("PTY package latency: failed in 2.50s", flush=True)
                    self.assertFalse(any("passed" in str(call) for call in output.call_args_list))
                self.assertEqual([c[1] for c in self.calls if c[1].startswith("tests/pty_")],
                                 ["tests/pty_latency.py"])
                self.assertEqual(list((self.root / "target/builds").iterdir()), [])

    def test_each_timed_out_gate_stops_and_removes_current_artifacts(self):
        run = self.module.run
        gates = ["latency", "render", "keymaps", "hints"]
        for index, gate in enumerate(gates):
            with self.subTest(gate=gate):
                self.build()
                self.calls.clear()
                output = self.root / "target/builds"
                (output / "unrelated").write_text("keep")
                def timeout(argv, **kwargs):
                    if argv[1] == f"tests/pty_{gate}.py":
                        return run([sys.executable, "-c", "import time; time.sleep(60)"],
                                   timeout=0.05)
                    return self.command(argv, **kwargs)
                with self.assertRaisesRegex(RuntimeError, "timed out"):
                    self.build(effect=timeout)
                self.assertEqual([p.name for p in output.iterdir()], ["unrelated"])
                self.assertEqual([c[1] for c in self.calls if c[1].startswith("tests/pty_")],
                                 [f"tests/pty_{g}.py" for g in gates[:index]])

    def test_bounded_command_timeout_kills_descendants_and_keeps_diagnostics(self):
        marker = self.root / "survived"
        child = ("import pathlib, time; print('descendant ready', flush=True); "
                 f"time.sleep(2); pathlib.Path({str(marker)!r}).touch()")
        parent = ("import subprocess, sys, time; print('gate output', flush=True); "
                  "print('gate error', file=sys.stderr, flush=True); "
                  f"subprocess.Popen([sys.executable, '-c', {child!r}]); time.sleep(60)")
        with self.assertRaisesRegex(RuntimeError, "timed out after 1") as failure:
            self.module.run([sys.executable, "-c", parent], timeout=1)
        self.assertIn("gate output", str(failure.exception))
        self.assertIn("gate error", str(failure.exception))
        self.assertIn("descendant ready", str(failure.exception))
        time.sleep(1.5)
        self.assertFalse(marker.exists(), "timed-out gate descendant must not survive")

    def test_bounded_command_success_and_failure_keep_output(self):
        for code in [0, 23]:
            argv = [sys.executable, "-c", "import sys; print('out'); "
                    f"print('err', file=sys.stderr); sys.exit({code})"]
            if code:
                with self.assertRaises(subprocess.CalledProcessError) as failure:
                    self.module.run(argv, timeout=5)
                result = failure.exception
                self.assertEqual(result.returncode, code)
                self.assertEqual(result.output, "out\n")
            else:
                result = self.module.run(argv, timeout=5)
                self.assertEqual(result.stdout, "out\n")
            self.assertEqual(result.stderr, "err\n")

    def test_bounded_command_interruption_and_exited_group_are_reaped(self):
        for error in [KeyboardInterrupt(), subprocess.TimeoutExpired(["gate"], 300)]:
            for exited in [False, True]:
                with self.subTest(error=type(error).__name__, exited=exited), patch.object(
                        self.module.subprocess, "Popen") as popen, patch.object(
                        self.module.os, "killpg", side_effect=ProcessLookupError() if exited else None) as kill:
                    process = popen.return_value
                    process.pid = 12345
                    process.communicate.side_effect = [error, ("partial stdout", "partial stderr")]
                    with self.assertRaises(RuntimeError if isinstance(error, subprocess.TimeoutExpired)
                                           else KeyboardInterrupt):
                        self.module.run(["gate"], timeout=300)
                    self.assertTrue(popen.call_args.kwargs["start_new_session"])
                    kill.assert_called_once_with(process.pid, signal.SIGKILL)
                    self.assertEqual(process.communicate.call_count, 2)

    def test_changed_binary_after_gate_is_never_packaged(self):
        def mutate(argv, **kwargs):
            result = self.command(argv, **kwargs)
            if argv[1] == "tests/pty_hints.py":
                Path(argv[2]).write_bytes(b"mutated")
            return result
        with self.assertRaisesRegex(RuntimeError, "changed"):
            self.build(effect=mutate)
        self.assertEqual(list((self.root / "target/builds").iterdir()), [])

    def test_unsupported_architecture_fails_before_build(self):
        with self.assertRaisesRegex(RuntimeError, "native"):
            self.build("i386")
        self.assertFalse(any("build-provenance.py" in c[1] for c in self.calls))

    def test_build_notice_and_packaging_errors_remove_stale_outputs(self):
        for stage in ["third-party-notices.py", "build-provenance.py", "archive"]:
            with self.subTest(stage=stage):
                self.build()
                def fail(argv, **kwargs):
                    if stage in argv[1]:
                        raise subprocess.CalledProcessError(19, argv)
                    return self.command(argv, **kwargs)
                with patch.object(self.module.tarfile, "open", side_effect=OSError("archive failure")) if stage == "archive" else patch.object(self.module, "digest", wraps=self.module.digest):
                    with self.assertRaises((OSError, subprocess.CalledProcessError)):
                        self.build(effect=fail)
                self.assertEqual(list((self.root / "target/builds").iterdir()), [])

    def test_compiler_override_is_rejected_not_silently_used(self):
        self.build()
        self.calls.clear()
        with patch.dict(os.environ, {"RUSTFLAGS": "-C link-arg=/unreviewed.a"}):
            with self.assertRaisesRegex(RuntimeError, "RUSTFLAGS"):
                self.build()
        self.assertFalse(any("build-provenance.py" in c[1] for c in self.calls))
        self.assertEqual(list((self.root / "target/builds").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
