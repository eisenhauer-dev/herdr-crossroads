use std::cell::Cell;

use crate::input::Key;
use crate::keymap::{Action, Context, Keymap};
use crate::search::matches_field;

use super::{Mode, Ui};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HelpContext {
    Select,
    Themes,
}

pub(crate) struct HelpEntry {
    pub(crate) action: Action,
    pub(crate) category: &'static str,
    pub(crate) keys: String,
    pub(crate) meaning: &'static str,
}

impl HelpContext {
    pub(crate) fn entries(&self, map: &Keymap) -> Vec<HelpEntry> {
        let (context, keys) = match self {
            Self::Select => (Context::Select, &SELECT_KEYS[..]),
            Self::Themes => (Context::Themes, &THEMES_KEYS[..]),
        };
        keys.iter()
            .map(|&(category, action, meaning)| HelpEntry {
                action: map
                    .bindings
                    .iter()
                    .find(|b| b.context == context && b.action.name() == action)
                    .unwrap()
                    .action,
                category,
                meaning,
                keys: map.display_labels(context, action),
            })
            .collect()
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Themes => "Themes",
        }
    }
}

const THEMES_KEYS: [(&str, &str, &str); 6] = [
    ("Navigation", "next_theme", "next theme"),
    ("Navigation", "previous_theme", "previous theme"),
    ("Actions", "toggle_preview", "toggle list/table preview"),
    ("Actions", "save", "save theme"),
    ("Actions", "cancel", "cancel theme selection"),
    ("Actions", "open_help", "show keys"),
];

const SELECT_KEYS: [(&str, &str, &str); 12] = [
    ("Navigation", "next_row", "next pane"),
    ("Navigation", "previous_row", "previous pane"),
    ("Navigation", "previous_tab", "previous tab"),
    ("Navigation", "next_tab", "next tab"),
    ("Navigation", "next_workspace", "next workspace"),
    ("Navigation", "previous_workspace", "previous workspace"),
    ("Actions", "focus_pane", "focus selected pane"),
    ("Actions", "open_search", "search"),
    ("Actions", "reset_search", "reset search"),
    ("Actions", "open_themes", "choose theme"),
    ("Actions", "open_help", "show keys"),
    ("Actions", "close", "close Crossroads"),
];

pub(crate) struct Help {
    pub(crate) context: HelpContext,
    pub(crate) query: String,
    pub(crate) searching: bool,
    pub(crate) selected: Option<usize>,
    pub(crate) viewport_start: Cell<usize>,
    pub(crate) entries: Vec<HelpEntry>,
}

impl Help {
    pub(super) fn new(context: HelpContext, map: &Keymap) -> Self {
        Self {
            query: String::new(),
            searching: false,
            selected: Some(0),
            viewport_start: Cell::new(0),
            entries: context.entries(map),
            context,
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
            if key == Key::Enter {
                return self.activate_help();
            }
            let action = self.keymap.action(Context::Help, key)?;
            return self.help_action(action);
        }
        None
    }

    pub(super) fn activate_help(&mut self) -> Option<String> {
        let visible = self.help.visible();
        let action = self
            .help
            .entries
            .get(self.help.selected_index(&visible)?)?
            .action;
        self.mode = match self.help.context {
            HelpContext::Select => Mode::Select,
            HelpContext::Themes => Mode::Themes,
        };
        self.action(action)
    }

    pub(super) fn help_action(&mut self, action: Action) -> Option<String> {
        let visible = self.help.visible();
        match action {
            Action::Close => {
                self.mode = match self.help.context {
                    HelpContext::Select => Mode::Select,
                    HelpContext::Themes => Mode::Themes,
                };
            }
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
                        (at + 1) % visible.len()
                    } else {
                        at.checked_sub(1).unwrap_or(visible.len() - 1)
                    };
                    self.help.selected = Some(visible[next]);
                }
            }
            _ => {}
        }
        None
    }
}
