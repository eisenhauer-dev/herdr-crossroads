use std::io;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use serde_json::Value;

use super::presentation::draw;
use super::selection::Ui;

#[derive(Clone, Copy)]
pub(super) enum Key {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
}

pub(super) struct Keys {
    pub(super) escape: u8,
    utf8: Vec<u8>,
}
impl Keys {
    pub(super) fn new() -> Self {
        Self {
            escape: 0,
            utf8: Vec::new(),
        }
    }
    pub(super) fn feed(&mut self, byte: u8) -> Option<Key> {
        if self.escape == 1 {
            self.escape = 0;
            if byte == b'[' {
                self.escape = 2;
                return None;
            }
            return Some(Key::Esc);
        }
        if self.escape == 2 {
            self.escape = 0;
            return match byte {
                b'A' => Some(Key::Up),
                b'B' => Some(Key::Down),
                b'C' => Some(Key::Right),
                b'D' => Some(Key::Left),
                b'Z' => Some(Key::BackTab),
                _ => None,
            };
        }
        match byte {
            27 => {
                self.escape = 1;
                None
            }
            b'\r' | b'\n' => Some(Key::Enter),
            b'\t' => Some(Key::Tab),
            127 | 8 => Some(Key::Backspace),
            _ => {
                self.utf8.push(byte);
                match std::str::from_utf8(&self.utf8) {
                    Ok(s) => {
                        let c = s.chars().next().map(Key::Char);
                        self.utf8.clear();
                        c
                    }
                    Err(e) if e.error_len().is_none() && self.utf8.len() < 4 => None,
                    Err(_) => {
                        self.utf8.clear();
                        None
                    }
                }
            }
        }
    }
    pub(super) fn flush(&mut self) -> Option<Key> {
        if self.escape == 1 {
            self.escape = 0;
            Some(Key::Esc)
        } else {
            None
        }
    }
}

#[cfg(unix)]
pub(super) fn focus(pane_id: &str) -> io::Result<()> {
    let path = std::env::var_os("HERDR_SOCKET_PATH")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERDR_SOCKET_PATH is missing"))?;
    let mut stream = UnixStream::connect(path)?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":pane_id}}),
    )?;
    stream.write_all(b"\n")?;
    let deadline = Instant::now() + Duration::from_millis(500);
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
        stream.set_read_timeout(Some(remaining))?;
        let mut byte = [0];
        let n = stream.read(&mut byte).map_err(|error| {
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) {
                io::Error::new(io::ErrorKind::TimedOut, "pane.focus reply timed out")
            } else {
                error
            }
        })?;
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

#[cfg(not(unix))]
pub(super) fn focus(_pane_id: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "pane focus requires a Unix socket",
    ))
}

pub(super) fn handle_key(ui: &mut Ui, key: Key, terminal: bool) -> io::Result<()> {
    if let Some(id) = ui.key(key) {
        focus(&id)?;
    }
    if !ui.closed {
        draw(ui, terminal)?;
    }
    Ok(())
}
