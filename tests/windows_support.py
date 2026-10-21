"""ctypes-only native ConPTY/byte-pipe fixtures; no Herdr/user configuration."""
import ctypes as C
from ctypes import wintypes as W
import os
from pathlib import Path
import queue
import subprocess
import threading
import time


K = C.WinDLL("kernel32", use_last_error=True)
HANDLE = W.HANDLE
INVALID = C.c_void_p(-1).value


def api(name, result, *arguments):
    function = getattr(K, name)
    function.restype = result
    function.argtypes = list(arguments)
    return function


close = api("CloseHandle", W.BOOL, HANDLE)
read = api("ReadFile", W.BOOL, HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p)
write = api("WriteFile", W.BOOL, HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p)
create_pipe = api("CreatePipe", W.BOOL, C.POINTER(HANDLE), C.POINTER(HANDLE), C.c_void_p, W.DWORD)
create_named = api("CreateNamedPipeW", HANDLE, W.LPCWSTR, W.DWORD, W.DWORD, W.DWORD,
                   W.DWORD, W.DWORD, W.DWORD, C.c_void_p)
connect_named = api("ConnectNamedPipe", W.BOOL, HANDLE, C.c_void_p)
disconnect_named = api("DisconnectNamedPipe", W.BOOL, HANDLE)
wait = api("WaitForSingleObject", W.DWORD, HANDLE, W.DWORD)
exit_code = api("GetExitCodeProcess", W.BOOL, HANDLE, C.POINTER(W.DWORD))
terminate = api("TerminateProcess", W.BOOL, HANDLE, W.UINT)


class Coord(C.Structure):
    _fields_ = [("X", C.c_short), ("Y", C.c_short)]


class Startup(C.Structure):
    _fields_ = [("cb", W.DWORD), ("reserved", W.LPWSTR), ("desktop", W.LPWSTR),
                ("title", W.LPWSTR), ("x", W.DWORD), ("y", W.DWORD), ("xs", W.DWORD),
                ("ys", W.DWORD), ("xc", W.DWORD), ("yc", W.DWORD), ("fill", W.DWORD),
                ("flags", W.DWORD), ("show", W.WORD), ("reserved_size", W.WORD),
                ("reserved_data", C.c_void_p), ("stdin", HANDLE), ("stdout", HANDLE), ("stderr", HANDLE)]


class StartupEx(C.Structure):
    _fields_ = [("startup", Startup), ("attributes", C.c_void_p)]


class ProcessInfo(C.Structure):
    _fields_ = [("process", HANDLE), ("thread", HANDLE), ("pid", W.DWORD), ("tid", W.DWORD)]


create_console = api("CreatePseudoConsole", C.c_long, Coord, HANDLE, HANDLE, W.DWORD, C.POINTER(HANDLE))
close_console = api("ClosePseudoConsole", None, HANDLE)
resize_console = api("ResizePseudoConsole", C.c_long, HANDLE, Coord)
init_attributes = api("InitializeProcThreadAttributeList", W.BOOL, C.c_void_p, W.DWORD, W.DWORD, C.POINTER(C.c_size_t))
update_attributes = api("UpdateProcThreadAttribute", W.BOOL, C.c_void_p, W.DWORD, C.c_size_t,
                        C.c_void_p, C.c_size_t, C.c_void_p, C.c_void_p)
delete_attributes = api("DeleteProcThreadAttributeList", None, C.c_void_p)
create_process = api("CreateProcessW", W.BOOL, W.LPCWSTR, W.LPWSTR, C.c_void_p, C.c_void_p,
                     W.BOOL, W.DWORD, C.c_void_p, W.LPCWSTR, C.POINTER(StartupEx), C.POINTER(ProcessInfo))


def checked(ok):
    if not ok:
        raise C.WinError(C.get_last_error())


def send(handle, data):
    offset = 0
    while offset < len(data):
        count = W.DWORD()
        checked(write(handle, data[offset:], len(data) - offset, C.byref(count), None))
        if count.value == 0:
            raise OSError("zero-length pipe write")
        offset += count.value


def receive(handle, size=4096):
    buffer = C.create_string_buffer(size)
    count = W.DWORD()
    if not read(handle, buffer, size, C.byref(count), None):
        if C.get_last_error() in (109, 232, 233):
            return b""
        raise C.WinError(C.get_last_error())
    return buffer.raw[:count.value]


class NamedServer:
    def __init__(self, marker, first=True):
        self.handle = create_named("\\\\.\\pipe\\" + marker, 3 | (0x80000 if first else 0), 0, 255, 65536, 65536, 0, None)
        if self.handle == INVALID:
            raise C.WinError(C.get_last_error())

    def accept(self):
        if not connect_named(self.handle, None) and C.get_last_error() != 535:
            raise C.WinError(C.get_last_error())

    def line(self):
        data = bytearray()
        while len(data) < 65536:
            byte = receive(self.handle, 1)
            if byte == b"\n":
                return bytes(data)
            if not byte:
                raise EOFError("client disconnected")
            data.extend(byte)
        raise ValueError("fixture request too long")

    def send(self, data):
        send(self.handle, data)

    def disconnect(self):
        checked(disconnect_named(self.handle))

    def close(self):
        close(self.handle)


class Console:
    def __init__(self, command, env, cwd):
        self.console = HANDLE()
        self.process = None
        self.output = bytearray()
        self.queue = queue.Queue()
        input_read, self.input = HANDLE(), HANDLE()
        self.read_handle, output_write = HANDLE(), HANDLE()
        checked(create_pipe(C.byref(input_read), C.byref(self.input), None, 0))
        checked(create_pipe(C.byref(self.read_handle), C.byref(output_write), None, 0))
        hr = create_console(Coord(120, 30), input_read, output_write, 0, C.byref(self.console))
        close(input_read)
        close(output_write)
        if hr < 0:
            raise OSError(f"CreatePseudoConsole HRESULT {hr:#x}")
        size = C.c_size_t()
        init_attributes(None, 1, 0, C.byref(size))
        attributes = C.create_string_buffer(size.value)
        checked(init_attributes(attributes, 1, 0, C.byref(size)))
        try:
            checked(update_attributes(attributes, 0, 0x20016, self.console,
                                      C.sizeof(HANDLE), None, None))
            startup = StartupEx()
            startup.startup.cb = C.sizeof(StartupEx)
            startup.attributes = C.cast(attributes, C.c_void_p)
            info = ProcessInfo()
            environment = C.create_unicode_buffer("\0".join(f"{k}={v}" for k, v in sorted(env.items(), key=lambda p: p[0].upper())) + "\0\0")
            line = C.create_unicode_buffer(subprocess.list2cmdline([str(v) for v in command]))
            checked(create_process(str(command[0]), line, None, None, False, 0x80000 | 0x400,
                                   environment, str(cwd), C.byref(startup), C.byref(info)))
            self.process = info.process
            close(info.thread)
        finally:
            delete_attributes(attributes)
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()

    def _read(self):
        try:
            while data := receive(self.read_handle):
                self.queue.put(data)
        except OSError as error:
            self.queue.put(error)

    def send(self, data):
        send(self.input, data)

    def until(self, text, timeout=5):
        deadline = time.monotonic() + timeout
        while text not in self.output:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise AssertionError(f"missing {text!r}: {bytes(self.output)[-4000:]!r}")
            data = self.queue.get(timeout=remaining)
            if isinstance(data, Exception):
                raise data
            self.output.extend(data)
        return bytes(self.output)

    def reset_output(self):
        while not self.queue.empty():
            self.output.extend(self.queue.get_nowait())
        self.output.clear()

    def exited(self, timeout=5):
        if wait(self.process, int(timeout * 1000)) != 0:
            raise AssertionError("native child did not exit within deadline")
        code = W.DWORD()
        checked(exit_code(self.process, C.byref(code)))
        return code.value

    def resize(self, width, height):
        hr = resize_console(self.console, Coord(width, height))
        if hr < 0:
            raise OSError(f"ResizePseudoConsole HRESULT {hr:#x}")

    def close(self):
        if self.process:
            if wait(self.process, 0) != 0:
                terminate(self.process, 1)
                wait(self.process, 5000)
            close(self.process)
            self.process = None
        close(self.input)
        close_console(self.console)
        self.reader.join(timeout=5)
        close(self.read_handle)
