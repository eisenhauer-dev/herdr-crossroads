use crate::transport::cli_output;
use std::{
    io,
    time::{Duration, Instant},
};

#[cfg(unix)]
#[test]
fn reconnect_cli_kills_a_stalled_command_at_its_deadline() {
    let started = Instant::now();
    let error = cli_output(std::ffi::OsStr::new("sh"), &["-c", "exec sleep 5"], true).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[cfg(unix)]
#[test]
fn pane_list_preserves_ids_and_exact_validation_errors() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-pane-list-{}", std::process::id()));
    for (body, expected) in [
        (
            r#"{"result":{"type":"pane_list","panes":[{"pane_id":"b"},{"pane_id":"a"},{"pane_id":"b"},{"pane_id":""}]}}"#,
            Ok(vec!["b", "a", ""]),
        ),
        (
            r#"{"result":{"type":"other"}}"#,
            Err((io::ErrorKind::InvalidData, "not a pane list")),
        ),
        (
            r#"{"result":{"type":"pane_list","panes":{}}}"#,
            Err((io::ErrorKind::InvalidData, "missing panes in snapshot")),
        ),
        (
            r#"{"result":{"type":"pane_list","panes":[{"pane_id":3}]}}"#,
            Err((io::ErrorKind::InvalidData, "missing pane_id in snapshot")),
        ),
    ] {
        std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{body}'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        match (
            crate::transport::pane_ids(path.as_os_str(), false),
            expected,
        ) {
            (Ok(ids), Ok(expected)) => assert_eq!(ids, expected),
            (Err(error), Err((kind, message))) => {
                assert_eq!(error.kind(), kind);
                assert_eq!(error.to_string(), message);
            }
            _ => panic!("unexpected pane-list result for {body}"),
        }
    }
    std::fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
    let error = crate::transport::pane_ids(path.as_os_str(), false).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(error.to_string(), "herdr pane list failed");
    std::fs::write(&path, "#!/bin/sh\nprintf 'invalid'\n").unwrap();
    assert_eq!(
        crate::transport::pane_ids(path.as_os_str(), false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Other
    );
    std::fs::remove_file(path).unwrap();
}
