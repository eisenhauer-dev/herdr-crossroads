"""Bound preparation commands and their child trees; never wait on inherited pipes."""
import os
import signal
import subprocess
import sys
import tempfile
import time


COMMAND_TIMEOUT = 300
# Retain diagnostic prefixes without allowing noisy commands to exhaust memory.
CAPTURE_LIMIT = 1024 * 1024


def run(arguments, *, cwd=None, env=None, capture=False, timeout=None):
    timeout = COMMAND_TIMEOUT if timeout is None else timeout
    started = time.monotonic()
    print(f"START ({timeout}s deadline): {subprocess.list2cmdline([str(a) for a in arguments])}", flush=True)
    # Files, rather than PIPE/communicate, keep orphaned inherited writers from
    # holding the supervisor open after its deadline.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(arguments, cwd=cwd, env=env,
                                   stdout=stdout if capture else None,
                                   stderr=stderr if capture else None,
                                   start_new_session=os.name != "nt")
        try:
            process.wait(timeout=timeout)
        except BaseException:
            print(f"STOP: command tree pid={process.pid} exceeded deadline or was interrupted", file=sys.stderr, flush=True)
            try:
                if os.name == "nt":
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                   timeout=5, check=True)
                else:
                    os.killpg(process.pid, signal.SIGKILL)
            except (OSError, subprocess.SubprocessError) as error:
                print(f"Tree cleanup failed: {error}", file=sys.stderr, flush=True)
            finally:
                process.kill()
                process.wait(timeout=5)
            raise
        finally:
            print(f"END pid={process.pid} code={process.returncode} elapsed={time.monotonic() - started:.2f}s", flush=True)
            if capture:
                sizes = [os.fstat(stream.fileno()).st_size for stream in (stdout, stderr)]
                stdout.seek(0)
                stderr.seek(0)
                output, errors = [stream.read(min(size, CAPTURE_LIMIT))
                                  for stream, size in zip((stdout, stderr), sizes)]
                for name, size in zip(("stdout", "stderr"), sizes):
                    if size > CAPTURE_LIMIT:
                        print(f"{name} capture truncated: retained first {CAPTURE_LIMIT} of {size} snapshot bytes",
                              file=sys.stderr, flush=True)
                if process.returncode != 0:
                    for stream, data in ((sys.stdout, output), (sys.stderr, errors)):
                        stream.write(data.decode("utf-8", "backslashreplace"))
                        stream.flush()
        result = subprocess.CompletedProcess(arguments, process.returncode,
                                             output if capture else None, errors if capture else None)
        result.check_returncode()
        return result
