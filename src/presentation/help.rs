use ratatui::{
    buffer::CellWidth,
    style::{Color, Modifier},
};

use super::{clipped, columns, marked_color, sgr_bg, sgr_fg, wrapped};
use crate::keymap::Context;
use crate::search::match_positions;
use crate::selection::{Hit, Ui};
use crate::theme::Theme;

impl Ui {
    pub(super) fn help_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let footer_rows = if self.help.searching {
            1
        } else {
            self.hint_rows(Context::Help, width, height)
        };
        let theme = self.theme();
        let legacy = theme == Theme::named("catppuccin");
        let accent = if legacy {
            Color::Rgb(180, 190, 254)
        } else {
            theme.accent
        };
        let table_width = width.saturating_sub(4);
        let aligned = |text: &str| format!("  {}", clipped(text, table_width));
        let search =
            self.search_prompt(Context::Help, &self.help.query, self.help.searching, width);
        let rule = format!("  {}", "─".repeat(table_width));
        let visible = self.help.visible();
        let selected = self.help.selected_index(&visible);
        // Headings and spacers scroll, but never participate in selection or matching.
        let entries = &self.help.entries;
        if entries
            .iter()
            .any(|e| columns(&e.keys) + columns(e.meaning) + 4 > table_width)
        {
            return self.long_help_layout(width, height, ansi);
        }
        let mut contents = Vec::new();
        let mut category = None;
        for &i in &visible {
            let entry = &entries[i];
            if category != Some(entry.category) {
                if category.is_some() {
                    contents.push((None, ""));
                }
                contents.push((None, entry.category));
                category = Some(entry.category);
            }
            contents.push((Some(i), ""));
        }
        if contents.is_empty() {
            contents.push((None, "No matching keys"));
        }
        let available = if ansi {
            height.saturating_sub(4 + footer_rows)
        } else {
            contents.len()
        };
        let mut start = if ansi {
            self.help
                .viewport_start
                .get()
                .min(contents.len().saturating_sub(available))
        } else {
            0
        };
        if let Some(at) = contents
            .iter()
            .position(|(i, _)| i.is_some() && *i == selected)
        {
            if at < start {
                start = if at > 0 && contents[at - 1].0.is_none() && !contents[at - 1].1.is_empty()
                {
                    at - 1
                } else {
                    at
                };
            }
            if available > 0 && at >= start + available {
                start = at + 1 - available;
            }
        }
        if ansi {
            self.help.viewport_start.set(start);
        }
        let key_width = entries
            .iter()
            .map(|entry| usize::from(entry.keys.cell_width()))
            .max()
            .unwrap_or(0);
        let meaning_width = table_width.saturating_sub(key_width + 4);
        let render_field = |text: &str, limit: usize| {
            let line = marked_color(
                text,
                limit,
                &match_positions(text, &self.help.query),
                Some(accent),
            );
            line.spans
                .iter()
                .map(|span| {
                    if ansi && span.style.add_modifier.contains(Modifier::UNDERLINED) {
                        format!(
                            "{}\x1b[4m{}\x1b[24m{}",
                            sgr_fg(accent),
                            span.content,
                            sgr_fg(theme.text)
                        )
                    } else {
                        span.content.to_string()
                    }
                })
                .collect::<String>()
        };
        let mut lines = vec![
            format!("  {search}"),
            aligned(&format!("Keys — {}", self.help.context.name())),
            rule.clone(),
        ];
        self.hit(2, 0, self.search_prompt_geometry(width).1, Hit::Search);
        for &(i, heading) in contents.iter().skip(start).take(available) {
            if let Some(i) = i {
                self.hit(2, lines.len(), table_width, Hit::Help(i));
                let entry = &entries[i];
                let active = selected == Some(i);
                let text = format!(
                    "{} {}{}  {}",
                    if active { ">" } else { " " },
                    render_field(&entry.keys, key_width.max(columns(&entry.keys))),
                    " ".repeat(key_width.saturating_sub(usize::from(entry.keys.cell_width()))),
                    render_field(entry.meaning, meaning_width)
                );
                lines.push(if ansi && active {
                    let padding = table_width.saturating_sub(
                        4 + key_width + columns(&clipped(entry.meaning, meaning_width)),
                    );
                    format!(
                        "  {}{text}{}\x1b[0m",
                        sgr_bg(if legacy {
                            Color::Indexed(236)
                        } else {
                            theme.selection
                        }),
                        " ".repeat(padding)
                    )
                } else {
                    format!("  {text}")
                });
            } else {
                lines.push(aligned(heading));
            }
        }
        if ansi {
            lines.resize(height.saturating_sub(1 + footer_rows), String::new());
        }
        lines.push(rule);
        if self.help.searching {
            lines.push(aligned("Enter/Esc: return to keys · Backspace: erase"));
            self.search_edit_hits(
                lines.len() - 1,
                "Enter/Esc: return to keys · Backspace: erase",
            );
        } else {
            lines.extend(self.hints_at(Context::Help, width, footer_rows, ansi, lines.len()));
        }
        if ansi {
            lines.truncate(height);
        }
        lines
    }

    fn long_help_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let footer_rows = if self.help.searching {
            1
        } else {
            self.hint_rows(Context::Help, width, height)
        };
        let entries = &self.help.entries;
        let theme = self.theme();
        let legacy = theme == Theme::named("catppuccin");
        let accent = if legacy {
            Color::Rgb(180, 190, 254)
        } else {
            theme.accent
        };
        let visible = self.help.visible();
        let selected = self.help.selected_index(&visible);
        let mut contents = Vec::new();
        let mut content_hits = Vec::new();
        let mut selected_start = 0;
        let mut selected_len = 0;
        let mut category = None;
        for i in visible {
            let entry = &entries[i];
            if category != Some(entry.category) {
                contents.push(format!("  {}", entry.category));
                content_hits.push(None);
                category = Some(entry.category);
            }
            let text = format!("{}: {}", entry.keys, entry.meaning);
            let rows = wrapped(&text, width.saturating_sub(6));
            let mut positions = match_positions(&entry.keys, &self.help.query);
            positions.extend(
                match_positions(entry.meaning, &self.help.query)
                    .into_iter()
                    .map(|at| at + entry.keys.chars().count() + 2),
            );
            if selected == Some(i) {
                selected_start = contents.len();
                selected_len = rows.len();
            }
            let mut byte_offset = 0;
            for (at, row) in rows.into_iter().enumerate() {
                content_hits.push(Some(i));
                let start = byte_offset + text[byte_offset..].find(&row).unwrap();
                let char_offset = text[..start].chars().count();
                byte_offset = start + row.len();
                let local: Vec<_> = positions
                    .iter()
                    .filter_map(|at| {
                        at.checked_sub(char_offset)
                            .filter(|at| *at < row.chars().count())
                    })
                    .collect();
                let marked = marked_color(&row, width.saturating_sub(6), &local, Some(accent));
                let rendered = marked
                    .spans
                    .iter()
                    .map(|span| {
                        if ansi && span.style.add_modifier.contains(Modifier::UNDERLINED) {
                            format!(
                                "{}\x1b[4m{}\x1b[24m{}",
                                sgr_fg(accent),
                                span.content,
                                sgr_fg(theme.text)
                            )
                        } else {
                            span.content.to_string()
                        }
                    })
                    .collect::<String>();
                let marker = if selected == Some(i) && at == 0 {
                    ">"
                } else {
                    " "
                };
                let line = format!("{marker} {rendered}");
                contents.push(if ansi && selected == Some(i) {
                    format!(
                        "  {}{line}{}\x1b[0m",
                        sgr_bg(if legacy {
                            Color::Indexed(236)
                        } else {
                            theme.selection
                        }),
                        " ".repeat(width.saturating_sub(6 + columns(&row)))
                    )
                } else {
                    format!("  {line}")
                });
            }
        }
        if contents.is_empty() {
            contents.push("  No matching keys".into());
            content_hits.push(None);
        }
        let budget = height.saturating_sub(4 + footer_rows).max(1);
        let start = if ansi {
            if selected_len > budget {
                self.hint_more.set(true);
                selected_start + self.hint_scroll.get() % selected_len
            } else {
                selected_start
                    .saturating_sub(budget.saturating_sub(selected_len))
                    .min(contents.len().saturating_sub(budget))
            }
        } else {
            0
        };
        self.help.viewport_start.set(start);
        let rule = format!("  {}", "─".repeat(width.saturating_sub(4)));
        let mut lines = vec![
            format!(
                "  {}",
                self.search_prompt(Context::Help, &self.help.query, self.help.searching, width)
            ),
            format!("  Keys — {}", self.help.context.name()),
            rule.clone(),
        ];
        self.hit(2, 0, self.search_prompt_geometry(width).1, Hit::Search);
        for (row, index) in content_hits
            .into_iter()
            .skip(start)
            .take(if ansi { budget } else { usize::MAX })
            .enumerate()
        {
            if let Some(index) = index {
                self.hit(2, 3 + row, width.saturating_sub(4), Hit::Help(index));
            }
        }
        lines.extend(
            contents
                .into_iter()
                .skip(start)
                .take(if ansi { budget } else { usize::MAX }),
        );
        if ansi {
            lines.resize(height.saturating_sub(1 + footer_rows), String::new());
        }
        lines.push(rule);
        if self.help.searching {
            lines.push("  Enter/Esc: return to keys · Backspace: erase".into());
            self.search_edit_hits(
                lines.len() - 1,
                "Enter/Esc: return to keys · Backspace: erase",
            );
        } else {
            lines.extend(self.hints_at(Context::Help, width, footer_rows, ansi, lines.len()));
        }
        if ansi {
            lines.truncate(height);
        }
        lines
    }
}
