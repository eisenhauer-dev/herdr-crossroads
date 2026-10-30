use std::time::{Duration, Instant};

use crate::input::{Key, Mouse, MouseButton};
use crate::keymap::{Action, Context, Keymap};

use super::{Mode, Ui};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Pane(String),
    Theme(usize),
    Help(usize),
    Action(Action),
    Search,
    Key(Key),
}

#[derive(Default)]
pub(crate) struct MouseState {
    pub(crate) hits: Vec<(usize, usize, usize, Hit)>,
    pub(crate) signature: String,
    pub(crate) click: Option<(Hit, usize, usize, Instant)>,
}

impl Ui {
    fn mouse_signature(&self) -> String {
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{:?}:{}:{:?}",
            self.mode,
            self.selected,
            self.query,
            self.help.selected,
            self.help.query,
            self.help.searching,
            self.theme_cursor,
            self.theme_table_preview,
            self.popup_size,
            self.hint_scroll.get(),
            self.rows
                .iter()
                .map(|r| (&r.id, &r.workspace_id, &r.tab_id, &r.status))
                .collect::<Vec<_>>()
        )
    }

    pub(crate) fn begin_mouse_layout(&self) {
        let signature = self.mouse_signature();
        let mut mouse = self.mouse.borrow_mut();
        if mouse.signature != signature {
            mouse.click = None;
        }
        mouse.signature = signature;
        mouse.hits.clear();
    }

    pub(crate) fn finish_mouse_layout(&self) {
        let mut mouse = self.mouse.borrow_mut();
        if mouse.click.as_ref().is_some_and(|(target, x, y, _)| {
            !mouse.hits.iter().any(|(left, row, width, hit)| {
                row == y && x >= left && x - left < *width && hit == target
            })
        }) {
            mouse.click = None;
        }
        mouse.signature = self.mouse_signature();
    }

    pub(crate) fn hit(&self, x: usize, y: usize, width: usize, target: Hit) {
        if width > 0 {
            self.mouse.borrow_mut().hits.push((x, y, width, target));
        }
    }

    pub(super) fn action(&mut self, action: Action) -> Option<String> {
        self.theme_error = None;
        self.auto_toast = None;
        match self.mode {
            Mode::Select => self.select_action(action, &self.visible()),
            Mode::Help => self.help_action(action),
            Mode::Themes => self.themes_action(action),
            Mode::Search => None,
        }
    }

    pub(crate) fn mouse_input(&mut self, event: Mouse, now: Instant) -> Option<String> {
        if !self.keymap.mouse_enabled {
            return None;
        }
        let (width, height) = self.popup_size?;
        if event.x >= width || event.y >= height {
            self.mouse.borrow_mut().click = None;
            return None;
        }
        if event.button == MouseButton::Release {
            return None;
        }
        if self.mouse.borrow().signature != self.mouse_signature() {
            self.mouse.borrow_mut().click = None;
            return None;
        }
        if matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
            self.mouse.borrow_mut().click = None;
            let key = Key::Char(if event.button == MouseButton::WheelDown {
                'j'
            } else {
                'k'
            });
            if self.configuration_failed()
                || self.mode == Mode::Search
                || (self.mode == Mode::Help && self.help.searching)
                || width < 80
                || height < 15
            {
                return self.keyboard(key);
            }
            let context = match self.mode {
                Mode::Select => Context::Select,
                Mode::Help => Context::Help,
                Mode::Themes => Context::Themes,
                Mode::Search => unreachable!(),
            };
            return self.action(Keymap::default().action(context, key)?);
        }
        let target = self
            .mouse
            .borrow()
            .hits
            .iter()
            .rev()
            .find_map(|(x, y, width, target)| {
                (*y == event.y && event.x >= *x && event.x - *x < *width).then(|| target.clone())
            });
        let previous = self.mouse.borrow_mut().click.take();
        let target = target?;
        let help_again = previous
            .as_ref()
            .is_some_and(|(old, _, _, _)| *old == target);
        let double = previous.is_some_and(|(old, x, y, at)| {
            old == target
                && x == event.x
                && y == event.y
                && now.saturating_duration_since(at) <= Duration::from_millis(400)
        });
        let result = match &target {
            Hit::Pane(id) => {
                self.theme_error = None;
                self.auto_toast = None;
                self.selected = Some(id.clone());
                if double {
                    self.mode = Mode::Select;
                    self.action(Action::FocusPane)
                } else {
                    None
                }
            }
            Hit::Theme(index) => {
                let previous = self.theme_cursor;
                self.theme_error = None;
                self.auto_toast = None;
                self.theme_cursor = *index;
                self.theme_cursor_changed(previous);
                if double {
                    self.action(Action::Save)
                } else {
                    None
                }
            }
            Hit::Help(index) => {
                if help_again && self.help.selected_index(&self.help.visible()) == Some(*index) {
                    self.activate_help()
                } else {
                    self.help.selected = Some(*index);
                    None
                }
            }
            Hit::Action(action) => self.action(*action),
            Hit::Key(key) => self.keyboard(*key),
            Hit::Search => {
                match self.mode {
                    Mode::Select => {
                        self.action(Action::OpenSearch);
                    }
                    Mode::Themes if self.theme_table_preview => {
                        self.search_return = Mode::Themes;
                        self.mode = Mode::Search;
                    }
                    Mode::Help => self.help.searching = true,
                    _ => {}
                }
                None
            }
        };
        let signature = self.mouse_signature();
        let mut mouse = self.mouse.borrow_mut();
        mouse.signature = signature;
        if (matches!(target, Hit::Pane(_) | Hit::Theme(_)) && !double)
            || (matches!(target, Hit::Help(_)) && !help_again)
        {
            mouse.click = Some((target, event.x, event.y, now));
        }
        result
    }
}
