//! Native-only tests prepared under the approved Windows TDD exception.
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::{atomic::Ordering, Arc};
use std::time::{Duration, Instant};

use crate::windows_pipe::{pipe_name, Pipe};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        access: u32,
        mode: u32,
        instances: u32,
        out_size: u32,
        in_size: u32,
        timeout: u32,
        security: *const std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn ConnectNamedPipe(pipe: *mut std::ffi::c_void, overlapped: *mut std::ffi::c_void) -> i32;
}

fn server(name: &str) -> std::fs::File {
    let path = pipe_name(OsStr::new(name)).unwrap();
    // SAFETY: the NUL-terminated name lives through the call; a fresh duplex
    // byte pipe is owned exactly once, and its default DACL is used only in tests.
    let handle = unsafe {
        CreateNamedPipeW(
            path.as_ptr(),
            3 | 0x80000,
            0,
            1,
            4096,
            4096,
            0,
            std::ptr::null(),
        )
    };
    assert_ne!(handle as isize, -1, "{}", std::io::Error::last_os_error());
    unsafe { std::fs::File::from(OwnedHandle::from_raw_handle(handle)) }
}

fn accept(pipe: &std::fs::File) {
    use std::os::windows::io::AsRawHandle;
    // SAFETY: a live server pipe handle, synchronous operation, no OVERLAPPED.
    let ok = unsafe { ConnectNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut()) };
    assert!(ok != 0 || std::io::Error::last_os_error().raw_os_error() == Some(535));
}

#[test]
fn generic_namespaced_mapping_prepends_without_normalizing_the_marker() {
    for marker in [
        "herdr.sock",
        r"C:\Users\test\herdr.sock",
        r"\\.\pipe\already",
        "herdr-🐑",
    ] {
        let name = pipe_name(OsStr::new(marker)).unwrap();
        assert_eq!(
            String::from_utf16(&name[..name.len() - 1]).unwrap(),
            format!(r"\\.\pipe\{marker}")
        );
        assert_eq!(name.last(), Some(&0));
    }
    assert!(pipe_name(OsStr::new("bad\0pipe")).is_err());
    assert!(pipe_name(OsStr::new("")).is_err());
}

#[test]
fn auto_theme_uses_windows_roaming_paths_after_explicit_overrides() {
    let keys = [
        "HERDR_CONFIG_PATH",
        "XDG_CONFIG_HOME",
        "APPDATA",
        "USERPROFILE",
        "SSH_CONNECTION",
        "SSH_CLIENT",
        "SSH_TTY",
    ];
    let saved = keys.map(|key| (key, std::env::var_os(key)));
    for key in keys {
        std::env::remove_var(key);
    }
    let root = std::env::temp_dir().join(format!("crossroads-auto-{}", std::process::id()));
    for (path, theme) in [
        ("profile/AppData/Roaming/herdr/config.toml", "dracula"),
        ("roaming/herdr/config.toml", "nord"),
        ("xdg/herdr/config.toml", "vesper"),
        ("explicit.toml", "terminal"),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("[theme]\nname = \"{theme}\"\n")).unwrap();
    }
    for (key, path, theme) in [
        ("USERPROFILE", "profile", "dracula"),
        ("APPDATA", "roaming", "nord"),
        ("XDG_CONFIG_HOME", "xdg", "vesper"),
        ("HERDR_CONFIG_PATH", "explicit.toml", "terminal"),
    ] {
        std::env::set_var(key, root.join(path));
        assert_eq!(crate::theme::auto_choice().0, theme);
    }
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pipe_delayed_partial_data_eof_and_overall_deadline() {
    let name = format!("crossroads-read-{}", std::process::id());
    let mut listener = server(&name);
    let worker = std::thread::spawn(move || {
        accept(&listener);
        std::thread::sleep(Duration::from_millis(50));
        listener.write_all(b"{\"ok\":").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        listener.write_all(b"true}\n").unwrap();
        std::thread::sleep(Duration::from_millis(100));
    });
    let mut stream = Pipe::connect(OsStr::new(&name)).unwrap();
    assert_eq!(
        crate::transport::socket_line(&mut stream, Some(Instant::now() + Duration::from_secs(1)))
            .unwrap()["ok"],
        true
    );
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    assert_eq!(
        stream.read(&mut [0]).unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
    worker.join().unwrap();
    assert!(crate::transport::socket_line(
        &mut stream,
        Some(Instant::now() + Duration::from_secs(1))
    )
    .is_err());
}

#[test]
fn idle_pipe_shutdown_interrupts_reader_and_missing_pipe_is_bounded() {
    let name = format!("crossroads-cancel-{}", std::process::id());
    let listener = server(&name);
    let mut stream = Pipe::connect(OsStr::new(&name)).unwrap();
    accept(&listener);
    let control = stream.try_clone().unwrap();
    let worker = std::thread::spawn(move || stream.read(&mut [0]));
    let started = Instant::now();
    control.shutdown(std::net::Shutdown::Both).unwrap();
    assert!(worker.join().unwrap().is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    let started = Instant::now();
    assert!(Pipe::connect(OsStr::new("crossroads-nonexistent-pipe")).is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn stable_id_focus_checks_correlated_response_and_times_out() {
    // This test alone mutates the socket environment; run the native unit suite
    // with --test-threads=1 to keep it isolated from all environment-based tests.
    for (case, reply, succeeds) in [
        (
            "ok",
            Some(
                r#"{"id":"crossroads-focus","result":{"type":"pane_info","pane":{"pane_id":"stable-b","focused":true}}}"#,
            ),
            true,
        ),
        (
            "wrong",
            Some(
                r#"{"id":"crossroads-focus","result":{"type":"pane_info","pane":{"pane_id":"stable-a","focused":true}}}"#,
            ),
            false,
        ),
        ("silent", None, false),
    ] {
        let name = format!("crossroads-focus-{}-{case}", std::process::id());
        let mut listener = server(&name);
        let worker = std::thread::spawn(move || {
            accept(&listener);
            let mut request = Vec::new();
            let mut byte = [0];
            while listener.read(&mut byte).unwrap() != 0 && byte[0] != b'\n' {
                request.push(byte[0]);
            }
            let request: serde_json::Value = serde_json::from_slice(&request).unwrap();
            assert_eq!(request["method"], "pane.focus");
            assert_eq!(request["params"]["pane_id"], "stable-b");
            if let Some(reply) = reply {
                writeln!(listener, "{reply}").unwrap();
            }
            std::thread::sleep(Duration::from_millis(650));
        });
        std::env::set_var("HERDR_SOCKET_PATH", name);
        let started = Instant::now();
        assert_eq!(crate::focus::focus("stable-b").is_ok(), succeeds);
        assert!(started.elapsed() < Duration::from_secs(1));
        worker.join().unwrap();
    }
    std::env::remove_var("HERDR_SOCKET_PATH");
}

#[test]
fn reconnect_cancel_stops_pipe_open_and_cli_before_spawn() {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(true));
    crate::transport::RECONNECT_CANCEL.with(|flag| *flag.borrow_mut() = Some(cancel.clone()));
    assert_eq!(
        Pipe::connect(OsStr::new("crossroads-cancel-open"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert_eq!(
        crate::transport::cli_output(OsStr::new("missing.exe"), &[], true)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    cancel.store(false, Ordering::Relaxed);
    crate::transport::RECONNECT_CANCEL.with(|flag| *flag.borrow_mut() = None);
}

#[test]
fn reconnect_cli_deadline_and_pipe_line_limit_are_enforced() {
    let fake = std::env::var_os("CROSSROADS_WINDOWS_TEST_HERDR").expect("native CLI fixture path");
    std::env::set_var("CROSSROADS_TEST_STALL", "1");
    let started = Instant::now();
    let result = crate::transport::cli_output(&fake, &["api", "snapshot"], true);
    std::env::remove_var("CROSSROADS_TEST_STALL");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(4));

    let name = format!("crossroads-overlong-{}", std::process::id());
    let mut listener = server(&name);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        accept(&listener);
        let _ = listener.write_all(&vec![b'x'; 65536]);
        let _ = done_rx.recv_timeout(Duration::from_secs(10));
    });
    let mut stream = Pipe::connect(OsStr::new(&name)).unwrap();
    assert_eq!(
        crate::transport::socket_line(&mut stream, Some(Instant::now() + Duration::from_secs(5)))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    drop(stream);
    done_tx.send(()).unwrap();
    writer.join().unwrap();
}
