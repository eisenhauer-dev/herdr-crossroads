use std::cell::Cell;

use crate::input::Key;
use crate::keymap::{Action, Context, Keymap};
use crate::search::matches_field;

use super::{Mode, Ui};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HelpContext {
    Select,
}

pub(crate) struct HelpEntry {
    pub(crate) category: &'static str,
    pub(crate) keys: String,
    pub(crate) meaning: &'static str,
}

impl HelpContext {
    pub(crate) fn entries(&self, map: &Keymap) -> Vec<HelpEntry> {
        SELECT_KEYS
            .iter()
            .map(|&(category, action, meaning)| HelpEntry {
                category,
                meaning,
                keys: if action == "workspaces" {
                    format!(
                        "→ {} | ← {}",
                        map.display_labels(Context::Select, "next_workspace"),
                        map.display_labels(Context::Select, "previous_workspace")
                    )
                } else {
                    map.display_labels(Context::Select, action)
                },
            })
            .collect()
    }
}

const SELECT_KEYS: [(&str, &str, &str); 11] = [
    ("Navigation", "next_row", "next pane"),
    ("Navigation", "previous_row", "previous pane"),
    ("Navigation", "previous_tab", "previous tab"),
    ("Navigation", "next_tab", "next tab"),
    ("Navigation", "workspaces", "next/previous workspace"),
    ("Actions", "focus_pane", "focus selected pane"),
    ("Actions", "open_search", "search"),
    ("Actions", "reset_search", "reset search"),
    ("Actions", "open_themes", "choose theme"),
    ("Actions", "open_help", "show keys"),
    ("Actions", "close", "close Crossroads"),
];

pub(crate) struct Help {
    pub(crate) query: String,
    pub(crate) searching: bool,
    pub(crate) selected: Option<usize>,
    pub(crate) viewport_start: Cell<usize>,
    pub(crate) entries: Vec<HelpEntry>,
}

impl Help {
    pub(super) fn select(map: &Keymap) -> Self {
        Self {
            query: String::new(),
            searching: false,
            selected: Some(0),
            viewport_start: Cell::new(0),
            entries: HelpContext::Select.entries(map),
        }
    }

    pub(crate) fn visible(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                matches_field(&entry.keys, &self.query) || matches_field(entry.meaning, &self.query)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub(crate) fn selected_index(&self, visible: &[usize]) -> Option<usize> {
        self.selected
            .filter(|i| visible.contains(i))
            .or_else(|| visible.first().copied())
    }
}

impl Ui {
    pub(super) fn help_key(&mut self, key: Key) -> Option<String> {
        if self.help.searching {
            match key {
                Key::Enter | Key::Esc => self.help.searching = false,
                Key::Backspace => {
                    self.help.query.pop();
                }
                Key::Char(c) if !c.is_control() => self.help.query.push(c),
                _ => {}
            }
        } else {
            let Some(action) = self.keymap.action(Context::Help, key) else {
                return None;
            };
            let visible = self.help.visible();
            match action {
                Action::Close => self.mode = Mode::Select,
                Action::OpenSearch => self.help.searching = true,
                Action::ResetSearch => {
                    self.help.selected = self.help.selected_index(&visible).or(self.help.selected);
                    self.help.query.clear();
                }
                Action::NextCategory | Action::PreviousCategory => {
                    if let Some(at) = self
                        .help
                        .selected_index(&visible)
                        .and_then(|i| visible.iter().position(|v| *v == i))
                    {
                        let next = if action == Action::NextCategory {
                            (at + 1).min(visible.len() - 1)
                        } else {
                            at.saturating_sub(1)
                        };
                        self.help.selected = Some(visible[next]);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
