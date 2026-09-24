use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(unix)]
fn fake_herdr() -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1 $2\" >> \"$CROSSROADS_TEST_LOG\"\n[ \"$1 $2\" = 'api snapshot' ] || exit 9\ncat '{}'\n",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn starts_with_eof_and_exits_successfully() {
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "65")
        .stdin(Stdio::null())
        .output()
        .expect("start the crossroads executable");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();

    assert!(output.status.success(), "status: {}", output.status);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("p-a") && text.contains("p-b") && text.contains("p-other"),
        "{text}"
    );
    assert!(!text.contains("p-self"), "{text}");
    assert!(
        text.find("p-b") < text.find("p-a") && text.find("p-a") < text.find("p-other"),
        "{text}"
    );
    assert!(
        text.contains("Claude") && text.contains("working"),
        "{text}"
    );
}

#[test]
fn closes_after_q_in_select() {
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "65")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start the crossroads executable");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"q")
        .expect("send q");
    let output = child.wait_with_output().expect("wait for crossroads");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();

    assert!(output.status.success(), "status: {}", output.status);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Select (/: search"));
}

#[test]
fn search_updates_on_each_character_and_returns_to_selection() {
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"/codexq\x1bq")?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("Search: c") && text.contains("Search: codex"),
        "{text}"
    );
    assert!(
        text.contains("Search: codexq") && text.contains("No matching panes"),
        "{text}"
    );
    assert!(output.status.success());
}

#[test]
fn esc_in_select_closes_on_piped_input_eof() {
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"\x1b").unwrap();
    // An EOF flushes an unambiguous trailing Esc on non-TTY stdin.
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("Select (/: search"));
}

#[cfg(target_os = "linux")]
#[test]
fn thousand_rows_pty_keypress_latency() {
    let output = Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pty_latency.py"))
        .arg(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .output()
        .expect("python3 with Linux PTY standard library");
    print!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn narrow_raw_pty_screen_keeps_rows_and_footer_readable() {
    let output = Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pty_render.py"))
        .arg(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .output()
        .expect("python3 with Linux PTY standard library");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_cli_reports_error_without_rendering_a_table() {
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", "/nonexistent/herdr-crossroads-test")
        .stdin(Stdio::null())
        .output()
        .expect("start the crossroads executable");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
