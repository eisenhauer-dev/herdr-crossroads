use std::io;
use std::process::Command;

pub(crate) fn run() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let agent = arguments.iter().any(|arg| arg == "--agent");
    if arguments
        .first()
        .is_some_and(|arg| arg == "--setup-keybind")
        || (agent && arguments.iter().any(|arg| arg == "--setup-keybind"))
    {
        if arguments.len() != 1 {
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
    crate::runtime::run(if agent {
        crate::selection::TableView::Agent
    } else {
        crate::selection::TableView::Select
    })
}

pub(crate) fn setup_command() -> Command {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg("scripts/setup-keybind.py");
    command
}
