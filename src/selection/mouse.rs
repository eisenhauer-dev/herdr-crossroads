use crate::input::{Key, Mouse, MouseButton};
use crate::keymap::Action;

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
    pub(crate) scroll_max: usize,
}

impl Ui {
    fn mouse_signature(&self) -> String {
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{:?}:{}:{:?}:{:?}",
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
                .collect::<Vec<_>>(),
            (
                self.viewport_start.get(),
                self.help.viewport_start.get(),
                self.theme_viewport_start.get()
            )
        )
    }

    pub(crate) fn begin_mouse_layout(&self) {
        let signature = self.mouse_signature();
        let mut mouse = self.mouse.borrow_mut();
        mouse.signature = signature;
        mouse.hits.clear();
        mouse.scroll_max = 0;
    }

    pub(crate) fn finish_mouse_layout(&self) {
        let mut mouse = self.mouse.borrow_mut();
        mouse.signature = self.mouse_signature();
    }

    pub(crate) fn hit(&self, x: usize, y: usize, width: usize, target: Hit) {
        if width > 0 {
            self.mouse.borrow_mut().hits.push((x, y, width, target));
        }
    }

    pub(super) fn action(&mut self, action: Action) -> Option<String> {
        self.follow_selection = true;
        self.theme_error = None;
        self.auto_toast = None;
        match self.mode {
            Mode::Select => self.select_action(action, &self.visible()),
            Mode::Help => self.help_action(action),
            Mode::Themes => self.themes_action(action),
            Mode::Search => None,
        }
    }

    pub(crate) fn mouse_input(&mut self, event: Mouse) -> Option<String> {
        if !self.keymap.mouse_enabled {
            return None;
        }
        let (width, height) = self.popup_size?;
        if event.x >= width || event.y >= height {
            return None;
        }
        if event.button == MouseButton::Release {
            return None;
        }
        if self.mouse.borrow().signature != self.mouse_signature() {
            return None;
        }
        if matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
            let down = event.button == MouseButton::WheelDown;
            if self.configuration_failed() || width < 80 || height < 15 {
                return self.keyboard(if down { Key::Down } else { Key::Up });
            }
            let start = if self.mode == Mode::Help {
                &self.help.viewport_start
            } else if self.mode == Mode::Themes && !self.theme_table_preview {
                &self.theme_viewport_start
            } else {
                &self.viewport_start
            };
            let max = self.mouse.borrow().scroll_max;
            start.set(if down {
                start.get().saturating_add(1).min(max)
            } else {
                start.get().saturating_sub(1).min(max)
            });
            self.follow_selection = false;
            return None;
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
        let target = target?;
        let activate = event.button == MouseButton::Left;
        let result = match &target {
            Hit::Pane(id) => {
                self.theme_error = None;
                self.auto_toast = None;
                self.selected = Some(id.clone());
                if activate {
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
                if activate {
                    self.action(Action::Save)
                } else {
                    None
                }
            }
            Hit::Help(index) => {
                self.help.selected = Some(*index);
                if activate {
                    self.activate_help()
                } else {
                    None
                }
            }
            Hit::Action(action) if activate => self.action(*action),
            Hit::Key(key) if activate => self.keyboard(*key),
            Hit::Search if activate => {
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
            _ => None,
        };
        let signature = self.mouse_signature();
        let mut mouse = self.mouse.borrow_mut();
        mouse.signature = signature;
        result
    }
}
