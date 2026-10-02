use std::io;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::time::{Duration, Instant};
use unicode_general_category::{get_general_category, GeneralCategory};

#[cfg(unix)]
use serde_json::Value;

use super::selection::Ui;
use super::terminal::draw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Ctrl(char),
    Alt(char),
    CtrlArrow(Arrow),
    AltArrow(Arrow),
}

impl Key {
    pub(super) fn parse(text: &str) -> Option<Self> {
        if text.chars().count() == 1 {
            return text
                .chars()
                .next()
                .filter(|c| {
                    *c == ' '
                        || !matches!(
                            get_general_category(*c),
                            GeneralCategory::NonspacingMark
                                | GeneralCategory::SpacingMark
                                | GeneralCategory::EnclosingMark
                                | GeneralCategory::Control
                                | GeneralCategory::Format
                                | GeneralCategory::Surrogate
                                | GeneralCategory::PrivateUse
                                | GeneralCategory::Unassigned
                                | GeneralCategory::SpaceSeparator
                                | GeneralCategory::LineSeparator
                                | GeneralCategory::ParagraphSeparator
                        )
                })
                .map(Self::Char);
        }
        let plain = match text {
            "Enter" => Self::Enter,
            "Esc" => Self::Esc,
            "Backspace" => Self::Backspace,
            "Up" => Self::Up,
            "Down" => Self::Down,
            "Left" => Self::Left,
            "Right" => Self::Right,
            "Tab" => Self::Tab,
            "Shift+Tab" => Self::BackTab,
            "Space" => Self::Char(' '),
            _ => return None,
        };
        Some(plain)
    }
    pub(super) fn label(self) -> String {
        match self {
            Self::Char(' ') => "Space".into(),
            Self::Char(c) => c.to_string(),
            Self::Ctrl(c) => format!("Ctrl+{c}"),
            Self::Alt(c) => format!("Alt+{c}"),
            Self::CtrlArrow(a) => format!("Ctrl+{a:?}"),
            Self::AltArrow(a) => format!("Alt+{a:?}"),
            Self::BackTab => "Shift+Tab".into(),
            other => format!("{other:?}"),
        }
    }

    pub(super) fn is_arrow(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::CtrlArrow(_)
                | Self::AltArrow(_)
        )
    }

    pub(super) fn display_label(self) -> String {
        let (modifier, arrow) = match self {
            Self::Up => ("", Arrow::Up),
            Self::Down => ("", Arrow::Down),
            Self::Left => ("", Arrow::Left),
            Self::Right => ("", Arrow::Right),
            Self::CtrlArrow(arrow) => ("Ctrl+", arrow),
            Self::AltArrow(arrow) => ("Alt+", arrow),
            Self::Char('/' | '|') => return format!("'{}'", self.label()),
            _ => return self.label(),
        };
        format!(
            "{modifier}{}",
            match arrow {
                Arrow::Up => "↑",
                Arrow::Down => "↓",
                Arrow::Left => "←",
                Arrow::Right => "→",
            }
        )
    }
}

pub(super) struct Keys {
    pub(super) escape: u8,
    utf8: Vec<u8>,
    sequence: Vec<u8>,
    alt: bool,
    pub(super) kitty_response: bool,
    pub(super) escape_started: Option<std::time::Instant>,
}
impl Keys {
    pub(super) fn new() -> Self {
        Self {
            escape: 0,
            utf8: Vec::new(),
            sequence: Vec::new(),
            alt: false,
            kitty_response: false,
            escape_started: None,
        }
    }
    pub(super) fn feed(&mut self, byte: u8) -> Option<Key> {
        if self.escape == 1 {
            if byte == 27 {
                self.escape_started = Some(std::time::Instant::now());
                return Some(Key::Esc);
            }
            self.escape = 0;
            if byte == b'[' || byte == b'O' {
                self.escape = if byte == b'[' { 2 } else { 3 };
                self.sequence.clear();
                return None;
            }
            self.escape_started = None;
            self.alt = true;
        }
        if self.escape >= 2 {
            self.sequence.push(byte);
            if !(0x40..=0x7e).contains(&byte) {
                if self.sequence.len() > 128 {
                    self.escape = 0;
                    self.sequence.clear();
                    self.escape_started = None;
                }
                return None;
            }
            self.escape = 0;
            self.escape_started = None;
            let sequence = String::from_utf8(std::mem::take(&mut self.sequence)).ok()?;
            if byte == b'u' && sequence.starts_with('?') {
                self.kitty_response = sequence[1..sequence.len() - 1].parse::<u32>().is_ok();
                return None;
            }
            return decode_sequence(&sequence);
        }
        if self.alt && (byte < 0x20 || byte == 127) {
            self.alt = false;
            return None;
        }
        match byte {
            27 => {
                self.escape = 1;
                self.escape_started = Some(std::time::Instant::now());
                None
            }
            b'\r' => Some(Key::Enter),
            b'\t' => Some(Key::Tab),
            127 => {
                self.alt = false;
                Some(Key::Backspace)
            }
            1..=26 => Some(Key::Ctrl((b'a' + byte - 1) as char)),
            _ => {
                self.utf8.push(byte);
                match std::str::from_utf8(&self.utf8) {
                    Ok(s) => {
                        let alt = std::mem::take(&mut self.alt);
                        let c = s.chars().next().and_then(|c| {
                            if alt {
                                c.is_ascii_lowercase().then_some(Key::Alt(c))
                            } else {
                                (!c.is_control()).then_some(Key::Char(c))
                            }
                        });
                        self.utf8.clear();
                        self.alt = false;
                        c
                    }
                    Err(e) if e.error_len().is_none() && self.utf8.len() < 4 => None,
                    Err(_) => {
                        self.utf8.clear();
                        self.alt = false;
                        None
                    }
                }
            }
        }
    }
    pub(super) fn flush(&mut self) -> Option<Key> {
        self.escape_started = None;
        if self.escape == 1 {
            self.escape = 0;
            Some(Key::Esc)
        } else {
            self.escape = 0;
            self.sequence.clear();
            None
        }
    }
}

fn arrow_key(arrow: Arrow, modifier: u32) -> Option<Key> {
    match modifier {
        1 => Some(match arrow {
            Arrow::Up => Key::Up,
            Arrow::Down => Key::Down,
            Arrow::Left => Key::Left,
            Arrow::Right => Key::Right,
        }),
        3 => Some(Key::AltArrow(arrow)),
        5 => Some(Key::CtrlArrow(arrow)),
        _ => None,
    }
}

fn decode_sequence(sequence: &str) -> Option<Key> {
    let last = sequence.chars().last()?;
    let body = &sequence[..sequence.len() - 1];
    let fields: Vec<_> = body.split(';').collect();
    let modifier = fields
        .get(1)
        .map(|s| s.split(':').next()?.parse::<u32>().ok())
        .unwrap_or(Some(1))?;
    // Kitty releases are not actions; press/repeat are accepted.
    if fields
        .get(1)
        .and_then(|s| s.split(':').nth(1))
        .is_some_and(|s| s != "1" && s != "2")
    {
        return None;
    }
    if let Some(arrow) = match last {
        'A' => Some(Arrow::Up),
        'B' => Some(Arrow::Down),
        'C' => Some(Arrow::Right),
        'D' => Some(Arrow::Left),
        _ => None,
    } {
        if !body.is_empty() && fields[0] != "1" {
            return None;
        }
        return arrow_key(arrow, modifier);
    }
    if last == 'Z' && body.is_empty() {
        return Some(Key::BackTab);
    }
    if last != 'u' || fields.len() > 3 {
        return None;
    }
    let code: u32 = fields[0].split(':').next()?.parse().ok()?;
    if let Some(arrow) = match code {
        57350 => Some(Arrow::Left),
        57351 => Some(Arrow::Right),
        57352 => Some(Arrow::Up),
        57353 => Some(Arrow::Down),
        _ => None,
    } {
        return arrow_key(arrow, modifier);
    }
    match modifier {
        1 => match code {
            13 => Some(Key::Enter),
            27 => Some(Key::Esc),
            127 => Some(Key::Backspace),
            9 => Some(Key::Tab),
            _ => char::from_u32(code)
                .filter(|c| !c.is_control() && code < 57344)
                .map(Key::Char),
        },
        2 if code == 9 => Some(Key::BackTab),
        2 => {
            let shifted = fields[0]
                .split(':')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(code);
            char::from_u32(shifted)
                .filter(|c| !c.is_control() && shifted < 57344)
                .map(|c| {
                    Key::Char(if shifted == code && c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    })
                })
        }
        3 | 5 => char::from_u32(code)
            .filter(char::is_ascii_lowercase)
            .map(|c| {
                if modifier == 5 {
                    Key::Ctrl(c)
                } else {
                    Key::Alt(c)
                }
            }),
        _ => None,
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
