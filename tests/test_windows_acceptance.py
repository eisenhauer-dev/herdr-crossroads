"""Native fake-host ConPTY gates, not real-Herdr popup or Windows 10 certification."""
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import threading
import time
import unittest


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.name == "nt", "requires native Windows ConPTY and named pipes")
class WindowsAcceptanceTests(unittest.TestCase):
    def setUp(self):
        from windows_support import Console, NamedServer
        self.Console, self.NamedServer = Console, NamedServer
        self.temporary = tempfile.TemporaryDirectory(prefix="crossroads-native-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.binary = Path(os.environ["CROSSROADS_WINDOWS_BINARY"]).resolve()
        self.herdr = Path(os.environ["CROSSROADS_WINDOWS_TEST_HERDR"]).resolve()
        self.log = self.base / "cli.log"
        self.env = {key: value for key, value in os.environ.items()
                    if not key.startswith(("HERDR_", "SSH_", "CROSSROADS_TEST_"))}
        self.env.update(HERDR_BIN_PATH=str(self.herdr), HERDR_PANE_ID="p-self",
                        HERDR_CONFIG_PATH=str(self.base / "missing-config.toml"),
                        HERDR_PLUGIN_CONFIG_DIR=str(self.base / "plugin-config"),
                        CROSSROADS_TEST_LOG=str(self.log))
        self.errors = []

    def console(self, command=None):
        console = self.Console(command or [self.binary], self.env, ROOT)
        self.addCleanup(console.close)
        return console

    def worker(self, function):
        def run():
            try:
                function()
            except Exception as error:
                self.errors.append(error)
        thread = threading.Thread(target=run, daemon=True)
        thread.start()
        return thread

    def joined(self, thread):
        thread.join(timeout=5)
        self.assertFalse(thread.is_alive(), "native fixture worker did not stop")
        if self.errors:
            raise self.errors[0]

    def test_console_modes_restore_inside_conpty(self):
        executable = Path(os.environ["CROSSROADS_WINDOWS_CONSOLE_TEST"]).resolve()
        console = self.console([executable, "--ignored", "--exact", "console_modes_and_size_are_restored_exactly", "--nocapture"])
        console.until(b"test result:")
        self.assertEqual(console.exited(), 0, bytes(console.output).decode("utf-8", "backslashreplace"))
        console.until(b"1 passed")

    def test_console_handles_ignore_redirected_parent_stdio(self):
        # Force redirected stdin/stdout/stderr regardless of the launching shell.
        # The nested test must receive its Rust output through ConPTY, not here.
        # Keep the outer 30s watchdog responsible for killing the whole tree.
        output = subprocess.run(
            [sys.executable, "-B", __file__, "-v",
             "WindowsAcceptanceTests.test_console_modes_restore_inside_conpty"],
            input=b"", capture_output=True, env=self.env, cwd=ROOT)
        self.assertEqual(output.returncode, 0, (output.stdout, output.stderr))

    def test_search_help_themes_navigation_resize_and_close(self):
        console = self.console()
        console.until(b"Claude")
        for key, expected in [(b"?", b"Help"), (b"q", b"Claude"),
                              (b"t", b"Themes"), (b"j", b"preview"),
                              (b" ", b"Themes"), (b"q", b"Claude"),
                              (b"/cod", b"Search"), (b"\r", b"codex"),
                              (b"r\x1b[B\x1b[C\t\x1b[Z", b"Claude")]:
            console.reset_output()
            console.send(key)
            console.until(expected)
        console.reset_output()
        console.resize(60, 12)
        console.until(b"too small")
        console.reset_output()
        console.resize(120, 30)
        console.until(b"Claude")
        console.send(b"q")
        self.assertEqual(console.exited(timeout=2), 0)
        self.assertEqual(self.log.read_text().count("api snapshot"), 1)

    def test_filtered_enter_focuses_stable_id_not_same_label(self):
        marker = f"crossroads-native-focus-{os.getpid()}"
        status = self.NamedServer(marker)
        focus = self.NamedServer(marker, first=False)
        self.addCleanup(status.close)
        self.addCleanup(focus.close)
        self.env["HERDR_SOCKET_PATH"] = marker
        stop = threading.Event()
        self.addCleanup(stop.set)
        def subscription():
            status.accept()
            self.assertEqual(json.loads(status.line())["method"], "events.subscribe")
            status.send(b'{"id":"crossroads-status","result":{"type":"subscription_started"}}\n')
            stop.wait(10)
        subscription_worker = self.worker(subscription)
        def focused():
            focus.accept()
            request = json.loads(focus.line())
            self.assertEqual(request["method"], "pane.focus")
            self.assertEqual(request["params"], {"pane_id": "p-other"})
            focus.send(b'{"id":"crossroads-focus","result":{"type":"pane_info","pane":{"pane_id":"p-other","focused":true}}}\n')
        focus_worker = self.worker(focused)
        console = self.console()
        console.until(b"Claude")
        console.send(b"/codex\r")
        console.until(b"codex")
        console.send(b"\r")
        self.assertEqual(console.exited(timeout=2), 0)
        self.joined(focus_worker)
        stop.set()
        self.joined(subscription_worker)

    def test_live_status_disconnect_reconnect_resnapshot_and_responsive_quit(self):
        marker = f"crossroads-native-status-{os.getpid()}"
        server = self.NamedServer(marker)
        self.addCleanup(server.close)
        self.env["HERDR_SOCKET_PATH"] = marker
        drop = threading.Event()
        stop = threading.Event()
        self.addCleanup(stop.set)
        self.addCleanup(drop.set)
        def status():
            for generation in range(2):
                server.accept()
                request = json.loads(server.line())
                self.assertEqual(request["method"], "events.subscribe")
                self.assertEqual(request["params"]["subscriptions"][0]["pane_id"], "p-b")
                server.send(b'{"id":"crossroads-status","result":{"type":"subscription_started"}}\n')
                event = {"event": "pane.agent_status_changed", "data": {
                    "pane_id": "p-b", "workspace_id": "w1", "agent_status": "blocked" if generation == 0 else "done"}}
                server.send(json.dumps(event).encode() + b"\n")
                if generation == 0:
                    if not drop.wait(10):
                        raise AssertionError("test never requested disconnect")
                    server.disconnect()
                else:
                    stop.wait(10)
        worker = self.worker(status)
        console = self.console()
        console.until(b"blocked")
        console.reset_output()
        drop.set()
        console.until(b"done", timeout=8)
        deadline = time.monotonic() + 3
        while self.log.read_text().count("api snapshot") < 2 and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertGreaterEqual(self.log.read_text().count("api snapshot"), 2)
        self.assertGreaterEqual(self.log.read_text().count("pane list"), 2)
        console.send(b"q")
        self.assertEqual(console.exited(timeout=2), 0)
        stop.set()
        self.joined(worker)

    def test_noninteractive_startup_eof_and_missing_cli(self):
        output = subprocess.run([self.binary], input=b"q", env=self.env, capture_output=True, timeout=5)
        self.assertEqual(output.returncode, 0, output.stderr)
        self.assertIn(b"Claude", output.stdout)
        output = subprocess.run([self.binary], input=b"", env=self.env, capture_output=True, timeout=5)
        self.assertEqual(output.returncode, 0, output.stderr)
        self.env["HERDR_BIN_PATH"] = str(self.base / "missing.exe")
        output = subprocess.run([self.binary], input=b"q", env=self.env, capture_output=True, timeout=5)
        self.assertNotEqual(output.returncode, 0)
        self.assertNotIn(b"Claude", output.stdout)

    def test_theme_save_replace_reopen_and_auto_removal(self):
        directory = Path(self.env["HERDR_PLUGIN_CONFIG_DIR"])
        path = directory / "theme.jsonc"
        for key in (b"tj\rq", b"tjj\rq"):
            console = self.console()
            console.until(b"Claude")
            console.send(key)
            self.assertEqual(console.exited(), 0)
            self.assertTrue(path.is_file())
            self.assertIn(json.loads(path.read_text())["theme"], ("catppuccin", "catppuccin-latte", "terminal"))
        console = self.console()
        console.until(b"Claude")
        console.send(b"t")
        console.until(b"selected: terminal")
        console.send(b"kkk\rq")
        self.assertEqual(console.exited(), 0)
        self.assertFalse(path.exists())


def native_suite():
    # A blocked native API (including fixture cleanup) cannot trap the suite.
    supervise = runpy.run_path(str(ROOT / "scripts/windows-process.py"))["run"]
    for name in unittest.defaultTestLoader.getTestCaseNames(WindowsAcceptanceTests):
        supervise([sys.executable, "-B", __file__, "-v", f"WindowsAcceptanceTests.{name}"],
                  cwd=ROOT, timeout=30)


if __name__ == "__main__":
    if os.name == "nt" and len(sys.argv) == 1:
        native_suite()
    else:
        unittest.main(verbosity=2)
