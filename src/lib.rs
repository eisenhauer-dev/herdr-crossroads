use std::io::{self, Read};
use std::path::Path;
#[cfg(unix)]
use std::sync::mpsc;
#[cfg(unix)]
use std::sync::{atomic::AtomicBool, Arc};
use std::thread;
use std::time::{Duration, Instant};

mod input;
mod presentation;
mod selection;
mod theme;
mod topology;
mod transport;

use input::{handle_key, Keys};
use presentation::{draw, terminal_size, StatusColors, Terminal};
use selection::Ui;
use topology::snapshot_rows;
#[cfg(unix)]
use transport::{subscribe, ReconnectAttempt, StatusMessage, StatusSubscription, RECONNECT_CANCEL};

pub fn run() -> io::Result<()> {
    let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").filter(|dir| !dir.is_empty());
    let status_colors = StatusColors::load(config_dir.as_deref().map(Path::new))?;
    let executable = std::env::var_os("HERDR_BIN_PATH")
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "herdr".into());
    let width = terminal_size().1;
    #[cfg(unix)]
    let live_status_enabled =
        std::env::var_os("HERDR_SOCKET_PATH").is_some_and(|path| !path.is_empty());
    #[cfg(unix)]
    let mut status = if live_status_enabled {
        subscribe(&executable, false).ok()
    } else {
        None
    };
    let mut ui = Ui::new(snapshot_rows(&executable, width, false)?);
    ui.status_colors = status_colors;
    let (theme_index, theme_error) = theme::load(config_dir.as_deref().map(Path::new));
    ui.theme_index = theme_index;
    ui.theme_invalid = theme_error.is_some();
    ui.theme_error = theme_error;
    if ui.theme_invalid || ui.theme_index != 0 {
        ui.auto_toast = None;
    }
    ui.config_dir = config_dir.map(Into::into);
    ui.width = width;
    let terminal = Terminal::enter()?;
    if terminal.0.is_some() {
        let (height, width) = terminal_size();
        ui.popup_size = Some((width, height));
    }
    #[cfg(unix)]
    if let Some(receiver) = &status {
        for _ in 0..1024 {
            match receiver.rx.try_recv() {
                Ok(StatusMessage::Event(event)) => {
                    ui.status_event(&event);
                }
                Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                    status = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }
    draw(&ui, terminal.0.is_some())?;
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    let raw_terminal = terminal.0.is_some();
    thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut byte = [0];
        loop {
            match input.read(&mut byte) {
                Ok(0) => {
                    if input_tx.send(None).is_err() || !raw_terminal {
                        break;
                    }
                }
                Ok(_) => {
                    if input_tx.send(Some(byte[0])).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = input_tx.send(None);
                    break;
                }
            }
        }
    });
    let mut keys = Keys::new();
    #[cfg(unix)]
    let mut next_size_check = Instant::now() + Duration::from_millis(250);
    #[cfg(unix)]
    let mut retry_at = Instant::now() + Duration::from_millis(500);
    #[cfg(unix)]
    let mut reconnect: Option<ReconnectAttempt> = None;
    #[cfg(unix)]
    let mut pending: Option<(StatusSubscription, Ui)> = None;
    loop {
        #[cfg(unix)]
        if terminal.0.is_some() && Instant::now() >= next_size_check {
            let (height, width) = terminal_size();
            if ui.popup_size != Some((width, height)) {
                let was_small = ui.popup_size.is_some_and(|(w, h)| w < 80 || h < 15);
                let is_small = width < 80 || height < 15;
                ui.popup_size = Some((width, height));
                if was_small != is_small {
                    // Do not apply buffered keys from the previous popup mode after resizing.
                    keys = Keys::new();
                    while input_rx.try_recv().is_ok() {}
                }
                draw(&ui, true)?;
            }
            next_size_check = Instant::now() + Duration::from_millis(250);
        }
        #[cfg(unix)]
        {
            let mut disconnected = false;
            if let Some(receiver) = &status {
                let mut changed = false;
                for _ in 0..1024 {
                    match receiver.rx.try_recv() {
                        Ok(StatusMessage::Event(event)) => {
                            changed |= ui.status_event(&event);
                        }
                        Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if changed {
                    draw(&ui, terminal.0.is_some())?;
                }
            }
            if disconnected {
                status = None;
                retry_at = Instant::now() + Duration::from_millis(500);
            }
            if let Some(worker) = &reconnect {
                match worker.rx.try_recv() {
                    Ok(Ok((receiver, rows))) => {
                        pending = Some((receiver, Ui::new(rows)));
                        reconnect = None;
                    }
                    Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                        reconnect = None;
                        retry_at = Instant::now() + Duration::from_millis(500);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some((receiver, staged)) = &mut pending {
                for _ in 0..1024 {
                    match receiver.rx.try_recv() {
                        Ok(StatusMessage::Event(event)) => {
                            staged.status_event(&event);
                        }
                        Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if disconnected {
                    pending = None;
                    retry_at = Instant::now() + Duration::from_millis(500);
                } else {
                    let (receiver, staged) = pending.take().unwrap();
                    ui.rows = staged.rows;
                    status = Some(receiver);
                    draw(&ui, terminal.0.is_some())?;
                }
            }
            if live_status_enabled
                && status.is_none()
                && reconnect.is_none()
                && pending.is_none()
                && Instant::now() >= retry_at
            {
                let (tx, rx) = mpsc::sync_channel(1);
                let executable = executable.clone();
                let cancel = Arc::new(AtomicBool::new(false));
                let worker_cancel = cancel.clone();
                let worker = thread::spawn(move || {
                    RECONNECT_CANCEL.with(|flag| *flag.borrow_mut() = Some(worker_cancel));
                    let result = subscribe(&executable, true).and_then(|receiver| {
                        snapshot_rows(&executable, width, true).map(|rows| (receiver, rows))
                    });
                    let _ = tx.send(result);
                });
                reconnect = Some(ReconnectAttempt {
                    rx,
                    cancel,
                    worker: Some(worker),
                });
                retry_at = Instant::now() + Duration::from_millis(500);
            }
        }
        let byte = match input_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(byte)) => byte,
            Ok(None) => {
                if let Some(key) = keys.flush() {
                    handle_key(&mut ui, key, terminal.0.is_some())?;
                }
                if terminal.0.is_none() || ui.closed {
                    break;
                }
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        };
        if keys.escape == 1 && byte != b'[' {
            if let Some(key) = keys.flush() {
                handle_key(&mut ui, key, terminal.0.is_some())?;
            }
            if ui.closed {
                break;
            }
        }
        if let Some(key) = keys.feed(byte) {
            handle_key(&mut ui, key, terminal.0.is_some())?;
            if ui.closed {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/presentation.rs"]
mod test_presentation;
#[cfg(test)]
#[path = "../tests/unit/selection.rs"]
mod test_selection;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
#[cfg(test)]
#[path = "../tests/unit/theme.rs"]
mod test_theme;
#[cfg(test)]
#[path = "../tests/unit/topology.rs"]
mod test_topology;
#[cfg(test)]
#[path = "../tests/unit/transport.rs"]
mod test_transport;
