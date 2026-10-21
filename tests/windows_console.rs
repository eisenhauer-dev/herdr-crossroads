#![cfg(windows)]
#[path = "../src/terminal/windows.rs"]
mod native_terminal;

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

#[test]
#[ignore = "windows_acceptance.py runs this test inside a native ConPTY"]
fn console_modes_and_size_are_restored_exactly() {
    let before = modes();
    {
        let mut terminal = native_terminal::Terminal::enter().unwrap();
        assert!(terminal.0.is_some());
        terminal.query().unwrap();
        terminal.enable_kitty().unwrap();
        let active = modes();
        assert_eq!(
            active[0] & 0x17,
            0,
            "no echo, cooked input, Ctrl-C interception or mouse capture"
        );
        assert_ne!(active[0] & 0x200, 0, "VT input enabled");
        assert_eq!(active[1] & 0x5, 0x5, "VT output enabled");
        assert_eq!(native_terminal::terminal_size(), (30, 120));
    }
    assert_eq!(modes(), before);
}
