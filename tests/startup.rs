use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn starts_with_eof_and_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .stdin(Stdio::null())
        .output()
        .expect("start the crossroads executable");

    assert!(output.status.success(), "status: {}", output.status);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Herdr Crossroads\nPress Enter to close.\n"
    );
}

#[test]
fn closes_after_enter() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start the crossroads executable");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"\n")
        .expect("send Enter");
    let output = child.wait_with_output().expect("wait for crossroads");

    assert!(output.status.success(), "status: {}", output.status);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Herdr Crossroads\nPress Enter to close.\n"
    );
}
