use std::cell::Cell;
use std::path::PathBuf;

#[cfg(unix)]
use serde_json::Value;

use super::input::Key;
use super::presentation::{clean, StatusColors};
use super::theme::{self, Theme, NAMES};
use super::topology::Row;

pub(super) fn matches_field(field: &str, query: &str) -> bool {
    let mut chars = field.to_lowercase().chars().collect::<Vec<_>>().into_iter();
    query
        .to_lowercase()
        .chars()
        .all(|c| chars.any(|next| next == c))
}

pub(super) fn match_positions(field: &str, query: &str) -> Vec<usize> {
    if query.is_empty() || !matches_field(field, query) {
        return Vec::new();
    }
    let lowered = field.to_lowercase();
    let query = query.to_lowercase();
    let mut wanted = query.chars().peekable();
    let mut positions = Vec::new();
    let mut source = field
        .chars()
        .enumerate()
        .flat_map(|(index, c)| std::iter::repeat_n(index, c.to_lowercase().count()));
    for lower in lowered.chars() {
        let index = source.next().unwrap();
        if wanted.peek() == Some(&lower) {
            wanted.next();
            if positions.last() != Some(&index) {
                positions.push(index);
            }
        }
        if wanted.peek().is_none() {
            break;
        }
    }
    positions
}

#[derive(PartialEq, Eq, Debug)]
pub(super) enum Mode {
    Select,
    Search,
    Help,
    Themes,
}

pub(super) struct Ui {
    pub(super) rows: Vec<Row>,
    pub(super) status_colors: StatusColors,
    pub(super) selected: Option<String>,
    pub(super) query: String,
    pub(super) mode: Mode,
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
        let selected = rows.first().map(|row| row.id.clone());
        let (auto_name, auto_unsupported) = theme::auto_choice();
        let mut ui = Self {
            rows,
            status_colors: StatusColors::default(),
            selected,
            query: String::new(),
            mode: Mode::Select,
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

    #[cfg(unix)]
    pub(super) fn status_event(&mut self, event: &Value) -> bool {
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
        let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.id == id && row.workspace_id == workspace_id && !row.agent.is_empty())
        else {
            return false;
        };
        row.status = clean(status);
        true
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
            if matches!(key, Key::Esc | Key::Char('?') | Key::Char('q')) {
                self.mode = Mode::Select;
            }
            return None;
        }
        if self.mode == Mode::Themes {
            let previous = self.theme_cursor;
            match key {
                Key::Char(' ') => self.theme_table_preview = !self.theme_table_preview,
                Key::Down | Key::Char('j') => {
                    self.theme_cursor = (self.theme_cursor + 1).min(NAMES.len() - 1)
                }
                Key::Up | Key::Char('k') => self.theme_cursor = self.theme_cursor.saturating_sub(1),
                Key::Esc => self.mode = Mode::Select,
                Key::Enter => match theme::save(self.config_dir.as_deref(), self.theme_cursor) {
                    Ok(()) => {
                        self.theme_index = self.theme_cursor;
                        self.theme_invalid = false;
                        self.mode = Mode::Select;
                    }
                    Err(_) => self.theme_error = Some("theme.json save failed; not saved".into()),
                },
                _ => {}
            }
            if previous != 0 && self.theme_cursor == 0 && !self.theme_invalid {
                self.auto_toast = self
                    .auto_unsupported
                    .as_ref()
                    .map(|name| format!("Herdr theme {name} not yet supported; using Mocha"));
            }
            return None;
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
        match key {
            Key::Char('t') => {
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
            Key::Char('/') => self.mode = Mode::Search,
            Key::Char('?') => self.mode = Mode::Help,
            Key::Char('q') | Key::Esc => self.closed = true,
            Key::Enter => {
                if let Some(i) = self.selected_index(&visible) {
                    self.closed = true;
                    return Some(self.rows[i].id.clone());
                }
            }
            Key::Char('r') => self.query.clear(),
            Key::Tab | Key::BackTab => {
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
                        if matches!(key, Key::Tab) {
                            blocks.get(at + 1)
                        } else {
                            at.checked_sub(1).and_then(|prev| blocks.get(prev))
                        }
                    }) {
                        self.selected = Some(self.rows[*at].id.clone());
                    }
                }
            }
            Key::Down | Key::Char('j') | Key::Up | Key::Char('k') => {
                if let Some(at) = self
                    .selected_index(&visible)
                    .and_then(|i| visible.iter().position(|v| *v == i))
                {
                    let next = if matches!(key, Key::Down | Key::Char('j')) {
                        (at + 1).min(visible.len() - 1)
                    } else {
                        at.saturating_sub(1)
                    };
                    self.selected = Some(self.rows[visible[next]].id.clone());
                }
            }
            Key::Left | Key::Char('h') | Key::Right | Key::Char('l') => {
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
                        let target = if matches!(key, Key::Left | Key::Char('h')) {
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
}
