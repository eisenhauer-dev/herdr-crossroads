"""Portable regression checks for the Windows preparation command watchdog."""
from contextlib import redirect_stdout, redirect_stderr
import io
import os
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
RUN = runpy.run_path(str(ROOT / "scripts/windows-process.py"))["run"]


class WindowsProcessTests(unittest.TestCase):
    def test_silent_command_has_a_deadline_and_kills_descendants(self):
        with tempfile.TemporaryDirectory(prefix="crossroads-watchdog-") as temporary:
            marker = Path(temporary) / "escaped"
            module = ROOT / "scripts/windows-process.py"
            script = (
                "import runpy,sys; "
                f"m=runpy.run_path({str(module)!r}); "
                "m['run'].__globals__['COMMAND_TIMEOUT']=0.3; "
                "m['run']([sys.executable,'-c',sys.argv[1]])"
            )
            descendant = f"import time,pathlib; time.sleep(1.5); pathlib.Path({str(marker)!r}).touch()"
            child = (
                "import subprocess,sys,time; "
                f"subprocess.Popen([sys.executable,'-c',{descendant!r}]); "
                "print('watchdog child started',flush=True); time.sleep(60)"
            )
            with subprocess.Popen([sys.executable, "-B", "-c", script, child],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  start_new_session=os.name != "nt") as process:
                try:
                    stdout, stderr = process.communicate(timeout=12)
                except subprocess.TimeoutExpired:
                    if os.name == "nt":
                        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], timeout=5)
                    else:
                        os.killpg(process.pid, signal.SIGKILL)
                    process.communicate(timeout=5)
                    self.fail("silent Windows build command outlived its watchdog deadline")
            self.assertNotEqual(process.returncode, 0)
            self.assertIn(b"deadline", stderr)
            self.assertIn(b"watchdog child started", stdout)
            time.sleep(1.6)
            self.assertFalse(marker.exists(), "timed-out command left a live descendant")

    def test_capture_success_and_failure_preserve_both_streams(self):
        for code in (0, 7):
            command = [sys.executable, "-c", f"import sys; print('out'); print('err',file=sys.stderr); sys.exit({code})"]
            with self.subTest(code=code):
                if code:
                    with self.assertRaises(subprocess.CalledProcessError) as caught:
                        RUN(command, capture=True, timeout=2)
                    result = caught.exception
                    self.assertEqual(result.returncode, 7)
                    self.assertEqual(result.output, b"out\n" if os.name != "nt" else b"out\r\n")
                else:
                    result = RUN(command, capture=True, timeout=2)
                    self.assertEqual(result.stdout.strip(), b"out")
                self.assertEqual(result.stderr.strip(), b"err")

    def test_captured_timeout_reports_output_without_waiting_for_inherited_pipes(self):
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors), self.assertRaises(subprocess.TimeoutExpired):
            RUN([sys.executable, "-c", "import time; print('before hang',flush=True); time.sleep(60)"],
                capture=True, timeout=0.2)
        self.assertIn("before hang", output.getvalue())
        self.assertIn("exceeded deadline", errors.getvalue())

    def test_windows_tree_cleanup_has_its_own_deadline(self):
        from unittest.mock import Mock
        for failure in (None, subprocess.TimeoutExpired("taskkill", 5), OSError("taskkill unavailable")):
            process = Mock(pid=123, returncode=-1)
            process.wait.side_effect = [subprocess.TimeoutExpired("fixture", 0.1), -1]
            with self.subTest(failure=failure), patch.dict(RUN.__globals__, os=SimpleNamespace(name="nt")):
                with patch("subprocess.Popen", return_value=process), patch("subprocess.run", side_effect=failure) as kill:
                    with self.assertRaises(subprocess.TimeoutExpired):
                        RUN(["fixture"], timeout=0.1)
                    kill.assert_called_once_with(["taskkill", "/PID", "123", "/T", "/F"], timeout=5, check=True)
                    process.kill.assert_called_once()
                    self.assertEqual(process.wait.call_args_list[-1].kwargs, {"timeout": 5})

    @unittest.skipIf(os.name == "nt", "POSIX failed process-group cleanup regression")
    def test_failed_cleanup_with_live_inherited_writer_only_reads_bounded_capture(self):
        output, errors = io.StringIO(), io.StringIO()
        popen, temporary_file = subprocess.Popen, tempfile.TemporaryFile
        writers, files, reads = [], [], []

        class GuardedCapture:
            def __enter__(self):
                self.file = temporary_file()
                files.append(self.file)
                return self

            def __exit__(self, *args):
                return self.file.__exit__(*args)

            def __getattr__(self, name):
                return getattr(self.file, name)

            def read(capture, size=-1):
                self.assertIsNone(writers[0].poll(), "inherited writer must survive cleanup")
                self.assertGreaterEqual(size, 0, "capture must never read to EOF")
                self.assertLessEqual(size, 4096, "capture must cap each stream")
                reads.append(size)
                return capture.file.read(size)

        with tempfile.TemporaryDirectory(prefix="crossroads-writer-") as temporary:
            ready = Path(temporary) / "ready"
            writer = (
                "import os,pathlib,time; "
                "os.write(1,b'x'*8192); os.write(2,b'y'*8192); "
                f"pathlib.Path({str(ready)!r}).touch(); "
                "\nwhile True:\n os.write(1,b'x'*256); os.write(2,b'y'*256); time.sleep(0.01)"
            )

            def start(arguments, **kwargs):
                # Keep the inherited writer owned by the suite so it can be reaped,
                # even when the supervisor's process-tree cleanup deliberately fails.
                writers.append(popen([sys.executable, "-c", writer],
                                     stdout=kwargs["stdout"], stderr=kwargs["stderr"]))
                deadline = time.monotonic() + 5
                while not ready.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(ready.exists(), "writer did not start")
                return popen(arguments, **kwargs)

            try:
                with patch.dict(RUN.__globals__, CAPTURE_LIMIT=4096), patch(
                        "tempfile.TemporaryFile", GuardedCapture), patch(
                        "subprocess.Popen", side_effect=start), patch(
                        "os.killpg", side_effect=OSError("tree cleanup unavailable")):
                    with redirect_stdout(output), redirect_stderr(errors), self.assertRaises(subprocess.TimeoutExpired):
                        RUN([sys.executable, "-c", "import time; time.sleep(60)"],
                            capture=True, timeout=0.1)
                self.assertEqual(reads, [4096, 4096])
                self.assertIn("x" * 4096, output.getvalue())
                self.assertIn("y" * 4096, errors.getvalue())
                self.assertIn("Tree cleanup failed", errors.getvalue())
                self.assertIn("stdout capture truncated", errors.getvalue())
                self.assertIn("stderr capture truncated", errors.getvalue())
                self.assertTrue(all(file.closed for file in files))
            finally:
                for process in writers:
                    process.kill()
                    process.wait(timeout=5)

    def test_capture_limit_and_truncation_for_success_and_failure(self):
        for code in (0, 7):
            for size in (0, 4095, 4096, 4097):
                with self.subTest(code=code, size=size):
                    output, errors = io.StringIO(), io.StringIO()
                    command = [sys.executable, "-c",
                               f"import os,sys; os.write(1,b'x'*{size}); os.write(2,b'y'*{size}); sys.exit({code})"]
                    with patch.dict(RUN.__globals__, CAPTURE_LIMIT=4096), redirect_stdout(output), redirect_stderr(errors):
                        if code:
                            with self.assertRaises(subprocess.CalledProcessError) as caught:
                                RUN(command, capture=True, timeout=2)
                            result = caught.exception
                            self.assertEqual(result.returncode, code)
                            self.assertIn("x" * min(size, 4096), output.getvalue())
                        else:
                            result = RUN(command, capture=True, timeout=2)
                        self.assertEqual(result.stdout if not code else result.output, b"x" * min(size, 4096))
                        self.assertEqual(result.stderr, b"y" * min(size, 4096))
                    for name in ("stdout", "stderr"):
                        self.assertEqual(f"{name} capture truncated" in errors.getvalue(), size > 4096)

    def test_capture_reads_only_the_size_snapshot_even_if_files_grow(self):
        temporary_file = tempfile.TemporaryFile

        class GrowingCapture:
            def __enter__(self):
                self.file = temporary_file()
                return self

            def __exit__(self, *args):
                return self.file.__exit__(*args)

            def __getattr__(self, name):
                return getattr(self.file, name)

            def read(capture, size=-1):
                self.assertEqual(size, 3, "read must use the size snapshot, not the cap or EOF")
                position = capture.file.tell()
                capture.file.seek(0, os.SEEK_END)
                capture.file.write(b"after snapshot")
                capture.file.seek(position)
                return capture.file.read(size)

        with patch("tempfile.TemporaryFile", GrowingCapture):
            result = RUN([sys.executable, "-c", "import os; os.write(1,b'out'); os.write(2,b'err')"],
                         capture=True, timeout=2)
        self.assertEqual(result.stdout, b"out")
        self.assertEqual(result.stderr, b"err")

    def test_acceptance_isolates_every_method_and_stops_on_first_failure(self):
        from unittest.mock import Mock
        acceptance = runpy.run_path(str(ROOT / "tests/test_windows_acceptance.py"))
        expected = unittest.defaultTestLoader.getTestCaseNames(acceptance["WindowsAcceptanceTests"])
        for failure in (None, subprocess.CalledProcessError(1, "fixture")):
            supervisor = Mock(side_effect=failure)
            with self.subTest(failure=failure), patch("runpy.run_path", return_value={"run": supervisor}):
                if failure:
                    with self.assertRaises(subprocess.CalledProcessError):
                        acceptance["native_suite"]()
                    self.assertEqual(supervisor.call_count, 1)
                else:
                    acceptance["native_suite"]()
                    self.assertEqual(supervisor.call_count, len(expected))
                    for name, call in zip(expected, supervisor.call_args_list):
                        self.assertEqual(call.args[0][-2:], ["-v", f"WindowsAcceptanceTests.{name}"])
                        self.assertEqual(call.kwargs, {"cwd": ROOT, "timeout": 30})

    def test_build_timeout_removes_all_selected_preparation_outputs(self):
        builder = runpy.run_path(str(ROOT / "scripts/build-windows.py"))["build"]
        with tempfile.TemporaryDirectory(prefix="crossroads-build-timeout-") as temporary:
            root = Path(temporary)
            stem = root / "target/builds/herdr-crossroads-0.2.0-x86_64-pc-windows-msvc"
            outputs = [Path(str(stem) + suffix) for suffix in (".tar.gz", ".tar.gz.sha256", ".provenance.json", ".link.map")]
            def command(arguments, **kwargs):
                if arguments[0] == "rustc":
                    return "rustc 1.99.0 fixture\nhost: x86_64-pc-windows-msvc\n"
                if arguments[0] == "cargo":
                    return "cargo 1.99.0 fixture\n"
                for output in outputs:
                    output.write_bytes(b"partial preparation")
                raise subprocess.TimeoutExpired(arguments, 0.1)
            def source(path):
                if Path(path).name == "install-binary.py":
                    return {"host_target": lambda: "x86_64-pc-windows-msvc"}
                return {"release_version": lambda: "0.2.0"}
            with patch.dict(builder.__globals__, ROOT=root, run=command), patch("os.chdir"), patch(
                    "platform.system", return_value="Windows"), patch("runpy.run_path", side_effect=source):
                with self.assertRaises(subprocess.TimeoutExpired):
                    builder()
            self.assertTrue(all(not output.exists() for output in outputs))


if __name__ == "__main__":
    unittest.main()
