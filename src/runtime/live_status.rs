use std::ffi::OsStr;
use std::io;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::presentation::clean;
use crate::selection::Ui;
use crate::terminal::draw;
use crate::topology::{snapshot_rows, Row};
use crate::transport::{subscribe, StatusMessage, StatusSubscription, RECONNECT_CANCEL};

struct ReconnectAttempt {
    rx: mpsc::Receiver<io::Result<(StatusSubscription, Vec<Row>)>>,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for ReconnectAttempt {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(super) fn bootstrap(status: &mut Option<StatusSubscription>, ui: &mut Ui) {
    if let Some(receiver) = &status {
        for _ in 0..1024 {
            match receiver.rx.try_recv() {
                Ok(StatusMessage::Event(event)) => {
                    ui.status_event(&event);
                }
                Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                    *status = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }
}

impl Ui {
    pub(crate) fn status_event(&mut self, event: &Value) -> bool {
        apply_status(&mut self.rows, event)
    }
}

pub(crate) fn apply_status(rows: &mut [Row], event: &Value) -> bool {
    if event["event"] != "pane.agent_status_changed" {
        return false;
    }
    let data = &event["data"];
    let (Some(id), Some(workspace_id), Some(status)) = (
        data["pane_id"].as_str(),
        data["workspace_id"].as_str(),
        data["agent_status"].as_str(),
    ) else {
        return false;
    };
    if !matches!(status, "working" | "blocked" | "done" | "idle" | "unknown") {
        return false;
    }
    let Some(row) = rows
        .iter_mut()
        .find(|row| row.id == id && row.workspace_id == workspace_id && !row.agent.is_empty())
    else {
        return false;
    };
    row.status = clean(status);
    true
}

pub(super) struct LiveStatus {
    // Drop staged state, then cancel/join a retry, before dropping the active subscription.
    pending: Option<(StatusSubscription, Vec<Row>)>,
    reconnect: Option<ReconnectAttempt>,
    status: Option<StatusSubscription>,
    enabled: bool,
    retry_at: Instant,
}

impl LiveStatus {
    pub(super) fn new(enabled: bool, status: Option<StatusSubscription>) -> Self {
        Self {
            enabled,
            status,
            retry_at: Instant::now() + Duration::from_millis(500),
            reconnect: None,
            pending: None,
        }
    }

    pub(super) fn poll(
        &mut self,
        ui: &mut Ui,
        terminal: bool,
        executable: &OsStr,
        width: usize,
    ) -> io::Result<()> {
        let mut disconnected = false;
        if let Some(receiver) = &self.status {
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
                draw(&ui, terminal)?;
            }
        }
        if disconnected {
            self.status = None;
            self.retry_at = Instant::now() + Duration::from_millis(500);
        }
        if let Some(worker) = &self.reconnect {
            match worker.rx.try_recv() {
                Ok(Ok((receiver, rows))) => {
                    self.pending = Some((receiver, rows));
                    self.reconnect = None;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.reconnect = None;
                    self.retry_at = Instant::now() + Duration::from_millis(500);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some((receiver, staged)) = &mut self.pending {
            for _ in 0..1024 {
                match receiver.rx.try_recv() {
                    Ok(StatusMessage::Event(event)) => {
                        apply_status(staged, &event);
                    }
                    Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
            if disconnected {
                self.pending = None;
                self.retry_at = Instant::now() + Duration::from_millis(500);
            } else {
                let (receiver, staged) = self.pending.take().unwrap();
                ui.rows = staged;
                self.status = Some(receiver);
                draw(&ui, terminal)?;
            }
        }
        if self.enabled
            && !ui.configuration_failed()
            && self.status.is_none()
            && self.reconnect.is_none()
            && self.pending.is_none()
            && Instant::now() >= self.retry_at
        {
            let (tx, rx) = mpsc::sync_channel(1);
            let executable = executable.to_owned();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let worker = thread::spawn(move || {
                RECONNECT_CANCEL.with(|flag| *flag.borrow_mut() = Some(worker_cancel));
                let result = subscribe(&executable, true).and_then(|receiver| {
                    snapshot_rows(&executable, width, true).map(|rows| (receiver, rows))
                });
                let _ = tx.send(result);
            });
            self.reconnect = Some(ReconnectAttempt {
                rx,
                cancel,
                worker: Some(worker),
            });
            self.retry_at = Instant::now() + Duration::from_millis(500);
        }
        Ok(())
    }
}
