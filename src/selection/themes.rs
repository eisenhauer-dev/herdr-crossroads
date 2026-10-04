use crate::input::Key;
use crate::keymap::{Action, Context};
use crate::theme::{self, NAMES};

use super::{Mode, Ui};

impl Ui {
    pub(super) fn themes_key(&mut self, key: Key) -> Option<String> {
        let Some(action) = self.keymap.action(Context::Themes, key) else {
            return None;
        };
        let previous = self.theme_cursor;
        match action {
            Action::TogglePreview => self.theme_table_preview = !self.theme_table_preview,
            Action::NextTheme => self.theme_cursor = (self.theme_cursor + 1).min(NAMES.len() - 1),
            Action::PreviousTheme => self.theme_cursor = self.theme_cursor.saturating_sub(1),
            Action::Cancel => self.mode = Mode::Select,
            Action::Save => match theme::save(self.config_dir.as_deref(), self.theme_cursor) {
                Ok(()) => {
                    self.theme_index = self.theme_cursor;
                    self.theme_invalid = false;
                    self.mode = Mode::Select;
                }
                Err(error) => self.theme_error = Some(format!("{error}; not saved")),
            },
            _ => {}
        }
        if previous != 0 && self.theme_cursor == 0 && !self.theme_invalid {
            self.auto_toast = self
                .auto_unsupported
                .as_ref()
                .map(|name| format!("Herdr theme {name} not yet supported; using Mocha"));
        }
        None
    }
}
