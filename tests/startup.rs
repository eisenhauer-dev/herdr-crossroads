use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(unix)]
#[test]
fn no_socket_variable_keeps_one_snapshot_and_never_lists_panes_on_retry() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    for socket_value in [None, Some("")] {
        let base = std::path::Path::new("/tmp/opencode").join(format!(
            "crossroads-no-env-{}-{}",
            std::process::id(),
            if socket_value.is_some() {
                "empty"
            } else {
                "absent"
            }
        ));
        std::fs::create_dir(&base).unwrap();
        let fake = base.join("herdr");
        let log = base.join("calls");
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'pane list' ]; then exit 9; fi\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; exit; fi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"));
        command
            .env("HERDR_BIN_PATH", &fake)
            .env("CROSSROADS_TEST_LOG", &log)
            .env("HERDR_PANE_ID", "p-self")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        if let Some(value) = socket_value {
            command.env("HERDR_SOCKET_PATH", value);
        } else {
            command.env_remove("HERDR_SOCKET_PATH");
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !log.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(log.exists(), "snapshot did not start");
        // Keep stdin open beyond the 500ms retry interval.
        std::thread::sleep(Duration::from_millis(700));
        child.stdin.take().unwrap().write_all(b"q").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Claude | working"));
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "api snapshot\nworktree list --workspace w1\nworktree list --workspace w2\n"
        );
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn live_status_burst_coalesces_frames_and_keeps_quit_responsive() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};
    let base = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-burst-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let listener = UnixListener::bind(base.join("socket")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = base.join("herdr");
    std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; exit; fi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("HERDR_SOCKET_PATH", base.join("socket"))
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "65")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(error) => panic!("subscription not opened: {error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut request = String::new();
    BufReader::new(&stream).read_line(&mut request).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request).unwrap()["method"],
        "events.subscribe"
    );
    stream
        .write_all(
            b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
        )
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap()
        .contains("Claude | working")
    {}
    let mut burst = String::new();
    for index in 0..256 {
        let status = if index == 255 {
            "done"
        } else if index % 2 == 0 {
            "idle"
        } else {
            "blocked"
        };
        burst.push_str(&format!("{{\"event\":\"pane.agent_status_changed\",\"data\":{{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"{status}\"}}}}\n"));
    }
    stream.write_all(burst.as_bytes()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut frames = 0;
    let mut got_done = false;
    while let Ok(line) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if line.contains("Claude | ") {
            frames += 1;
            if line.contains("Claude | done") {
                got_done = true;
                break;
            }
        }
    }
    let started_quit = Instant::now();
    child.stdin.take().unwrap().write_all(b"q").unwrap();
    let quit_deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() && Instant::now() < quit_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let exited = child.try_wait().unwrap().is_some();
    if !exited {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    std::fs::remove_dir_all(base).unwrap();
    assert!(got_done, "last event not rendered");
    assert!(
        frames <= 16,
        "burst rendered {frames} frames instead of batched redraws"
    );
    assert!(
        exited && output.status.success() && started_quit.elapsed() < Duration::from_secs(2),
        "q blocked behind burst"
    );
}

#[cfg(unix)]
#[test]
fn status_subscription_bootstrap_event_and_later_update_use_exact_pane_id() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let base = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-live-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&base).unwrap();
    let socket = base.join("api.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let fake = base.join("herdr");
    let log = base.join("calls");
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-self\"}},{{\"pane_id\":\"p-a\"}},{{\"pane_id\":\"p-other\"}},{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then sleep 0.15; cat '{}'; exit; fi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "150")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(pair) => break pair,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(error) => {
                child.kill().unwrap();
                let _ = child.wait();
                std::fs::remove_dir_all(&base).unwrap();
                panic!("expected status subscription after pane list: {error}");
            }
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(request["method"], "events.subscribe");
    assert_eq!(
        request["params"]["subscriptions"].as_array().unwrap().len(),
        4
    );
    assert!(request["params"]["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == &serde_json::json!({"type":"pane.agent_status_changed","pane_id":"p-b"})));
    stream
        .write_all(
            format!(
                "{{\"id\":{},\"result\":{{\"type\":\"subscription_started\"}}}}\n",
                request["id"]
            )
            .as_bytes(),
        )
        .unwrap();
    stream.write_all(b"{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"done\"}}\n").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    stream.write_all(b"{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-other\",\"workspace_id\":\"w1\",\"agent_status\":\"blocked\"}}\n{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"closed-id\",\"workspace_id\":\"w1\",\"agent_status\":\"blocked\"}}\n{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"blocked\"}}\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    child.stdin.take().unwrap().write_all(b"q").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.lines()
            .any(|l| l.contains("Claude") && l.contains("done")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.contains("Claude") && l.contains("blocked")),
        "{text}"
    );
    assert!(
        !text
            .lines()
            .any(|l| l.contains("codex") && l.contains("blocked")),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .take(2)
            .collect::<Vec<_>>(),
        ["pane list", "api snapshot"]
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[cfg(unix)]
#[test]
fn disconnected_status_socket_resubscribes_then_resnapshots_before_updates() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};
    for buffered_status in [None, Some("idle"), Some("retry")] {
        let base = std::path::Path::new("/tmp/opencode").join(format!(
            "crossroads-resync-{}-{}",
            std::process::id(),
            buffered_status.unwrap_or("snapshot")
        ));
        std::fs::create_dir(&base).unwrap();
        let listener = UnixListener::bind(base.join("socket")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let fake = base.join("herdr");
        let log = base.join("calls");
        let mut replacement: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/topology.json")).unwrap();
        replacement["result"]["snapshot"]["agents"][0]["agent_status"] = "done".into();
        std::fs::write(base.join("replacement.json"), replacement.to_string()).unwrap();
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then\n  if [ -f \"$CROSSROADS_BASE/first\" ]; then\n    touch \"$CROSSROADS_BASE/second-started\"\n    while [ ! -f \"$CROSSROADS_BASE/release\" ]; do sleep 0.01; done\n    if [ \"$CROSSROADS_RETRY\" = '1' ] && [ ! -f \"$CROSSROADS_BASE/failed\" ]; then touch \"$CROSSROADS_BASE/failed\"; echo invalid; exit; fi\n    cat \"$CROSSROADS_BASE/replacement.json\"\n  else\n    touch \"$CROSSROADS_BASE/first\"\n    cat '{}'\n  fi\n  exit\nfi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
            .env("HERDR_BIN_PATH", &fake)
            .env("CROSSROADS_TEST_LOG", &log)
            .env("CROSSROADS_BASE", &base)
            .env(
                "CROSSROADS_RETRY",
                if buffered_status == Some("retry") {
                    "1"
                } else {
                    "0"
                },
            )
            .env("HERDR_SOCKET_PATH", base.join("socket"))
            .env("HERDR_PANE_ID", "p-self")
            .env("COLUMNS", "65")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let (lines_tx, lines_rx) = std::sync::mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if lines_tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut output_lines = Vec::new();
        let mut until_line = |needle: &str| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let line = lines_rx
                    .recv_timeout(remaining)
                    .expect("expected rendered status before deadline");
                let found = line.contains(needle);
                output_lines.push(line);
                if found {
                    break;
                }
            }
        };
        let accept = |listener: &UnixListener| {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("subscription not re-established: {error}"),
                }
            }
        };
        let mut first = accept(&listener);
        let mut line = String::new();
        BufReader::new(&first).read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap()["method"],
            "events.subscribe"
        );
        first
            .write_all(
                b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
            )
            .unwrap();
        first.write_all(b"{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"blocked\"}}\n").unwrap();
        until_line("Claude | blocked");
        let deadline = Instant::now() + Duration::from_secs(3);
        while !base.join("first").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            base.join("first").exists(),
            "initial snapshot did not start"
        );
        drop(first);
        let mut second = accept(&listener);
        line.clear();
        BufReader::new(&second).read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap()["params"]["subscriptions"][0]
                ["pane_id"],
            "p-b"
        );
        second
            .write_all(
                b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !base.join("second-started").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !base.join("second-started").exists() {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("reconnect snapshot did not start");
        }
        if let Some(status) = buffered_status.filter(|status| *status != "retry") {
            second.write_all(format!("{{\"event\":\"pane.agent_status_changed\",\"data\":{{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"{status}\"}}}}\n").as_bytes()).unwrap();
        }
        std::fs::write(base.join("release"), "").unwrap();
        let mut retry_connection = None;
        if buffered_status == Some("retry") {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !base.join("failed").exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                base.join("failed").exists(),
                "first reconnect snapshot did not fail"
            );
            second.shutdown(std::net::Shutdown::Both).unwrap();
            let mut third = accept(&listener);
            line.clear();
            BufReader::new(&third).read_line(&mut line).unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&line).unwrap()["method"],
                "events.subscribe"
            );
            third.write_all(b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n").unwrap();
            retry_connection = Some(third);
        }
        let mut expected = buffered_status
            .filter(|status| *status != "retry")
            .unwrap_or("done");
        until_line(&format!("Claude | {expected}"));
        if buffered_status == Some("idle") {
            second.write_all(b"{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"done\"}}\n").unwrap();
            until_line("Claude | done");
            expected = "done";
        }
        drop(retry_connection);
        child.stdin.take().unwrap().write_all(b"q").unwrap();
        let output = child.wait_with_output().unwrap();
        reader.join().unwrap();
        assert!(output.status.success());
        let last_agent = output_lines
            .iter()
            .filter(|line| line.contains("Claude | "))
            .last()
            .unwrap();
        assert!(
            last_agent.contains(&format!("Claude | {expected}")),
            "{output_lines:?}"
        );
        assert!(
            output_lines
                .iter()
                .any(|line| line.contains("Claude | blocked")),
            "{output_lines:?}"
        );
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(calls.starts_with("pane list\napi snapshot\n"), "{calls}");
        let expected_calls = if buffered_status == Some("retry") {
            3
        } else {
            2
        };
        assert_eq!(
            calls.matches("pane list\n").count(),
            expected_calls,
            "{calls}"
        );
        assert_eq!(
            calls.matches("api snapshot\n").count(),
            expected_calls,
            "{calls}"
        );
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn reconnect_installs_snapshot_while_status_sender_is_still_active() {
    use std::io::{BufRead, BufReader};
    use std::net::Shutdown;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let base = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-active-reconnect-{}",
        std::process::id()
    ));
    std::fs::create_dir(&base).unwrap();
    let listener = UnixListener::bind(base.join("socket")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut replacement: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/topology.json")).unwrap();
    replacement["result"]["snapshot"]["workspaces"][1]["label"] = "Fresh topology".into();
    std::fs::write(base.join("replacement.json"), replacement.to_string()).unwrap();
    let fake = base.join("herdr");
    std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then\n  if [ -f \"$CROSSROADS_BASE/first\" ]; then\n    touch \"$CROSSROADS_BASE/held\"\n    while [ ! -f \"$CROSSROADS_BASE/release\" ]; do sleep 0.01; done\n    cat \"$CROSSROADS_BASE/replacement.json\"\n  else\n    touch \"$CROSSROADS_BASE/first\"\n    cat '{}'\n  fi\n  exit\nfi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_BASE", &base)
        .env("HERDR_SOCKET_PATH", base.join("socket"))
        .env("HERDR_PANE_ID", "p-self")
        .env("COLUMNS", "150")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(256);
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let accept = || {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = String::new();
                    BufReader::new(&stream).read_line(&mut request).unwrap();
                    assert_eq!(
                        serde_json::from_str::<serde_json::Value>(&request).unwrap()["method"],
                        "events.subscribe"
                    );
                    stream.write_all(b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n").unwrap();
                    break stream;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("subscription not opened: {error}"),
            }
        }
    };
    let first = accept();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !base.join("first").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(base.join("first").exists());
    drop(first);
    let second = accept();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !base.join("held").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(base.join("held").exists());

    let stop = Arc::new(AtomicBool::new(false));
    let sent = Arc::new(AtomicUsize::new(0));
    let sending = stop.clone();
    let count = sent.clone();
    let mut writer_stream = second.try_clone().unwrap();
    let writer = std::thread::spawn(move || {
        let event = b"{\"event\":\"pane.agent_status_changed\",\"data\":{\"pane_id\":\"p-b\",\"workspace_id\":\"w1\",\"agent_status\":\"idle\"}}\n";
        while !sending.load(Ordering::Relaxed) {
            if writer_stream.write_all(event).is_err() {
                break;
            }
            count.fetch_add(1, Ordering::Relaxed);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while sent.load(Ordering::Relaxed) < 1100 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let prefilled = sent.load(Ordering::Relaxed);
    std::fs::write(base.join("release"), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut installed = false;
    while let Ok(line) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if line.contains("Fresh topology") {
            installed = true;
            break;
        }
    }
    let still_sending = sent.load(Ordering::Relaxed) > prefilled && !writer.is_finished();
    stop.store(true, Ordering::Relaxed);
    let _ = second.shutdown(Shutdown::Both);
    writer.join().unwrap();
    child.stdin.take().unwrap().write_all(b"q").unwrap();
    let quit_deadline = Instant::now() + Duration::from_secs(3);
    while rx
        .recv_timeout(quit_deadline.saturating_duration_since(Instant::now()))
        .is_ok()
    {}
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    std::fs::remove_dir_all(base).unwrap();
    assert!(
        prefilled >= 1100,
        "did not queue more than a batch: {prefilled}"
    );
    assert!(installed && still_sending, "snapshot was not installed during active status traffic: installed={installed}, still_sending={still_sending}");
    assert!(output.status.success());
}

#[cfg(unix)]
#[test]
fn blocked_reconnect_snapshot_does_not_block_quit() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};
    struct ReleaseOnDrop(std::path::PathBuf);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::write(&self.0, "");
        }
    }
    let base = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-reconnect-quit-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let release_on_drop = ReleaseOnDrop(base.join("release"));
    let listener = UnixListener::bind(base.join("socket")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = base.join("herdr");
    std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then\n  if [ -f \"$CROSSROADS_BASE/first\" ]; then\n    touch \"$CROSSROADS_BASE/held\"\n    while [ ! -f \"$CROSSROADS_BASE/release\" ]; do sleep 0.01; done\n  else touch \"$CROSSROADS_BASE/first\"; fi\n  cat '{}'\n  exit\nfi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_BASE", &base)
        .env("HERDR_SOCKET_PATH", base.join("socket"))
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let accept = || {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("expected subscription: {error}"),
            }
        }
    };
    let mut first = accept();
    let mut request = String::new();
    BufReader::new(&first).read_line(&mut request).unwrap();
    first
        .write_all(
            b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !base.join("first").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(base.join("first").exists());
    drop(first);
    let mut second = accept();
    request.clear();
    BufReader::new(&second).read_line(&mut request).unwrap();
    second
        .write_all(
            b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !base.join("held").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    // Always unblock the fake CLI before waiting for the child, even when the assertion fails.
    let held = base.join("held").exists();
    child.stdin.as_mut().unwrap().write_all(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_millis(350);
    let mut exited = false;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::write(base.join("release"), "").unwrap();
    if !exited {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    drop(release_on_drop);
    std::fs::remove_dir_all(base).unwrap();
    assert!(held, "reconnect snapshot did not reach the held command");
    assert!(exited, "q was blocked by reconnect snapshot");
    assert!(output.status.success());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn full_reconnect_listen_queue_does_not_block_quit() {
    use std::io::{BufRead, BufReader};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::{Duration, Instant};

    unsafe extern "C" {
        fn listen(fd: std::ffi::c_int, backlog: std::ffi::c_int) -> std::ffi::c_int;
    }
    let base = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-full-queue-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let socket = base.join("socket");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let fake = base.join("herdr");
    let log = base.join("calls");
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; exit; fi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", &socket)
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut first = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("initial subscription not opened: {error}");
            }
        }
    };
    first
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = String::new();
    BufReader::new(&first).read_line(&mut request).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request).unwrap()["method"],
        "events.subscribe"
    );
    first
        .write_all(
            b"{\"id\":\"crossroads-status\",\"result\":{\"type\":\"subscription_started\"}}\n",
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !std::fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("api snapshot\n")
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(std::fs::read_to_string(&log)
        .unwrap()
        .contains("api snapshot\n"));
    // Queue one connection before shrinking the backlog to zero; keep it unaccepted.
    let _queued = UnixStream::connect(&socket).unwrap();
    // SAFETY: the listener owns a valid listening socket and remains alive here.
    assert_eq!(unsafe { listen(listener.as_raw_fd(), 0) }, 0);
    drop(first);
    let deadline = Instant::now() + Duration::from_secs(3);
    while std::fs::read_to_string(&log)
        .unwrap()
        .matches("pane list\n")
        .count()
        < 2
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let retried = std::fs::read_to_string(&log)
        .unwrap()
        .matches("pane list\n")
        .count()
        >= 2;
    std::thread::sleep(Duration::from_millis(60));
    let no_second_snapshot = std::fs::read_to_string(&log)
        .unwrap()
        .matches("api snapshot\n")
        .count()
        == 1;
    child.stdin.take().unwrap().write_all(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_millis(350);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let exited = child.try_wait().unwrap().is_some();
    if !exited {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(base).unwrap();
    assert!(
        retried && no_second_snapshot,
        "reconnect did not reach a full queue"
    );
    assert!(
        exited && output.status.success(),
        "q waited for full reconnect queue"
    );
}

#[cfg(unix)]
#[test]
fn valid_pane_preflight_without_socket_uses_single_snapshot_and_exits() {
    use std::os::unix::fs::PermissionsExt;
    let base = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-no-socket-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let fake = base.join("herdr");
    let log = base.join("calls");
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CROSSROADS_TEST_LOG\"\nif [ \"$*\" = 'pane list' ]; then echo '{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}}]}}}}'; exit; fi\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; exit; fi\necho '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", &fake)
        .env("CROSSROADS_TEST_LOG", &log)
        .env("HERDR_SOCKET_PATH", base.join("missing.sock"))
        .env("HERDR_PANE_ID", "p-self")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Claude | working"));
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pane list\napi snapshot\nworktree list --workspace w1\nworktree list --workspace w2\n"
    );
    std::fs::remove_dir_all(base).unwrap();
}

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
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pane list\napi snapshot\n"
    );
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
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pane list\napi snapshot\n"
    );
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
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pane list\napi snapshot\n"
    );
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
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pane list\napi snapshot\n"
    );
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
        .env_remove("HERDR_SOCKET_PATH")
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
        .env_remove("HERDR_SOCKET_PATH")
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
            .env_remove("HERDR_SOCKET_PATH")
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
        .env_remove("HERDR_SOCKET_PATH")
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
        Some("  Enter: focus · Esc/q: exit · ?: keys")
    );
}

#[test]
fn search_updates_on_each_character_and_returns_to_selection() {
    let fake = fake_herdr();
    let log = fake.with_extension("log");
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env_remove("HERDR_SOCKET_PATH")
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
        .env_remove("HERDR_SOCKET_PATH")
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
        Some("  Enter: focus · Esc/q: exit · ?: keys")
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
