#[cfg(not(windows))]
use std::io::IsTerminal;
use std::io::{self, Write};
#[cfg(not(windows))]
use std::process::{Command, Stdio};

#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
pub(crate) mod windows_input;
#[cfg(windows)]
pub(super) use windows::{terminal_size, Terminal};

use ratatui::{buffer::CellWidth, style::Color};

use super::presentation::{clipped, sgr_fg};
use super::selection::{Mode, Ui};

#[cfg(not(windows))]
pub(super) fn terminal_size() -> (usize, usize) {
    let measured = Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let mut fields = s.split_whitespace();
            Some((
                fields.next()?.parse::<usize>().ok()?,
                fields.next()?.parse::<usize>().ok()?,
            ))
        });
    let width = measured
        .map(|(_, width)| width)
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok()))
        .filter(|n| *n > 0)
        .unwrap_or(80);
    (
        measured
            .map(|(height, _)| height)
            .filter(|n| *n > 0)
            .unwrap_or(24),
        width,
    )
}

#[cfg(not(windows))]
pub(super) struct Terminal(pub(super) Option<String>, bool, bool);
#[cfg(not(windows))]
impl Terminal {
    pub(super) fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() {
            return Ok(Self(None, false, false));
        }
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()?;
        if !saved.status.success() {
            return Err(io::Error::other("stty -g failed"));
        }
        let state = String::from_utf8(saved.stdout).map_err(io::Error::other)?;
        let terminal = Self(Some(state.trim().to_owned()), false, false);
        let status = Command::new("stty")
            .args(["raw", "-echo", "min", "0", "time", "1"])
            .stdin(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(io::Error::other("stty raw failed"));
        }
        Ok(terminal)
    }
    pub(super) fn query(&self) -> io::Result<()> {
        if self.0.is_some() {
            let mut out = io::stdout().lock();
            out.write_all(b"\x1b[?u")?;
            out.flush()?;
        }
        Ok(())
    }
    pub(super) fn enable_kitty(&mut self) -> io::Result<()> {
        // Mark ownership before writing, so even a partial write gets a best-effort pop.
        self.1 = true;
        let mut out = io::stdout().lock();
        out.write_all(b"\x1b[>1u")?;
        out.flush()
    }
    pub(super) fn enable_mouse(&mut self) -> io::Result<()> {
        if self.0.is_some() {
            self.2 = true;
            let mut out = io::stdout().lock();
            out.write_all(b"\x1b[?1000h\x1b[?1006h")?;
            out.flush()?;
        }
        Ok(())
    }
}
#[cfg(not(windows))]
impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(state) = &self.0 {
            let mut out = io::stdout().lock();
            if self.2 {
                let _ = out.write_all(b"\x1b[?1000l\x1b[?1006l");
            }
            if self.1 {
                let _ = out.write_all(b"\x1b[<u");
            }
            let _ = out.write_all(b"\x1b[?25h");
            let _ = out.flush();
            let _ = Command::new("stty")
                .arg(state)
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

pub(super) fn draw(ui: &Ui, terminal: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if terminal {
        let (width, height) = ui.popup_size.expect("interactive terminal size");
        let frame = ui.terminal_frame(width, height);
        if ui.theme().background != Color::Reset {
            write!(out, "\x1b[H\x1b[J{frame}")?;
            write!(out, "\x1b[0m")?;
        } else {
            write!(out, "\x1b[H\x1b[J")?;
            for (index, line) in frame.split("\r\n").enumerate() {
                if index > 0 {
                    write!(out, "\r\n")?;
                }
                write!(out, "{}{line}", sgr_fg(ui.theme().text))?;
            }
            write!(out, "\x1b[0m")?;
        }
        if (ui.mode == Mode::Search || (ui.mode == Mode::Help && ui.help.searching))
            && width >= 80
            && height >= 15
        {
            // Keep the cursor after the clipped query, before padding and the mode badge.
            let query = if ui.mode == Mode::Help {
                &ui.help.query
            } else {
                &ui.query
            };
            let (_, available) = ui.search_prompt_geometry(width);
            let x = 2 + clipped(&format!("Search: {query}"), available).cell_width() as usize;
            write!(
                out,
                "\x1b[1;{}H\x1b[?25h",
                x.min(width.saturating_sub(1)) + 1
            )?;
        } else {
            out.write_all(b"\x1b[?25l")?;
        }
    } else {
        out.write_all(ui.frame().as_bytes())?;
    }
    out.flush()
}
