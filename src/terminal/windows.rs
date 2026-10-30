use super::windows_input::input_mode;
use std::io::{self, IsTerminal, Write};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
}

pub(crate) fn terminal_size() -> (usize, usize) {
    ratatui::crossterm::terminal::size()
        .ok()
        .filter(|(width, height)| *width > 0 && *height > 0)
        .map(|(width, height)| (height as usize, width as usize))
        .unwrap_or((24, 80))
}

// Keep the existing runtime's interactive marker; native modes are retained
// separately and restored exactly, including when entering output mode fails.
pub(crate) struct Terminal(pub(crate) Option<String>, Option<(u32, u32)>, bool);

fn mode(which: u32) -> io::Result<u32> {
    let mut mode = 0;
    // SAFETY: GetStdHandle returns a borrowed process handle; mode is writable.
    if unsafe { GetConsoleMode(GetStdHandle(which), &mut mode) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(mode)
}

fn set_mode(which: u32, mode: u32) -> io::Result<()> {
    // SAFETY: the process owns the standard console handle; no ownership transfer.
    if unsafe { SetConsoleMode(GetStdHandle(which), mode) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl Terminal {
    pub(crate) fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() {
            return Ok(Self(None, None, false));
        }
        let input = mode(-10i32 as u32)?;
        let output = mode(-11i32 as u32)?;
        // Input: VT sequences, no cooked line/echo/Ctrl-C handling or Quick Edit.
        // Output: processed output and VT rendering. Mouse delivery is enabled
        // separately, only together with VT input for the byte-stream reader.
        let terminal = Self(Some(String::new()), Some((input, output)), false);
        set_mode(-10i32 as u32, input_mode(input, false))?;
        set_mode(-11i32 as u32, output | 0x1 | 0x4)?;
        Ok(terminal)
    }
    pub(crate) fn query(&self) -> io::Result<()> {
        Ok(())
    }
    pub(crate) fn enable_kitty(&mut self) -> io::Result<()> {
        Ok(())
    }
    pub(crate) fn enable_mouse(&mut self) -> io::Result<()> {
        if self.0.is_some() {
            self.2 = true;
            // ConPTY requests host mouse delivery from ENABLE_MOUSE_INPUT.
            // VT input must remain on so SGR reports pass through as bytes.
            set_mode(-10i32 as u32, input_mode(mode(-10i32 as u32)?, true))?;
            let mut out = io::stdout().lock();
            out.write_all(b"\x1b[?1000h\x1b[?1006h")?;
            out.flush()?;
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some((input, output)) = self.1 {
            let mut out = io::stdout().lock();
            if self.2 {
                let _ = out.write_all(b"\x1b[?1000l\x1b[?1006l");
            }
            let _ = out.write_all(b"\x1b[0m\x1b[?25h");
            let _ = out.flush();
            let _ = set_mode(-10i32 as u32, input);
            let _ = set_mode(-11i32 as u32, output);
        }
    }
}
