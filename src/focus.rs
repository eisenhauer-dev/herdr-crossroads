#[cfg(windows)]
use crate::windows_pipe::Pipe as LocalStream;
use std::io;
#[cfg(any(unix, windows))]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream as LocalStream;
#[cfg(any(unix, windows))]
use std::time::{Duration, Instant};

#[cfg(any(unix, windows))]
use serde_json::Value;

#[cfg(any(unix, windows))]
pub(super) fn focus(pane_id: &str) -> io::Result<()> {
    let path = std::env::var_os("HERDR_SOCKET_PATH")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERDR_SOCKET_PATH is missing"))?;
    let mut stream = LocalStream::connect(path)?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":pane_id}}),
    )?;
    stream.write_all(b"\n")?;
    let deadline = Instant::now() + Duration::from_millis(500);
    stream.set_nonblocking(true)?;
    let mut reply = Vec::new();
    loop {
        if reply.len() >= 16 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pane.focus reply too long",
            ));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "pane.focus reply timed out"))?;
        let mut byte = [0];
        let n = match stream.read(&mut byte) {
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(remaining.min(Duration::from_millis(1)));
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "pane.focus reply timed out",
            ));
        }
        if n == 0 {
            break;
        }
        reply.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    let response: Value = serde_json::from_slice(&reply).map_err(io::Error::other)?;
    if response["id"] != "crossroads-focus"
        || response["error"].is_object()
        || response["result"]["type"] != "pane_info"
        || response["result"]["pane"]["pane_id"] != pane_id
        || response["result"]["pane"]["focused"] != true
    {
        return Err(io::Error::other(format!("pane.focus failed: {response}")));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn focus(_pane_id: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "pane focus requires a Unix socket",
    ))
}
