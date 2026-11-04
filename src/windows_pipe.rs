//! Herdr 0.9.5 / interprocess 2.4.4 GenericNamespaced byte-pipe transport.
use std::cell::Cell;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetNamedPipeHandleState(
        pipe: *mut std::ffi::c_void,
        mode: *const u32,
        max_count: *const u32,
        timeout: *const u32,
    ) -> i32;
    fn PeekNamedPipe(
        pipe: *mut std::ffi::c_void,
        buffer: *mut std::ffi::c_void,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        remaining: *mut u32,
    ) -> i32;
}

pub(crate) fn pipe_name(marker: &OsStr) -> io::Result<Vec<u16>> {
    // Herdr passes Path::to_string_lossy() to GenericNamespaced. Interprocess
    // prepends this literal even when the marker already resembles a pipe path.
    let marker = marker.to_string_lossy();
    if marker.is_empty() || marker.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Herdr pipe marker",
        ));
    }
    Ok(format!(r"\\.\pipe\{marker}")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}

#[derive(Debug)]
pub(crate) struct Pipe {
    file: File,
    stopped: Arc<AtomicBool>,
    read_timeout: Cell<Option<Duration>>,
    write_timeout: Cell<Option<Duration>>,
    nonblocking: Cell<bool>,
}

fn cancelled() -> bool {
    crate::transport::RECONNECT_CANCEL.with(|flag| {
        flag.borrow()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    })
}

impl Pipe {
    pub(crate) fn connect(marker: impl AsRef<OsStr>) -> io::Result<Self> {
        use std::os::windows::ffi::OsStringExt;
        let name = pipe_name(marker.as_ref())?;
        let path = std::ffi::OsString::from_wide(&name[..name.len() - 1]);
        let deadline = Instant::now() + Duration::from_millis(500);
        let file = loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "reconnect cancelled",
                ));
            }
            // SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION: do not permit a
            // pipe server to impersonate this client. Never open the marker file.
            match OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(0x0011_0000)
                .open(&path)
            {
                Ok(file) => break file,
                Err(error) if error.raw_os_error() == Some(231) && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        };
        // PIPE_READMODE_BYTE | PIPE_NOWAIT. Both reads and writes must remain
        // bounded/cancellable; synchronous WriteFile on a full pipe can block.
        let mode = 1u32;
        // SAFETY: file owns a live duplex named-pipe handle; mode is a valid
        // readable DWORD, and the optional collection parameters are absent.
        if unsafe {
            SetNamedPipeHandleState(
                file.as_raw_handle(),
                &mode,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            file,
            stopped: Arc::new(AtomicBool::new(false)),
            read_timeout: Cell::new(None),
            write_timeout: Cell::new(Some(Duration::from_millis(500))),
            nonblocking: Cell::new(false),
        })
    }

    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.read_timeout.set(timeout);
        Ok(())
    }
    pub(crate) fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.write_timeout.set(timeout);
        Ok(())
    }
    pub(crate) fn set_nonblocking(&self, enabled: bool) -> io::Result<()> {
        self.nonblocking.set(enabled);
        Ok(())
    }
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            stopped: self.stopped.clone(),
            read_timeout: self.read_timeout.clone(),
            write_timeout: self.write_timeout.clone(),
            nonblocking: self.nonblocking.clone(),
        })
    }
    pub(crate) fn shutdown(&self, _how: std::net::Shutdown) -> io::Result<()> {
        self.stopped.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn wait(&self, deadline: Option<Instant>) -> io::Result<()> {
        if self.stopped.load(Ordering::Relaxed) || cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "pipe operation cancelled",
            ));
        }
        if self.nonblocking.get() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        if deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        std::thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}

impl Read for Pipe {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let deadline = self
            .read_timeout
            .get()
            .map(|timeout| Instant::now() + timeout);
        loop {
            if self.stopped.load(Ordering::Relaxed) || cancelled() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let mut available = 0u32;
            // SAFETY: the live handle is queried without a data buffer; only
            // available is written. There is exactly one reader per subscription.
            let ok = unsafe {
                PeekNamedPipe(
                    self.file.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                if matches!(error.raw_os_error(), Some(109 | 232 | 233)) {
                    return Ok(0);
                }
                return Err(error);
            }
            if available > 0 {
                let count = bytes.len().min(available as usize);
                return self.file.read(&mut bytes[..count]);
            }
            self.wait(deadline)?;
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let deadline = self
            .write_timeout
            .get()
            .map(|timeout| Instant::now() + timeout);
        loop {
            if self.stopped.load(Ordering::Relaxed) || cancelled() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            match self.file.write(bytes) {
                Ok(0) => self.wait(deadline)?,
                Err(error) if error.raw_os_error() == Some(232) => self.wait(deadline)?,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
