use std::cell::Cell;
use std::path::PathBuf;

use super::input::Key;
use super::keymap::{Action, Context, Keymap};
use super::search::matches_field;
use super::status_colors::StatusColors;
use super::theme::{self, Theme, NAMES};
use super::topology::Row;

mod help;
mod themes;

use help::Help;

#[derive(PartialEq, Eq, Debug)]
pub(super) enum Mode {
    Select,
    Search,
    Help,
    Themes,
}

pub(super) struct Ui {
    pub(super) keymap: Keymap,
    pub(super) keymap_errors: Vec<String>,
    pub(super) status_errors: super::config::Diagnostics,
    pub(super) theme_diagnostics: super::config::Diagnostics,
    pub(super) error_scroll: Cell<usize>,
    pub(super) hint_scroll: Cell<usize>,
    pub(super) hint_more: Cell<bool>,
    pub(super) rows: Vec<Row>,
    pub(super) status_colors: StatusColors,
    pub(super) selected: Option<String>,
    pub(super) query: String,
    pub(super) mode: Mode,
    pub(super) help: Help,
    pub(super) closed: bool,
    pub(super) width: usize,
    pub(super) popup_size: Option<(usize, usize)>,
    pub(super) viewport_start: Cell<usize>,
    pub(super) last_selected_at: Cell<Option<usize>>,
    pub(super) theme_index: usize,
    pub(super) theme_cursor: usize,
    pub(super) theme_table_preview: bool,
    pub(super) theme_error: Option<String>,
    pub(super) theme_invalid: bool,
    pub(super) auto_name: &'static str,
    pub(super) auto_unsupported: Option<String>,
    pub(super) auto_toast: Option<String>,
    pub(super) config_dir: Option<PathBuf>,
}

impl Ui {
    pub(super) fn new(rows: Vec<Row>) -> Self {
        let (auto_name, auto_unsupported) = theme::auto_choice();
        Self::with_theme_choice(rows, auto_name, auto_unsupported)
    }

    pub(super) fn with_theme_choice(
        rows: Vec<Row>,
        auto_name: &'static str,
        auto_unsupported: Option<String>,
    ) -> Self {
        let selected = rows.first().map(|row| row.id.clone());
        let mut ui = Self {
            keymap: Keymap::default(),
            keymap_errors: Vec::new(),
            status_errors: super::config::Diagnostics::default(),
            theme_diagnostics: super::config::Diagnostics::default(),
            error_scroll: Cell::new(0),
            hint_scroll: Cell::new(0),
            hint_more: Cell::new(false),
            rows,
            status_colors: StatusColors::default(),
            selected,
            query: String::new(),
            mode: Mode::Select,
            help: Help::select(&Keymap::default()),
            closed: false,
            width: 160,
            popup_size: None,
            viewport_start: Cell::new(0),
            last_selected_at: Cell::new(None),
            theme_index: 0,
            theme_cursor: 0,
            theme_table_preview: false,
            theme_error: None,
            theme_invalid: false,
            auto_name,
            auto_unsupported: None,
            auto_toast: None,
            config_dir: None,
        };
        ui.set_auto_choice(auto_name, auto_unsupported);
        ui
    }

    pub(super) fn set_auto_choice(&mut self, name: &'static str, unsupported: Option<String>) {
        self.auto_name = name;
        self.auto_unsupported = unsupported;
        self.auto_toast = if self.theme_index == 0 && !self.theme_invalid {
            self.auto_unsupported
                .as_ref()
                .map(|name| format!("Herdr theme {name} not yet supported; using Mocha"))
        } else {
            None
        };
    }

    pub(super) fn theme_notice(&self) -> Option<&str> {
        self.theme_error.as_deref().or(self.auto_toast.as_deref())
    }

    pub(super) fn theme(&self) -> Theme {
        let name = if self.mode == Mode::Themes {
            NAMES[self.theme_cursor]
        } else {
            NAMES[self.theme_index]
        };
        Theme::named(if name == NAMES[0] && self.theme_invalid {
            "catppuccin"
        } else if name == NAMES[0] {
            self.auto_name
        } else {
            name
        })
    }

    pub(super) fn visible(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                [&row.workspace, &row.tab, &row.pane, &row.agent]
                    .iter()
                    .any(|field| matches_field(field, &self.query))
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub(super) fn selected_index(&self, visible: &[usize]) -> Option<usize> {
        visible
            .iter()
            .copied()
            .find(|&i| self.selected.as_deref() == Some(&self.rows[i].id))
            .or_else(|| visible.first().copied())
    }

    pub(super) fn key(&mut self, key: Key) -> Option<String> {
        if self.configuration_failed() {
            match key {
                Key::Up | Key::Char('k') => self
                    .error_scroll
                    .set(self.error_scroll.get().saturating_sub(1)),
                Key::Down | Key::Char('j') => self
                    .error_scroll
                    .set(self.error_scroll.get().saturating_add(1)),
                Key::Enter | Key::Esc | Key::Char('q') => self.closed = true,
                _ => {}
            }
            return None;
        }
        self.theme_error = None;
        self.auto_toast = None;
        if self
            .popup_size
            .is_some_and(|(width, height)| width < 80 || height < 15)
        {
            if matches!(key, Key::Enter | Key::Esc | Key::Char('q')) {
                self.closed = true;
            }
            return None;
        }
        if self.mode == Mode::Help {
            return self.help_key(key);
        }
        if self.mode == Mode::Themes {
            return self.themes_key(key);
        }
        if self.mode == Mode::Search {
            match key {
                Key::Enter | Key::Esc => self.mode = Mode::Select,
                Key::Backspace => {
                    self.query.pop();
                }
                Key::Char(c) if !c.is_control() => self.query.push(c),
                _ => {}
            }
            return None;
        }
        let visible = self.visible();
        let Some(action) = self.keymap.action(Context::Select, key) else {
            return None;
        };
        match action {
            Action::OpenThemes => {
                self.theme_cursor = self.theme_index;
                self.theme_table_preview = false;
                self.mode = Mode::Themes;
                if self.theme_cursor == 0 && !self.theme_invalid {
                    self.auto_toast = self
                        .auto_unsupported
                        .as_ref()
                        .map(|name| format!("Herdr theme {name} not yet supported; using Mocha"));
                }
            }
            Action::OpenSearch => self.mode = Mode::Search,
            Action::OpenHelp => {
                self.help = Help::select(&self.keymap);
                self.mode = Mode::Help;
            }
            Action::Close => self.closed = true,
            Action::FocusPane => {
                if let Some(i) = self.selected_index(&visible) {
                    self.closed = true;
                    return Some(self.rows[i].id.clone());
                }
            }
            Action::ResetSearch => self.query.clear(),
            Action::NextWorkspace | Action::PreviousWorkspace => {
                if let Some(i) = self.selected_index(&visible) {
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0
                                || self.rows[visible[at - 1]].workspace_id
                                    != self.rows[v].workspace_id)
                                .then_some(v)
                        })
                        .collect();
                    if let Some(at) = blocks.iter().rposition(|&b| b <= i).and_then(|at| {
                        if action == Action::NextWorkspace {
                            blocks.get(at + 1)
                        } else {
                            at.checked_sub(1).and_then(|prev| blocks.get(prev))
                        }
                    }) {
                        self.selected = Some(self.rows[*at].id.clone());
                    }
                }
            }
            Action::NextRow | Action::PreviousRow => {
                if let Some(at) = self
                    .selected_index(&visible)
                    .and_then(|i| visible.iter().position(|v| *v == i))
                {
                    let next = if action == Action::NextRow {
                        (at + 1).min(visible.len() - 1)
                    } else {
                        at.saturating_sub(1)
                    };
                    self.selected = Some(self.rows[visible[next]].id.clone());
                }
            }
            Action::PreviousTab | Action::NextTab => {
                if let Some(i) = self.selected_index(&visible) {
                    let current = &self.rows[i].tab_id;
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0 || self.rows[visible[at - 1]].tab_id != self.rows[v].tab_id)
                                .then_some(v)
                        })
                        .collect();
                    if let Some(at) = blocks.iter().position(|&b| &self.rows[b].tab_id == current) {
                        let target = if action == Action::PreviousTab {
                            at.checked_sub(1)
                        } else {
                            blocks.get(at + 1).map(|_| at + 1)
                        };
                        if let Some(next) = target {
                            self.selected = Some(self.rows[blocks[next]].id.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    pub(super) fn configuration_failed(&self) -> bool {
        !self.keymap_errors.is_empty() || !self.status_errors.is_empty()
    }
}
