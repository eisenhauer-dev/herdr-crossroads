use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row as TableRow, StatefulWidget, Table, TableState},
};

use super::{clipped, columns, marked_color, sgr_bg, sgr_fg};
use crate::keymap::Context;
use crate::selection::{match_positions, matches_field, Mode, Ui};
use crate::theme::Theme;

impl Ui {
    pub(super) fn table_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let preview = self.mode == Mode::Themes;
        let hint_rows = if self.mode == Mode::Search {
            1
        } else {
            self.hint_rows(
                if preview {
                    Context::Themes
                } else {
                    Context::Select
                },
                width,
                height,
            )
        };
        let theme = self.theme();
        let legacy = theme == Theme::named("catppuccin");
        let accent = if legacy {
            Color::Rgb(180, 190, 254)
        } else {
            theme.accent
        };
        let limit = width.saturating_sub(1);
        let table_text_limit = width.saturating_sub(4);
        let aligned = |text: &str| format!("  {}", clipped(text, table_text_limit));
        let visible = self.visible();
        let selected = self.selected_index(&visible);
        let mut lines = Vec::new();
        let search = self.search_prompt(
            Context::Select,
            &self.query,
            self.mode != Mode::Select,
            width,
        );
        if preview {
            lines.push(aligned(&format!(
                "Themes · saved: {} · preview: {}",
                crate::theme::NAMES[self.theme_index],
                crate::theme::NAMES[self.theme_cursor]
            )));
        }
        lines.push(aligned(&search));
        {
            let table_limit = limit.saturating_sub(3);
            let inner = table_limit.saturating_sub(4);
            let roomy = inner >= 102;
            let table_margin = "  ";
            let short = ansi && height <= 6;
            let header_rule = roomy && !short && height >= 8;
            let footer_rows = hint_rows + usize::from(self.theme_notice().is_some());
            let available = height.saturating_sub(
                1 + usize::from(preview)
                    + footer_rows
                    + if short { 1 } else { 3 }
                    + usize::from(header_rule),
            );
            let selected_at = visible.iter().position(|i| Some(*i) == selected);
            let span = |start: usize, end: usize| {
                (start..=end)
                    .map(|at| {
                        1 + usize::from(
                            at > start
                                && self.rows[visible[at - 1]].workspace_id
                                    != self.rows[visible[at]].workspace_id,
                        )
                    })
                    .sum::<usize>()
            };
            let mut start = if ansi {
                self.viewport_start
                    .get()
                    .min(visible.len().saturating_sub(1))
            } else {
                0
            };
            if let Some(at) = selected_at {
                if ansi
                    && !self
                        .last_selected_at
                        .get()
                        .is_some_and(|previous| previous.abs_diff(at) <= 1)
                {
                    start = at;
                    while start > 0 && span(start - 1, at) <= available {
                        start -= 1;
                    }
                }
                if at < start {
                    start = at;
                }
                if available > 0 {
                    while start < at && span(start, at) > available {
                        start += 1;
                    }
                }
            }
            if ansi {
                self.viewport_start.set(start);
                self.last_selected_at.set(selected_at);
            }
            let mut shown = Vec::new();
            for at in start..visible.len() {
                if ansi && (available == 0 || span(start, at) > available) {
                    break;
                }
                shown.push(visible[at]);
            }
            let full = inner >= 30 && (!visible.is_empty() || width >= 80);
            let context = inner >= 18 && (!visible.is_empty() || width >= 80);
            let expanded = if (72..102).contains(&inner) {
                let mut widths = [26, 16, 28];
                // Assign each additional cell to the column furthest behind its target share.
                for step in 1..=inner - 72 {
                    let weights = [21, 19, 17];
                    let next = (0..3)
                        .max_by_key(|&i| {
                            (
                                step as isize * weights[i]
                                    - (widths[i] - [26, 16, 28][i]) as isize * 57,
                                -(i as isize),
                            )
                        })
                        .unwrap();
                    widths[next] += 1;
                }
                Some(widths)
            } else {
                None
            };
            let workspace_width = if roomy {
                inner * 60 / 141
            } else if let Some(widths) = expanded {
                widths[0]
            } else if inner >= 30 {
                if inner >= 52 {
                    (inner * 27 / 52).clamp(5, if inner == 52 { 27 } else { 26 })
                } else {
                    (inner / 3).clamp(5, 24)
                }
            } else {
                (inner / 5).clamp(5, 22)
            };
            let tab_width = if roomy {
                inner * 38 / 141
            } else if let Some(widths) = expanded {
                widths[1]
            } else {
                if inner >= 52 {
                    (inner * 12 / 52).clamp(5, 22)
                } else {
                    (inner / 5).clamp(5, 22)
                }
            };
            let status_width = (inner / 6).clamp(6, 12);
            let separate_status = roomy && full;
            let name_width = if separate_status {
                inner.saturating_sub(workspace_width + tab_width + status_width + 3)
            } else if let Some(widths) = expanded {
                widths[2]
            } else if context {
                inner.saturating_sub(workspace_width + tab_width + 2)
            } else {
                inner
            };
            let mut table_rows = Vec::new();
            let mut separator_rows = Vec::new();
            if header_rule {
                separator_rows.push((2, false));
                table_rows.push(TableRow::new(vec![""; 4]));
            }
            for (position, &i) in shown.iter().enumerate() {
                let row = &self.rows[i];
                let previous = position.checked_sub(1).map(|at| &self.rows[shown[at]]);
                if previous.is_some_and(|prev| prev.workspace_id != row.workspace_id) {
                    separator_rows.push((
                        table_rows.len() + if short { 1 } else { 2 },
                        row.linked_worktree,
                    ));
                    table_rows.push(TableRow::new(vec![
                        "";
                        if separate_status {
                            4
                        } else if context {
                            3
                        } else {
                            1
                        }
                    ]));
                }
                let workspace =
                    if previous.is_some_and(|prev| prev.workspace_id == row.workspace_id) {
                        ""
                    } else {
                        &row.workspace
                    };
                let tab = if previous.is_some_and(|prev| prev.tab_id == row.tab_id) {
                    ""
                } else {
                    &row.tab
                };
                let name = if row.agent.is_empty() {
                    row.pane.clone()
                } else if !self.query.is_empty()
                    && matches_field(&row.pane, &self.query)
                    && row.pane != row.agent
                {
                    format!("{} ({})", row.agent, row.pane)
                } else {
                    row.agent.clone()
                };
                let mut cells = Vec::new();
                if context {
                    cells.push(marked_color(
                        workspace,
                        workspace_width,
                        &match_positions(workspace, &self.query),
                        Some(accent),
                    ));
                    cells.push(marked_color(
                        tab,
                        tab_width,
                        &match_positions(tab, &self.query),
                        Some(accent),
                    ));
                }
                let mut name_positions = if row.agent.is_empty() {
                    match_positions(&row.pane, &self.query)
                } else {
                    match_positions(&row.agent, &self.query)
                };
                if name != row.agent && !row.agent.is_empty() {
                    name_positions.extend(
                        match_positions(&row.pane, &self.query)
                            .into_iter()
                            .map(|index| row.agent.chars().count() + 2 + index),
                    );
                }
                let inline = full
                    && !separate_status
                    && !row.agent.is_empty()
                    && !row.status.is_empty()
                    && name_width >= columns(&row.status) + if inner >= 52 { 4 } else { 9 };
                if inline {
                    let prefix_width = name_width - columns(&row.status) - 3;
                    let prefix = if prefix_width == 1 {
                        name.chars()
                            .next()
                            .filter(char::is_ascii)
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| ".".to_owned())
                    } else {
                        name.clone()
                    };
                    let mut spans =
                        marked_color(&prefix, prefix_width, &name_positions, Some(accent)).spans;
                    spans.extend([
                        Span::raw(" "),
                        Span::styled("|", Style::default().add_modifier(Modifier::DIM)),
                        Span::raw(" "),
                        Span::styled(
                            row.status.clone(),
                            Style::default().fg(self.status_colors.themed(&row.status, theme)),
                        ),
                    ]);
                    cells.push(Line::from(spans));
                } else {
                    cells.push(marked_color(
                        &name,
                        name_width,
                        &name_positions,
                        Some(accent),
                    ));
                }
                if separate_status {
                    cells.push(Line::from(Span::styled(
                        clipped(&row.status, status_width),
                        Style::default().fg(self.status_colors.themed(&row.status, theme)),
                    )));
                }
                table_rows.push(TableRow::new(cells));
            }
            if shown.is_empty() {
                let mut cells = if context {
                    vec![Line::raw(""), Line::raw("")]
                } else {
                    vec![]
                };
                cells.push(Line::raw("No matching panes"));
                if separate_status {
                    cells.push(Line::raw(""));
                }
                table_rows.push(TableRow::new(cells));
            }
            let table_height = if ansi {
                height.saturating_sub(1 + usize::from(preview) + footer_rows)
            } else {
                table_rows.len() + 3
            };
            if table_limit >= 8 && table_height >= if short { 2 } else { 3 } {
                let area = Rect::new(
                    0,
                    0,
                    table_limit.min(u16::MAX as usize) as u16,
                    table_height.min(u16::MAX as usize) as u16,
                );
                let mut buffer = Buffer::empty(area);
                let mut widths = Vec::new();
                let mut headers = Vec::new();
                if context {
                    widths.extend([
                        Constraint::Length(workspace_width as u16),
                        Constraint::Length(tab_width as u16),
                    ]);
                    headers.extend(["Workspace", "Tab"]);
                }
                widths.push(Constraint::Length(name_width as u16));
                headers.push("Agent/Pane Name");
                if separate_status {
                    widths.push(Constraint::Length(status_width as u16));
                    headers.push("Status");
                }
                let mut table = Table::new(table_rows, widths)
                    .block(Block::default().borders(if short {
                        Borders::TOP
                    } else {
                        Borders::TOP | Borders::BOTTOM
                    }))
                    .column_spacing(1)
                    .row_highlight_style(Style::default().bg(if legacy {
                        Color::Indexed(236)
                    } else {
                        theme.selection
                    }))
                    .highlight_symbol("> ");
                if !short {
                    table = table.header(TableRow::new(headers));
                }
                let selected_row = shown.iter().position(|&i| Some(i) == selected).map(|at| {
                    at + usize::from(header_rule)
                        + shown[..=at]
                            .windows(2)
                            .filter(|pair| {
                                self.rows[pair[0]].workspace_id != self.rows[pair[1]].workspace_id
                            })
                            .count()
                });
                let mut state = TableState::default().with_selected(selected_row);
                StatefulWidget::render(table, area, &mut buffer, &mut state);
                for y in 0..area.height {
                    let selected_line = ansi
                        && selected_row
                            .is_some_and(|at| y as usize == at + if short { 1 } else { 2 });
                    let mut content = String::new();
                    let mut x = 0;
                    let mut foreground = None;
                    let mut dim = false;
                    let mut underlined = false;
                    let last = (0..area.width).rfind(|&x| buffer[(x, y)].symbol() != " ");
                    while x < area.width {
                        let cell = &buffer[(x, y)];
                        if last.is_none_or(|last| x > last) {
                            break;
                        }
                        if ansi && cell.fg != foreground.unwrap_or(Color::Reset) {
                            content.push_str(&sgr_fg(if cell.fg == Color::Reset {
                                theme.text
                            } else {
                                cell.fg
                            }));
                            foreground = (cell.fg != Color::Reset).then_some(cell.fg);
                        }
                        if ansi && cell.modifier.contains(Modifier::DIM) != dim {
                            dim = !dim;
                            content.push_str(if dim { "\x1b[2m" } else { "\x1b[22m" });
                        }
                        if ansi && cell.modifier.contains(Modifier::UNDERLINED) != underlined {
                            underlined = !underlined;
                            content.push_str(if underlined { "\x1b[4m" } else { "\x1b[24m" });
                        }
                        content.push_str(cell.symbol());
                        x += cell.cell_width().max(1);
                    }
                    if ansi && foreground.is_some() {
                        content.push_str(&sgr_fg(theme.text));
                    }
                    if ansi && dim {
                        content.push_str("\x1b[22m");
                    }
                    if ansi && underlined {
                        content.push_str("\x1b[24m");
                    }
                    if selected_line {
                        content.push_str(&" ".repeat(table_limit.saturating_sub(x as usize)));
                    }
                    let inset = separator_rows
                        .iter()
                        .find_map(|(at, inset)| (*at == y as usize).then_some(*inset));
                    let content = if let Some(inset) = inset {
                        let rule =
                            "┄".repeat(table_limit.saturating_sub(if inset { 4 } else { 0 }));
                        let margin = if inset { "      " } else { table_margin };
                        if ansi {
                            format!(
                                "{margin}{}\x1b[2m{rule}\x1b[22m",
                                if legacy {
                                    String::new()
                                } else {
                                    sgr_fg(theme.dim)
                                }
                            )
                        } else {
                            format!("{margin}{rule}")
                        }
                    } else {
                        format!("{table_margin}{content}")
                    };
                    if selected_line {
                        lines.push(format!(
                            "{table_margin}{}{}\x1b[0m",
                            if legacy {
                                "\x1b[48;5;236m".to_owned()
                            } else {
                                sgr_bg(theme.selection)
                            },
                            &content[table_margin.len()..]
                        ));
                    } else {
                        lines.push(content);
                    }
                }
            }
        }
        if preview {
            let footer_rows = hint_rows + usize::from(self.theme_notice().is_some());
            if ansi {
                lines.truncate(height.saturating_sub(footer_rows));
                lines.resize(height.saturating_sub(footer_rows), String::new());
            }
            if let Some(notice) = self.notice_line(width, ansi) {
                lines.push(notice);
            }
            lines.extend(self.hints(Context::Themes, width, hint_rows, ansi));
            return lines;
        }
        let footer = "Enter/Esc: select  Backspace: erase";
        if ansi {
            let body_height =
                height.saturating_sub(hint_rows + usize::from(self.theme_notice().is_some()));
            lines.truncate(body_height);
            lines.resize(body_height, String::new());
        }
        if !ansi || height > 0 {
            if self.mode == Mode::Select {
                if let Some(notice) = self.notice_line(width, ansi) {
                    lines.push(notice);
                }
                lines.extend(self.hints(Context::Select, width, hint_rows, ansi));
            } else {
                if let Some(notice) = self.notice_line(width, ansi) {
                    lines.push(notice);
                }
                lines.push(aligned(footer));
            }
        }
        lines
    }
}
