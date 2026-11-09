use std::io;
use std::process::Command;

pub(crate) fn run() -> io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--setup-keybind")) {
        if arguments.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--setup-keybind takes no arguments",
            ));
        }
        let mut command = setup_command();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            return Err(command.exec());
        }
        #[cfg(not(unix))]
        std::process::exit(command.status()?.code().unwrap_or(1));
    }
    crate::runtime::run()
}

pub(crate) fn setup_command() -> Command {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg("scripts/setup-keybind.py");
    command
}
