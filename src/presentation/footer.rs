use ratatui::buffer::CellWidth;
use ratatui::{style::Style, text::Span};

use super::{clipped, columns, wrapped};
use crate::input::Key;
use crate::keymap::{Action, Context};
use crate::selection::{Hit, Mode, TableView, Ui};

impl Ui {
    pub(crate) fn search_prompt_geometry(&self, width: usize) -> (String, usize) {
        let badge = match if self.mode == Mode::Search {
            self.search_return
        } else {
            self.mode
        } {
            Mode::Select | Mode::Search => match self.table_view {
                TableView::Select => "MODE: SELECT",
                TableView::Agent => "MODE: AGENT",
            },
            Mode::Help => "MODE: HELP",
            Mode::Themes => "MODE: THEMES",
        };
        let badge = if badge == "MODE: AGENT" {
            format!("{badge} [{}]", self.agent_status_filter.unwrap_or("all"))
        } else {
            badge.to_owned()
        };
        let available = width.saturating_sub(4 + badge.len() + 1);
        (badge, available)
    }

    pub(crate) fn search_text(&self, query: &str, width: usize) -> String {
        let (_, available) = self.search_prompt_geometry(width);
        let label = clipped("Search: ", available.saturating_sub(1));
        let budget = available.saturating_sub(label.cell_width() as usize + 1);
        let source = Span::raw(query);
        let graphemes: Vec<_> = source.styled_graphemes(Style::default()).collect();
        let mut start = query.len();
        let mut used = 0;
        for grapheme in graphemes.iter().rev() {
            let cells = grapheme.symbol.cell_width() as usize;
            if used + cells > budget {
                break;
            }
            used += cells;
            start -= grapheme.symbol.len();
        }
        format!("{label}{}", &query[start..])
    }

    pub(super) fn search_prompt(
        &self,
        context: Context,
        query: &str,
        searching: bool,
        width: usize,
        ansi: bool,
    ) -> String {
        let (badge, available) = self.search_prompt_geometry(width);
        let text = if !query.is_empty() || searching {
            self.search_text(query, width)
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
            "{text}{}{}{badge}{}",
            " ".repeat(available.saturating_sub(text.cell_width() as usize) + 1),
            if ansi { "\x1b[7m" } else { "" },
            if ansi { "\x1b[27m" } else { "" }
        )
    }

    fn hint_lines(&self, context: Context, width: usize) -> Vec<Vec<(String, Hit)>> {
        let actions: &[(Hit, &str)] = match context {
            Context::Select | Context::Agent => &[
                (Hit::Action(Action::NextRow), "↓pane"),
                (Hit::Action(Action::PreviousRow), "↑pane"),
                (Hit::Action(Action::PreviousTab), "↑tab"),
                (Hit::Action(Action::NextTab), "↓tab"),
                (Hit::Action(Action::NextWorkspace), "↓ws"),
                (Hit::Action(Action::PreviousWorkspace), "↑ws"),
                (Hit::Action(Action::FocusPane), "focus"),
                (Hit::Action(Action::Close), "exit"),
                (Hit::Action(Action::OpenHelp), "keys"),
                if context == Context::Agent {
                    (Hit::Action(Action::OpenSelect), "select")
                } else {
                    (Hit::Action(Action::OpenAgent), "agent")
                },
            ],
            Context::Help => &[
                (Hit::Action(Action::NextCategory), "↓move"),
                (Hit::Action(Action::PreviousCategory), "↑move"),
                (Hit::Key(Key::Enter), "run"),
                (Hit::Action(Action::Close), "close keys"),
            ],
            Context::Themes => &[
                (Hit::Action(Action::NextTheme), "↓theme"),
                (Hit::Action(Action::PreviousTheme), "↑theme"),
                (
                    Hit::Action(Action::TogglePreview),
                    if self.theme_table_preview {
                        "list"
                    } else {
                        "table"
                    },
                ),
                (Hit::Action(Action::Save), "save"),
                (Hit::Action(Action::Cancel), "cancel"),
            ],
        };
        let mut lines: Vec<Vec<(String, Hit)>> = Vec::new();
        for (hit, meaning) in actions {
            let unit = format!(
                "{}: {meaning}",
                match hit {
                    Hit::Action(action) => self.keymap.footer_labels(context, action.name()),
                    Hit::Key(Key::Enter) => "Enter".to_owned(),
                    _ => unreachable!(),
                }
            );
            if let Some(line) = lines.last_mut().filter(|line| {
                line.iter().map(|(text, _)| columns(text)).sum::<usize>()
                    + line.len() * columns(" · ")
                    + columns(&unit)
                    <= width.saturating_sub(4)
            }) {
                line.push((unit, hit.clone()));
            } else {
                lines.push(vec![(unit, hit.clone())]);
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
            Context::Select | Context::Agent => table_minimum,
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

    #[cfg(test)]
    pub(crate) fn hints(
        &self,
        context: Context,
        width: usize,
        budget: usize,
        ansi: bool,
    ) -> Vec<String> {
        self.hints_at(context, width, budget, ansi, 0)
    }

    pub(crate) fn hints_at(
        &self,
        context: Context,
        width: usize,
        budget: usize,
        ansi: bool,
        y: usize,
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
            .enumerate()
            .map(|(row, units)| {
                let mut line = String::from("  ");
                let mut x = 2;
                for (index, (unit, hit)) in units.into_iter().enumerate() {
                    if index > 0 {
                        line.push_str(" · ");
                        x += " · ".cell_width() as usize;
                    }
                    let width = unit.cell_width() as usize;
                    self.hit(x, y + row, width, hit);
                    line.push_str(&unit);
                    x += width;
                }
                line
            })
            .collect();
        if ansi {
            result.resize(budget, String::new());
        }
        result
    }

    pub(super) fn search_edit_hits(&self, y: usize, text: &str) {
        use crate::input::Key;
        let erase = text.find("Backspace").unwrap();
        let return_label = text[..erase].trim_end_matches([' ', '·']);
        self.hit(
            2,
            y,
            return_label.cell_width() as usize,
            Hit::Key(Key::Enter),
        );
        let x = 2 + text[..erase].cell_width() as usize;
        self.hit(
            x,
            y,
            "Backspace: erase".cell_width() as usize,
            Hit::Key(Key::Backspace),
        );
    }
}
