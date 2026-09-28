use std::io::{self, Read, Write};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::os::{
    fd::{AsRawFd, FromRawFd, OwnedFd},
    unix::ffi::OsStrExt,
};
#[cfg(unix)]
use std::os::{unix::fs::OpenOptionsExt, unix::net::UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc,
};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use serde_json::Value;

use super::topology::{entries, field, Row};

pub(super) enum StatusMessage {
    Event(Value),
    Disconnected,
}

#[cfg(unix)]
thread_local! {
    pub(super) static RECONNECT_CANCEL: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(unix)]
pub(super) struct ReconnectAttempt {
    pub(super) rx: Receiver<io::Result<(StatusSubscription, Vec<Row>)>>,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) worker: Option<thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl Drop for ReconnectAttempt {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(unix)]
pub(super) struct StatusSubscription {
    pub(super) rx: Receiver<StatusMessage>,
    pub(super) socket: UnixStream,
}

#[cfg(unix)]
impl Drop for StatusSubscription {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}

// Reconnect CLI output goes to a file rather than a pipe: a stuck child (or a child
// retaining stdout after its parent exits) cannot keep a pipe reader thread alive.
pub(super) fn cli_output(
    executable: &std::ffi::OsStr,
    args: &[&str],
    bounded: bool,
) -> io::Result<std::process::Output> {
    if !bounded {
        return Command::new(executable).args(args).output();
    }
    #[cfg(unix)]
    {
        let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let temp_dir = Path::new("/tmp/opencode");
        let dir = if temp_dir.is_dir() {
            temp_dir.to_path_buf()
        } else {
            std::env::temp_dir()
        };
        let path = dir.join(format!(
            "crossroads-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let result = (|| {
            let mut child = Command::new(executable)
                .args(args)
                .stdout(Stdio::from(file))
                .stderr(Stdio::null())
                .spawn()?;
            let deadline = Instant::now() + Duration::from_secs(2);
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => {}
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(error);
                    }
                }
                if cancel
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Relaxed))
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "reconnect cancelled",
                    ));
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "reconnect CLI timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            };
            if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reconnect CLI output too large",
                ));
            }
            Ok(std::process::Output {
                status,
                stdout: std::fs::read(&path)?,
                stderr: Vec::new(),
            })
        })();
        let _ = std::fs::remove_file(path);
        result
    }
    #[cfg(not(unix))]
    unreachable!("bounded CLI is only used by Unix reconnects")
}

#[cfg(unix)]
pub(super) fn socket_line(stream: &mut UnixStream, deadline: Option<Instant>) -> io::Result<Value> {
    let mut bytes = Vec::new();
    let mut byte = [0];
    let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
    loop {
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        if let Some(deadline) = deadline {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "subscription ack timed out")
                })?;
            stream.set_read_timeout(Some(if cancel.is_some() {
                remaining.min(Duration::from_millis(20))
            } else {
                remaining
            }))?;
        }
        if bytes.len() >= 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "event line too long",
            ));
        }
        let read = match stream.read(&mut byte) {
            Err(error)
                if cancel.is_some()
                    && matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
            {
                continue
            }
            result => result?,
        };
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "event socket closed",
            ));
        }
        if byte[0] == b'\n' {
            return serde_json::from_slice(&bytes).map_err(io::Error::other);
        }
        bytes.push(byte[0]);
    }
}

#[cfg(unix)]
pub(super) fn pane_ids(executable: &std::ffi::OsStr, bounded: bool) -> io::Result<Vec<String>> {
    let output = cli_output(executable, &["pane", "list"], bounded)?;
    if !output.status.success() {
        return Err(io::Error::other("herdr pane list failed"));
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    if data["result"]["type"] != "pane_list" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a pane list",
        ));
    }
    let panes = entries(&data["result"], "panes")?;
    let mut ids = Vec::new();
    for pane in panes {
        let id = field(pane, "pane_id")?;
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

// The Linux x86_64 socket constants and sockaddr_un layout match the local
// sys/socket.h and sys/un.h ABI; other targets keep the existing path.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn reconnect_socket(path: &std::ffi::OsStr) -> io::Result<UnixStream> {
    #[repr(C)]
    struct SocketAddress {
        family: u16,
        path: [u8; 108],
    }
    unsafe extern "C" {
        fn socket(
            domain: std::ffi::c_int,
            kind: std::ffi::c_int,
            protocol: std::ffi::c_int,
        ) -> std::ffi::c_int;
        fn connect(
            fd: std::ffi::c_int,
            address: *const std::ffi::c_void,
            length: u32,
        ) -> std::ffi::c_int;
    }
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes.len() >= 108 || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Unix socket path",
        ));
    }
    let mut address = SocketAddress {
        family: 1,
        path: [0; 108],
    }; // AF_UNIX
    address.path[..bytes.len()].copy_from_slice(bytes);
    let length = (std::mem::offset_of!(SocketAddress, path) + bytes.len() + 1) as u32;
    let deadline = Instant::now() + Duration::from_millis(500);
    let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
    loop {
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "reconnect connect timed out",
            ));
        }
        // SAFETY: these are the verified Linux x86_64 AF_UNIX, SOCK_STREAM,
        // SOCK_NONBLOCK and SOCK_CLOEXEC constants; socket returns an owned fd.
        let raw = unsafe { socket(1, 1 | 0o4000 | 0o2000000, 0) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: socket returned a fresh valid descriptor, transferred exactly once.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: address is a live repr(C) sockaddr_un prefix, length includes
        // its terminating NUL and never exceeds the allocated address.
        let result = unsafe {
            connect(
                fd.as_raw_fd(),
                (&address as *const SocketAddress).cast(),
                length,
            )
        };
        if result == 0 {
            let stream = UnixStream::from(fd);
            stream.set_nonblocking(false)?;
            if cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "reconnect cancelled",
                ));
            }
            return Ok(stream);
        }
        let error = io::Error::last_os_error();
        // A failed nonblocking connect leaves socket state unspecified: drop it
        // and create a fresh socket for every retry, including EAGAIN.
        drop(fd);
        if error.raw_os_error() != Some(11) {
            return Err(error);
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(20)),
        );
    }
}

#[cfg(unix)]
pub(super) fn subscribe(
    executable: &std::ffi::OsStr,
    bounded: bool,
) -> io::Result<StatusSubscription> {
    let path = std::env::var_os("HERDR_SOCKET_PATH")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERDR_SOCKET_PATH missing"))?;
    let ids = pane_ids(executable, bounded)?;
    let mut stream = if bounded {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            reconnect_socket(&path)?
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            UnixStream::connect(&path)?
        }
    } else {
        UnixStream::connect(&path)?
    };
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({
            "id":"crossroads-status", "method":"events.subscribe",
            "params":{"subscriptions":ids.iter().map(|id| serde_json::json!({"type":"pane.agent_status_changed","pane_id":id})).collect::<Vec<_>>()}
        }),
    )?;
    stream.write_all(b"\n")?;
    let reply = socket_line(
        &mut stream,
        Some(Instant::now() + Duration::from_millis(500)),
    )?;
    if reply["id"] != "crossroads-status" || reply["result"]["type"] != "subscription_started" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "status subscription rejected",
        ));
    }
    stream.set_read_timeout(None)?;
    let socket = stream.try_clone()?;
    let (tx, rx): (SyncSender<StatusMessage>, _) = mpsc::sync_channel(1024);
    thread::spawn(move || {
        while let Ok(event) = socket_line(&mut stream, None) {
            if tx.send(StatusMessage::Event(event)).is_err() {
                return;
            }
        }
        let _ = tx.send(StatusMessage::Disconnected);
    });
    Ok(StatusSubscription { rx, socket })
}
