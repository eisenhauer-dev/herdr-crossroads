use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(unix)]
fn no_focus_case(input: &[u8]) -> (std::process::Output, Option<serde_json::Value>) {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let socket = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-focus-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let request = loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                stream
                    .write_all(
                        b"{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"pane_info\",\"pane\":{\"pane_id\":\"p-b\",\"focused\":true}}}\n",
                    )
                    .unwrap();
                break Some(serde_json::from_str(&line).unwrap());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if child.try_wait().unwrap().is_some() {
                    break None;
                }
                std::thread::yield_now();
            }
            Err(e) => panic!("accept: {e}"),
        }
    };
    let output = child.wait_with_output().unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_file(socket).unwrap();
    (output, request)
}

#[cfg(unix)]
fn focus_case(input: &[u8], reply: &str) -> (std::process::Output, serde_json::Value) {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let socket = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-select-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&socket).unwrap();
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    listener.set_nonblocking(true).unwrap();
    let request = loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                stream.write_all(reply.as_bytes()).unwrap();
                break serde_json::from_str::<serde_json::Value>(&line).unwrap();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if child.try_wait().unwrap().is_some() {
                    panic!("no focus request");
                }
                std::thread::yield_now();
            }
            Err(e) => panic!("accept: {e}"),
        }
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    let expired = loop {
        if child.try_wait().unwrap().is_some() {
            break false;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break true;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let output = child.wait_with_output().unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_file(socket).unwrap();
    assert!(!expired, "focus reply hung until test watchdog killed it");
    (output, request)
}

#[cfg(unix)]
#[test]
fn select_enter_focuses_visible_filtered_fallback_by_id() {
    let (output, request) = focus_case(
        b"/codex\n\n",
        "{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"pane_info\",\"pane\":{\"pane_id\":\"p-other\",\"focused\":true}}}\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        request,
        serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":"p-other"}})
    );
}

#[cfg(unix)]
#[test]
fn stale_or_invalid_focus_response_fails_without_another_target() {
    for reply in [
        "{\"id\":\"crossroads-focus\",\"error\":{\"code\":\"not_found\",\"message\":\"pane not found\"}}\n",
        "{\"id\":\"wrong\",\"result\":{\"type\":\"pane_info\"}}\n",
        "{\"id\":\"crossroads-focus\",\"result\":{}}\n",
        "{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"wrong\",\"pane\":{\"pane_id\":\"p-b\",\"focused\":true}}}\n",
        "{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"pane_info\"}}\n",
        "{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"pane_info\",\"pane\":{\"pane_id\":\"p-other\",\"focused\":true}}}\n",
        "{\"id\":\"crossroads-focus\",\"result\":{\"type\":\"pane_info\",\"pane\":{\"pane_id\":\"p-b\",\"focused\":false}}}\n",
        "broken\n",
    ] {
        let (output, request) = focus_case(b"\n", reply);
        assert!(!output.status.success());
        assert_eq!(request, serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":"p-b"}}));
    }
}

#[cfg(unix)]
#[test]
fn overlong_focus_reply_is_rejected() {
    let reply = format!(
        "{{\"id\":\"crossroads-focus\",\"result\":{{\"type\":\"pane_info\",\"pane\":{{\"pane_id\":\"p-b\",\"focused\":true}},\"padding\":\"{}\"}}}}\n",
        "x".repeat(20_000)
    );
    let (output, request) = focus_case(b"\n", &reply);
    assert!(!output.status.success(), "overlong reply was accepted");
    assert_eq!(request["params"]["pane_id"], "p-b");
}

#[cfg(unix)]
#[test]
fn trickling_focus_reply_expires_at_overall_deadline() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};

    let socket = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-trickle-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    let watchdog = Instant::now() + Duration::from_secs(3);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < watchdog => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("no focus request before watchdog: {e}");
            }
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    let started = Instant::now();
    // Keep the socket active more often than its per-read timeout; never send a newline.
    for _ in 0..9 {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if let Err(error) = stream.write_all(b" ") {
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let elapsed = started.elapsed();
    let finished = child.try_wait().unwrap().is_some();
    if !finished {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_file(socket).unwrap();
    assert!(
        finished && elapsed < Duration::from_millis(800),
        "focus read exceeded overall deadline ({elapsed:?})"
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"));
    assert_eq!(request["params"]["pane_id"], "p-b");
}

#[cfg(unix)]
#[test]
fn silent_focus_server_times_out_without_retrying_another_target() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};

    let socket = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-silent-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("no focus request before deadline: {e}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    // Keep the connection open, but never send a reply.
    let expired = loop {
        if child.try_wait().unwrap().is_some() {
            break false;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break true;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let output = child.wait_with_output().unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "api snapshot\n");
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_file(socket).unwrap();
    assert!(!expired, "focus read hung until test watchdog killed it");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"));
    assert_eq!(
        request,
        serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":"p-b"}})
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[cfg(unix)]
#[test]
fn non_select_keys_and_no_matches_never_focus() {
    for input in [
        b"/codex\nq".as_slice(),
        b"/codex\x1bq",
        b"/nomatch\n\n",
        b"q",
        b"\x1b",
    ] {
        let (output, request) = no_focus_case(input);
        assert!(output.status.success());
        assert!(request.is_none(), "unexpected request: {request:?}");
    }
}

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
            "#!/bin/sh\nif [ \"$1 $2\" = 'worktree list' ]; then echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; exit; fi\nprintf '%s\\n' \"$1 $2\" >> \"$CROSSROADS_TEST_LOG\"\n[ \"$1 $2\" = 'api snapshot' ] || exit 9\ncat '{}'\n",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn plugin_pane_title_is_parsed_from_manifest() {
    let output = Command::new("python3")
        .args(["-c", "import tomllib,sys; p=tomllib.load(open(sys.argv[1], 'rb')); assert p['name'] == 'Herdr Crossroads'; assert p['panes'][0]['id'] == 'crossroads'; assert p['panes'][0]['title'] == 'Herdr Crossroads'", concat!(env!("CARGO_MANIFEST_DIR"), "/herdr-plugin.toml")])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn status_colors_config_defaults_partial_override_and_invalid_files() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let base = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-colors-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&base).unwrap();
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let run = |config: Option<&str>| {
        match config {
            Some(contents) => std::fs::write(base.join("status-colors.json"), contents).unwrap(),
            None => {
                let _ = std::fs::remove_file(base.join("status-colors.json"));
            }
        }
        Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
            .env("HERDR_BIN_PATH", &fake)
            .env("CROSSROADS_TEST_LOG", &log)
            .env("HERDR_PLUGIN_CONFIG_DIR", &base)
            .env("HERDR_PANE_ID", "p-self")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let missing = run(None);
    assert!(missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stdout).contains("Claude | working"));
    let partial = run(Some(r##"{"working":"#123ABC"}"##));
    assert!(
        partial.status.success(),
        "{}",
        String::from_utf8_lossy(&partial.stderr)
    );
    let text = String::from_utf8_lossy(&partial.stdout);
    assert!(
        text.contains("Claude | working") && text.contains("codex | idle"),
        "{text}"
    );
    for invalid in [
        "{",
        "[]",
        r##"{"working":"red"}"##,
        r##"{"working":"#123abz"}"##,
        r##"{"other":"#123ABC"}"##,
    ] {
        let result = run(Some(invalid));
        assert!(!result.status.success(), "{invalid}");
        assert!(result.stdout.is_empty(), "{invalid}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("status-colors.json"),
            "{invalid}: {:?}",
            result.stderr
        );
    }
    std::fs::remove_file(base.join("status-colors.json")).unwrap();
    std::fs::create_dir(base.join("status-colors.json")).unwrap();
    let unreadable = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_PLUGIN_CONFIG_DIR", &base)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!unreadable.status.success());
    assert!(unreadable.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unreadable.stderr).contains("status-colors.json"));
    std::fs::remove_file(log).unwrap();
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_dir_all(base).unwrap();
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
        !text.contains("\x1b[?25"),
        "piped output changed cursor: {text}"
    );
    assert!(
        text.contains("Claude") && text.contains("codex") && text.contains("pane 3"),
        "{text}"
    );
    assert!(!text.contains("p-self"), "{text}");
    assert!(
        text.find("Claude") < text.find("pane 3") && text.find("pane 3") < text.find("codex"),
        "{text}"
    );
    assert!(
        text.contains("Claude")
            && text.contains("Workspace")
            && text.contains("Agent/Pane Name")
            && !text.contains("p-b"),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn startup_marks_only_linked_workspace_by_id_and_does_not_requery_on_search() {
    use std::os::unix::fs::PermissionsExt;
    let base = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-worktree-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let fake = base.join("herdr");
    let log = base.join("calls.log");
    std::fs::write(&fake, format!(r#"#!/bin/sh
printf '%s\n' "$*" >> "$CROSSROADS_TEST_LOG"
if [ "$*" = 'api snapshot' ]; then cat '{}'; exit; fi
if [ "$*" = 'worktree list --workspace w1' ]; then
  printf '%s\n' '{{"result":{{"type":"worktree_list","worktrees":[{{"is_linked_worktree":false,"open_workspace_id":"w1","branch":"main","path":"/private/not-for-display"}},{{"is_linked_worktree":true,"open_workspace_id":"w2","branch":"feature/demo","path":"/private/not-for-display"}}]}}}}'
  exit
fi
exit 9
"#, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "120")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"/codexq\x1bq")?;
            child.wait_with_output()
        })
        .unwrap();
    let calls = std::fs::read_to_string(&log).unwrap();
    std::fs::remove_dir_all(base).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("↳ Same (feature/demo)"), "{text}");
    assert!(
        text.lines()
            .any(|line| line.contains("Claude") && line.contains("Same") && !line.contains('↳')),
        "{text}"
    );
    assert!(!text.contains("/private/not-for-display"), "{text}");
    assert_eq!(calls, "api snapshot\nworktree list --workspace w1\n");
}

#[cfg(unix)]
#[test]
fn failed_or_invalid_worktree_lookup_falls_back_to_plain_workspace_names() {
    use std::os::unix::fs::PermissionsExt;
    for (suffix, response) in [("error", "exit 9"), ("invalid", "printf 'invalid json\\n'")] {
        let base = std::path::Path::new("/tmp/opencode").join(format!(
            "crossroads-worktree-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&base).unwrap();
        let fake = base.join("herdr");
        let log = base.join("calls.log");
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; exit; fi\n{response}\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
            .env("HERDR_BIN_PATH", &fake)
            .env("CROSSROADS_TEST_LOG", &log)
            .env("HERDR_PANE_ID", "p-self")
            .env("COLUMNS", "120")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let calls = std::fs::read_to_string(&log).unwrap();
        std::fs::remove_dir_all(base).unwrap();
        assert!(
            output.status.success(),
            "{suffix}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains("Same") && text.contains("Claude") && text.contains("codex"),
            "{text}"
        );
        assert!(!text.contains('↳'), "{text}");
        assert_eq!(
            calls,
            "api snapshot\nworktree list --workspace w1\nworktree list --workspace w2\n"
        );
    }
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
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace"));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).lines().last(),
        Some("Enter: focus · Esc/q: exit · ?: keys")
    );
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
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace"));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).lines().last(),
        Some("Enter: focus · Esc/q: exit · ?: keys")
    );
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
