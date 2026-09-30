use crate::selection::Ui;
use crate::topology::rows;
use std::io;

pub(super) const SNAPSHOT: &str = include_str!("../fixtures/topology.json");
pub(super) fn ui() -> Ui {
    let mut state = Ui::new(rows(SNAPSHOT, Some("p-self"), 160).unwrap());
    state.set_auto_choice("catppuccin", None);
    state
}

pub(super) fn without_match_marks(frame: &str) -> String {
    without_name_accent(frame)
        .replace("\x1b[4m", "")
        .replace("\x1b[24m", "")
}

pub(super) fn without_name_accent(frame: &str) -> String {
    frame
        .replace("\x1b[38;2;180;190;254m", "")
        .replace("\x1b[39m\x1b[24m", "\x1b[24m")
}

pub(super) fn without_status_ansi(line: &str) -> String {
    line.replace("\x1b[38;2;249;226;175m", "")
        .replace("\x1b[38;2;166;227;161m", "")
        .replace("\x1b[39m", "")
}

pub(super) fn selected_prefix(line: &str) -> bool {
    line.starts_with("  \x1b[48;5;236m> ")
}

pub(super) fn render(response: &str, own_pane: Option<&str>, width: usize) -> io::Result<String> {
    let mut state = Ui::new(rows(response, own_pane, width)?);
    state.set_auto_choice("catppuccin", None);
    Ok(state.frame_width(width))
}
