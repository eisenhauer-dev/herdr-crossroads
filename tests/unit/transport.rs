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
