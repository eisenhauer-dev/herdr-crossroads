#![cfg(windows)]
#[path = "../src/terminal/windows.rs"]
mod native_terminal;
#[path = "../src/terminal/windows_input.rs"]
mod windows_input;
use std::io::{Read, Write};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
}

fn modes() -> [u32; 2] {
    [-10i32, -11i32].map(|which| {
        let mut mode = 0;
        // SAFETY: borrowed process console handles, writable DWORD output.
        let handle = unsafe { GetStdHandle(which as u32) };
        let result = unsafe { GetConsoleMode(handle, &mut mode) };
        let error = std::io::Error::last_os_error();
        eprintln!("console which={which} handle={handle:p} mode={mode:#x} result={result}");
        assert_ne!(
            result, 0,
            "initial/restored GetConsoleMode({which}, {handle:p}) failed: {error}"
        );
        mode
    })
}

fn mouse_modes() -> [bool; 2] {
    // Query ConPTY's actual inner VT state rather than assuming application
    // output bytes survive its outer-protocol translation unchanged.
    let _terminal = native_terminal::Terminal::enter().unwrap();
    let mut output = std::io::stdout().lock();
    output.write_all(b"\x1b[?1000$p\x1b[?1006$p").unwrap();
    output.flush().unwrap();
    drop(output);
    let mut input = std::io::stdin().lock();
    let mut replies = Vec::new();
    while replies.windows(2).filter(|pair| *pair == b"$y").count() < 2 {
        assert!(replies.len() < 256, "unexpected mode replies: {replies:?}");
        let mut byte = [0];
        input.read_exact(&mut byte).unwrap();
        replies.push(byte[0]);
    }
    [1000, 1006].map(|mode| {
        let enabled = format!("\x1b[?{mode};1$y");
        let disabled = format!("\x1b[?{mode};2$y");
        assert!(
            replies
                .windows(enabled.len())
                .any(|part| part == enabled.as_bytes())
                || replies
                    .windows(disabled.len())
                    .any(|part| part == disabled.as_bytes()),
            "unsupported/invalid mode {mode}: {replies:?}"
        );
        replies
            .windows(enabled.len())
            .any(|part| part == enabled.as_bytes())
    })
}

#[test]
#[ignore = "windows_acceptance.py runs this test inside a native ConPTY"]
fn console_modes_and_size_are_restored_exactly() {
    let before = modes();
    assert_eq!(mouse_modes(), [false, false]);
    {
        let mut terminal = native_terminal::Terminal::enter().unwrap();
        assert!(terminal.0.is_some());
        terminal.query().unwrap();
        terminal.enable_kitty().unwrap();
        assert_eq!(
            modes()[0] & 0x10,
            0,
            "opt-out does not request mouse delivery"
        );
        terminal.enable_mouse().unwrap();
        assert_eq!(mouse_modes(), [true, true]);
        let active = modes();
        assert_eq!(
            active[0] & 0x47,
            0,
            "no echo, cooked input, Ctrl-C interception or Quick Edit"
        );
        assert_ne!(active[0] & 0x200, 0, "VT input enabled");
        assert_ne!(
            active[0] & 0x10,
            0,
            "ConPTY mouse delivery enabled with VT byte input"
        );
        assert_eq!(active[1] & 0x5, 0x5, "VT output enabled");
        assert_eq!(native_terminal::terminal_size(), (30, 120));
    }
    assert_eq!(modes(), before);
    assert_eq!(mouse_modes(), [false, false]);
    let panic = std::panic::catch_unwind(|| {
        let mut terminal = native_terminal::Terminal::enter().unwrap();
        terminal.enable_mouse().unwrap();
        panic!("exercise owned mouse cleanup during unwinding");
    });
    assert!(panic.is_err());
    assert_eq!(modes(), before);
    assert_eq!(mouse_modes(), [false, false]);
}
