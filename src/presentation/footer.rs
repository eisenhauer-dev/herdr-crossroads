use ratatui::buffer::CellWidth;

use super::{clipped, columns, wrapped};
use crate::keymap::Context;
use crate::selection::{Mode, Ui};

impl Ui {
    pub(crate) fn search_prompt_geometry(&self, width: usize) -> (&'static str, usize) {
        let badge = match self.mode {
            Mode::Select => "Select",
            Mode::Search => "Search",
            Mode::Help => "Help",
            Mode::Themes => "Themes",
        };
        (badge, width.saturating_sub(4 + badge.len() + 1))
    }

    pub(super) fn search_prompt(
        &self,
        context: Context,
        query: &str,
        searching: bool,
        width: usize,
    ) -> String {
        let (badge, available) = self.search_prompt_geometry(width);
        let text = if !query.is_empty() || searching {
            clipped(&format!("Search: {query}"), available)
        } else {
            let text = format!(
                "Search: {} to search",
                self.keymap.display_labels(context, "open_search")
            );
            let pages = wrapped(&text, available);
            if pages.len() > 1 {
                self.hint_more.set(true);
            }
            clipped(&pages[self.hint_scroll.get() % pages.len()], available)
        };
        format!(
            "{text}{}{badge}",
            " ".repeat(available.saturating_sub(text.cell_width() as usize) + 1)
        )
    }

    fn hint_lines(&self, context: Context, width: usize) -> Vec<String> {
        let actions: &[(&str, &str)] = match context {
            Context::Select => &[
                ("next_row", "↓pane"),
                ("previous_row", "↑pane"),
                ("previous_tab", "↑tab"),
                ("next_tab", "↓tab"),
                ("next_workspace", "↓ws"),
                ("previous_workspace", "↑ws"),
                ("focus_pane", "focus"),
                ("close", "exit"),
                ("open_help", "keys"),
            ],
            Context::Help => &[
                ("next_category", "↓move"),
                ("previous_category", "↑move"),
                ("close", "close keys"),
            ],
            Context::Themes => &[
                ("next_theme", "↓theme"),
                ("previous_theme", "↑theme"),
                (
                    "toggle_preview",
                    if self.theme_table_preview {
                        "list"
                    } else {
                        "table"
                    },
                ),
                ("save", "save"),
                ("cancel", "cancel"),
            ],
        };
        let mut lines: Vec<String> = Vec::new();
        for &(action, meaning) in actions {
            let unit = format!("{}: {meaning}", self.keymap.footer_labels(context, action));
            if let Some(line) = lines.last_mut().filter(|line| {
                columns(line) + columns(" · ") + columns(&unit) <= width.saturating_sub(4)
            }) {
                line.push_str(" · ");
                line.push_str(&unit);
            } else {
                lines.push(unit);
            }
        }
        lines
    }

    pub(crate) fn hint_rows(&self, context: Context, width: usize, height: usize) -> usize {
        let minimum = if context == Context::Themes { 2 } else { 1 };
        let table_minimum = if height <= 6 {
            3
        } else {
            5 + usize::from(width >= 110 && height >= 8)
        };
        let minimum_body = match context {
            Context::Select => table_minimum,
            Context::Help => 4,
            Context::Themes if self.theme_table_preview => table_minimum + 1,
            Context::Themes => 2,
        };
        self.hint_lines(context, width)
            .len()
            .max(minimum)
            .min(3)
            .min(
                height
                    .saturating_sub(minimum_body + usize::from(self.theme_notice().is_some()))
                    .max(1),
            )
    }

    pub(crate) fn hints(
        &self,
        context: Context,
        width: usize,
        budget: usize,
        ansi: bool,
    ) -> Vec<String> {
        let lines = self.hint_lines(context, width);
        let budget = budget.max(1);
        let pages = lines.len().div_ceil(budget);
        if ansi && pages > 1 {
            self.hint_more.set(true);
        }
        let start = if ansi {
            (self.hint_scroll.get() % pages) * budget
        } else {
            0
        };
        let mut result: Vec<_> = lines
            .into_iter()
            .skip(start)
            .take(if ansi { budget } else { usize::MAX })
            .map(|s| format!("  {s}"))
            .collect();
        if ansi {
            result.resize(budget, String::new());
        }
        result
    }
}
