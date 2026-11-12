use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;

use super::input::Key;
use super::keymap::{Action, Context, Keymap};
use super::search::matches_field;
use super::status_colors::StatusColors;
use super::theme::{self, Theme, NAMES};
use super::topology::Row;

mod help;
mod mouse;
mod themes;
pub(crate) use mouse::{Hit, MouseState};

use help::{Help, HelpContext};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Mode {
    Select,
    Search,
    Help,
    Themes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TableView {
    Select,
    Agent,
}

pub(super) struct Ui {
    pub(crate) mouse: std::cell::RefCell<MouseState>,
    pub(super) keymap: Keymap,
    pub(super) keymap_errors: Vec<String>,
    pub(super) status_errors: super::config::Diagnostics,
    pub(super) theme_diagnostics: super::config::Diagnostics,
    pub(super) error_scroll: Cell<usize>,
    pub(super) hint_scroll: Cell<usize>,
    pub(super) hint_more: Cell<bool>,
    pub(super) rows: Vec<Row>,
    pub(super) table_view: TableView,
    pub(super) agent_status_filter: Option<&'static str>,
    // Opening-snapshot membership is not recaptured when reconnect replaces rows.
    agent_panes: HashSet<String>,
    blocked_agents: Vec<String>,
    pub(super) status_colors: StatusColors,
    pub(super) selected: Option<String>,
    pub(super) query: String,
    pub(super) mode: Mode,
    pub(super) search_return: Mode,
    pub(super) help: Help,
    pub(super) closed: bool,
    pub(super) width: usize,
    pub(super) popup_size: Option<(usize, usize)>,
    pub(super) viewport_start: Cell<usize>,
    pub(super) follow_selection: bool,
    pub(super) last_selected_at: Cell<Option<usize>>,
    pub(super) theme_index: usize,
    pub(super) theme_cursor: usize,
    pub(super) theme_viewport_start: Cell<usize>,
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
        let agent_panes = rows
            .iter()
            .filter(|row| !row.agent.is_empty())
            .map(|row| row.id.clone())
            .collect();
        let mut ui = Self {
            mouse: std::cell::RefCell::new(MouseState::default()),
            keymap: Keymap::default(),
            keymap_errors: Vec::new(),
            status_errors: super::config::Diagnostics::default(),
            theme_diagnostics: super::config::Diagnostics::default(),
            error_scroll: Cell::new(0),
            hint_scroll: Cell::new(0),
            hint_more: Cell::new(false),
            rows,
            table_view: TableView::Select,
            agent_status_filter: None,
            agent_panes,
            blocked_agents: Vec::new(),
            status_colors: StatusColors::default(),
            selected,
            query: String::new(),
            mode: Mode::Select,
            search_return: Mode::Select,
            help: Help::new(HelpContext::Select, &Keymap::default()),
            closed: false,
            width: 160,
            popup_size: None,
            viewport_start: Cell::new(0),
            follow_selection: true,
            last_selected_at: Cell::new(None),
            theme_index: 0,
            theme_cursor: 0,
            theme_viewport_start: Cell::new(0),
            theme_table_preview: false,
            theme_error: None,
            theme_invalid: false,
            auto_name,
            auto_unsupported: None,
            auto_toast: None,
            config_dir: None,
        };
        ui.set_auto_choice(auto_name, auto_unsupported);
        ui.reconcile_agent_order();
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
        let name = if self.mode == Mode::Themes
            || (self.mode == Mode::Search && self.search_return == Mode::Themes)
            || (self.mode == Mode::Help && self.help.context == HelpContext::Themes)
        {
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
        let mut visible: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                self.table_view == TableView::Select || self.agent_panes.contains(&row.id)
            })
            .filter(|(_, row)| {
                self.table_view == TableView::Select
                    || self.agent_status_filter.is_none_or(|status| {
                        row.status == status || (status == "unknown" && row.status.is_empty())
                    })
            })
            .filter(|(_, row)| {
                [&row.workspace, &row.tab, &row.pane, &row.agent]
                    .iter()
                    .any(|field| matches_field(field, &self.query))
                    || (self.table_view == TableView::Agent
                        && [
                            row.agent_metadata.title.as_deref(),
                            row.agent_metadata
                                .repository
                                .as_ref()
                                .map(|(_, name)| name.as_str()),
                            row.agent_metadata.branch.as_deref(),
                        ]
                        .into_iter()
                        .flatten()
                        .any(|field| matches_field(field, &self.query)))
            })
            .map(|(i, _)| i)
            .collect();
        if self.table_view == TableView::Agent {
            visible.sort_by_key(|&i| {
                self.blocked_agents
                    .iter()
                    .position(|id| *id == self.rows[i].id)
                    .unwrap_or(self.blocked_agents.len())
            });
        }
        visible
    }

    pub(super) fn set_table_view(&mut self, view: TableView) {
        self.table_view = view;
        self.mouse.borrow_mut().hits.clear();
        self.reconcile_selection();
        self.follow_selection = true;
    }

    pub(super) fn table_context(&self) -> Context {
        match self.table_view {
            TableView::Select => Context::Select,
            TableView::Agent => Context::Agent,
        }
    }

    pub(super) fn has_agent_panes(&self) -> bool {
        self.rows
            .iter()
            .any(|row| self.agent_panes.contains(&row.id))
    }

    pub(super) fn replace_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.reconcile_agent_order();
        self.reconcile_selection();
    }

    pub(crate) fn reconcile_agent_order(&mut self) {
        self.blocked_agents.retain(|id| {
            self.rows
                .iter()
                .any(|row| row.id == *id && row.status == "blocked")
        });
        for row in &self.rows {
            if row.status == "blocked"
                && self.agent_panes.contains(&row.id)
                && !self.blocked_agents.contains(&row.id)
            {
                self.blocked_agents.push(row.id.clone());
            }
        }
        if self.table_view == TableView::Agent && self.agent_status_filter.is_some() {
            self.reconcile_selection();
        }
    }

    fn reconcile_selection(&mut self) {
        self.selected = self
            .selected_index(&self.visible())
            .map(|i| self.rows[i].id.clone());
    }

    pub(super) fn selected_index(&self, visible: &[usize]) -> Option<usize> {
        visible
            .iter()
            .copied()
            .find(|&i| self.selected.as_deref() == Some(&self.rows[i].id))
            .or_else(|| visible.first().copied())
    }

    pub(super) fn key(&mut self, key: Key) -> Option<String> {
        if let Key::Mouse(mouse) = key {
            return self.mouse_input(mouse);
        }
        self.keyboard(key)
    }

    fn keyboard(&mut self, key: Key) -> Option<String> {
        self.follow_selection = true;
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
                Key::Enter | Key::Esc => self.mode = self.search_return,
                Key::Backspace => {
                    self.query.pop();
                }
                Key::Char(c) if !c.is_control() => self.query.push(c),
                _ => {}
            }
            if self.table_view == TableView::Agent {
                self.reconcile_selection();
            }
            return None;
        }
        let visible = self.visible();
        let action = self.keymap.action(self.table_context(), key)?;
        self.select_action(action, &visible)
    }

    fn select_action(&mut self, action: Action, visible: &[usize]) -> Option<String> {
        match action {
            Action::FilterAll
            | Action::FilterBlocked
            | Action::FilterDone
            | Action::FilterWorking
            | Action::FilterIdle
            | Action::FilterUnknown => {
                self.agent_status_filter = match action {
                    Action::FilterBlocked => Some("blocked"),
                    Action::FilterDone => Some("done"),
                    Action::FilterWorking => Some("working"),
                    Action::FilterIdle => Some("idle"),
                    Action::FilterUnknown => Some("unknown"),
                    _ => None,
                };
                self.mouse.borrow_mut().hits.clear();
                self.reconcile_selection();
            }
            Action::OpenAgent => self.set_table_view(TableView::Agent),
            Action::OpenSelect => self.set_table_view(TableView::Select),
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
            Action::OpenSearch => {
                self.search_return = Mode::Select;
                self.mode = Mode::Search;
            }
            Action::OpenHelp => {
                self.help = Help::new(
                    match self.table_view {
                        TableView::Select => HelpContext::Select,
                        TableView::Agent => HelpContext::Agent,
                    },
                    &self.keymap,
                );
                self.mode = Mode::Help;
            }
            Action::Close => self.closed = true,
            Action::FocusPane => {
                if let Some(i) = self.selected_index(visible) {
                    self.closed = true;
                    return Some(self.rows[i].id.clone());
                }
            }
            Action::ResetSearch => {
                self.query.clear();
                if self.table_view == TableView::Agent {
                    self.reconcile_selection();
                }
            }
            Action::NextWorkspace | Action::PreviousWorkspace => {
                if let Some(i) = self.selected_index(visible) {
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0
                                || self.rows[visible[at - 1]].workspace_id
                                    != self.rows[v].workspace_id)
                                .then_some(at)
                        })
                        .collect();
                    let position = visible.iter().position(|&v| v == i).unwrap();
                    if let Some(at) = blocks.iter().rposition(|&b| b <= position) {
                        let next = if action == Action::NextWorkspace {
                            (at + 1) % blocks.len()
                        } else {
                            at.checked_sub(1).unwrap_or(blocks.len() - 1)
                        };
                        self.selected = Some(self.rows[visible[blocks[next]]].id.clone());
                    }
                }
            }
            Action::NextRow | Action::PreviousRow => {
                if let Some(at) = self
                    .selected_index(visible)
                    .and_then(|i| visible.iter().position(|v| *v == i))
                {
                    let next = if action == Action::NextRow {
                        (at + 1) % visible.len()
                    } else {
                        at.checked_sub(1).unwrap_or(visible.len() - 1)
                    };
                    self.selected = Some(self.rows[visible[next]].id.clone());
                }
            }
            Action::PreviousTab | Action::NextTab => {
                if let Some(i) = self.selected_index(visible) {
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0 || self.rows[visible[at - 1]].tab_id != self.rows[v].tab_id)
                                .then_some(at)
                        })
                        .collect();
                    let position = visible.iter().position(|&v| v == i).unwrap();
                    if let Some(at) = blocks.iter().rposition(|&b| b <= position) {
                        let next = if action == Action::PreviousTab {
                            at.checked_sub(1).unwrap_or(blocks.len() - 1)
                        } else {
                            (at + 1) % blocks.len()
                        };
                        self.selected = Some(self.rows[visible[blocks[next]]].id.clone());
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
