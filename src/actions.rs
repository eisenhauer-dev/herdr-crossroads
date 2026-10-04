use std::io;

use crate::focus::focus;
use crate::input::Key;
use crate::selection::Ui;
use crate::terminal::draw;

pub(crate) fn handle_key(ui: &mut Ui, key: Key, terminal: bool) -> io::Result<()> {
    if let Some(id) = ui.key(key) {
        focus(&id)?;
    }
    if !ui.closed {
        draw(ui, terminal)?;
    }
    Ok(())
}
