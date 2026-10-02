use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row as TableRow, StatefulWidget, Table, TableState},
};

use super::keymap::Context;
use super::selection::{match_positions, matches_field, Mode, Ui};
use super::theme::Theme;

pub(super) fn wrapped(text: &str, width: usize) -> Vec<String> {
    let width = width.max(2);
    let mut lines = vec![String::new()];
    for c in clean(text).chars() {
        if columns(lines.last().unwrap()) + columns(&c.to_string()) > width {
            let last = lines.last_mut().unwrap();
            let tail = last
                .rfind([' ', '/'])
                .filter(|at| *at > 0)
                .map(|at| {
                    let tail = last[at + 1..].to_owned();
                    last.truncate(at + usize::from(last.as_bytes()[at] == b'/'));
                    tail
                })
                .unwrap_or_default();
            lines.push(tail);
        }
        lines.last_mut().unwrap().push(c);
    }
    lines
}

pub(super) fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

// Without a terminal-width crate, count non-ASCII characters conservatively as two columns.
pub(super) fn columns(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

pub(super) fn clipped(s: &str, limit: usize) -> String {
    if columns(s) <= limit {
        return s.to_owned();
    }
    if limit < 2 {
        return ".".repeat(limit);
    }
    let mut result = String::new();
    let source = Span::raw(s);
    for grapheme in source.styled_graphemes(Style::default()) {
        if columns(&result) + columns(grapheme.symbol) > limit - 2 {
            break;
        }
        result.push_str(grapheme.symbol);
    }
    result.push('…');
    result
}

fn clipped_source(text: &str, limit: usize) -> String {
    if usize::from(text.cell_width()) <= limit {
        return text.to_owned();
    }
    if limit == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    let source = Span::raw(text);
    for grapheme in source.styled_graphemes(Style::default()) {
        let cells = usize::from(grapheme.symbol.cell_width());
        if used + cells > limit - 1 {
            break;
        }
        result.push_str(grapheme.symbol);
        used += cells;
    }
    result.push('…');
    result
}

pub(super) struct StatusColors {
    pub(super) working: Color,
    blocked: Color,
    done: Color,
    pub(super) idle: Color,
    pub(super) unknown: Color,
    overrides: [bool; 5],
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            working: Color::Rgb(249, 226, 175),
            blocked: Color::Rgb(243, 139, 168),
            done: Color::Rgb(148, 226, 213),
            idle: Color::Rgb(166, 227, 161),
            unknown: Color::Rgb(108, 112, 134),
            overrides: [false; 5],
        }
    }
}

impl StatusColors {
    pub(super) fn for_status(&self, status: &str) -> Color {
        match status {
            "working" => self.working,
            "blocked" => self.blocked,
            "done" => self.done,
            "idle" => self.idle,
            _ => self.unknown,
        }
    }

    pub(super) fn themed(&self, status: &str, theme: Theme) -> Color {
        let (index, color) = match status {
            "working" => (0, theme.working),
            "blocked" => (1, theme.blocked),
            "done" => (2, theme.done),
            "idle" => (3, theme.idle),
            _ => (4, theme.unknown),
        };
        if self.overrides[index] {
            self.for_status(status)
        } else {
            color
        }
    }

    #[cfg(test)]
    pub(super) fn load(dir: Option<&Path>) -> io::Result<Self> {
        let (colors, diagnostics) = Self::load_diagnostics(dir);
        if diagnostics.is_empty() {
            Ok(colors)
        } else {
            Err(io::Error::other(diagnostics.messages().join("; ")))
        }
    }

    pub(super) fn load_diagnostics(dir: Option<&Path>) -> (Self, super::config::Diagnostics) {
        let mut colors = Self::default();
        let mut diagnostics = super::config::Diagnostics::default();
        let content = match super::config::read(dir, "status-colors") {
            Ok(Some((path, content))) => {
                diagnostics.source = Some(path);
                content
            }
            Ok(None) => return (colors, diagnostics),
            Err(error) => {
                diagnostics.push("Status colors IO", error.to_string());
                diagnostics.source = Some(error.path);
                return (colors, diagnostics);
            }
        };
        let parsed = match super::config::parse(&content) {
            Ok(parsed) => parsed,
            Err(error) => {
                diagnostics.push("Status colors syntax", error);
                return (colors, diagnostics);
            }
        };
        for duplicate in parsed.duplicate_fields {
            let location = super::config::location(&duplicate.location);
            diagnostics.push(
                "Status colors duplicate fields",
                if location.is_empty() {
                    format!("duplicate field {}", duplicate.field)
                } else {
                    format!("{location}: duplicate field {}", duplicate.field)
                },
            );
        }
        let Some(entries) = parsed.ast.as_object() else {
            diagnostics.push("Status colors structure", "expected JSON object");
            return (colors, diagnostics);
        };
        for entry in &entries.properties {
            let key = entry.name.as_str();
            if !["working", "blocked", "done", "idle", "unknown"].contains(&key) {
                diagnostics.push(
                    "Status colors unknown keys",
                    format!("unknown status key {key}"),
                );
            }
            let hex = entry
                .value
                .as_string_lit()
                .map(|literal| literal.value.as_ref())
                .filter(|hex| {
                    hex.len() == 7
                        && hex.starts_with('#')
                        && hex.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                });
            let Some(hex) = hex else {
                diagnostics.push(
                    "Status colors invalid values",
                    format!("{key} must be #RRGGBB"),
                );
                continue;
            };
            let target = match key {
                "working" => &mut colors.working,
                "blocked" => &mut colors.blocked,
                "done" => &mut colors.done,
                "idle" => &mut colors.idle,
                "unknown" => &mut colors.unknown,
                _ => continue,
            };
            *target = Color::Rgb(
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
            );
            colors.overrides[match key {
                "working" => 0,
                "blocked" => 1,
                "done" => 2,
                "idle" => 3,
                _ => 4,
            }] = true;
        }
        (colors, diagnostics)
    }
}

#[cfg(test)]
pub(super) fn marked(text: &str, limit: usize, positions: &[usize], accent: bool) -> Line<'static> {
    marked_color(
        text,
        limit,
        positions,
        accent.then_some(Color::Rgb(180, 190, 254)),
    )
}

pub(super) fn marked_color(
    text: &str,
    limit: usize,
    positions: &[usize],
    accent: Option<Color>,
) -> Line<'static> {
    let clipped = clipped(text, limit);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut index = 0;
    let graphemes = Span::raw(clipped.clone());
    let last = graphemes
        .styled_graphemes(Style::default())
        .count()
        .saturating_sub(1);
    for (at, grapheme) in graphemes.styled_graphemes(Style::default()).enumerate() {
        let count = grapheme.symbol.chars().count();
        let matched = !(clipped != text && at == last && grapheme.symbol == "…")
            && (index..index + count).any(|position| positions.contains(&position));
        let style = if matched {
            let style = Style::default().add_modifier(Modifier::UNDERLINED);
            if let Some(color) = accent {
                style.fg(color)
            } else {
                style
            }
        } else {
            Style::default()
        };
        if let Some(span) = spans.last_mut().filter(|span| span.style == style) {
            span.content.to_mut().push_str(grapheme.symbol);
        } else {
            spans.push(Span::styled(grapheme.symbol.to_owned(), style));
        }
        index += count;
    }
    Line::from(spans)
}

impl Ui {
    fn source_line(&self, source: &Path, width: usize, ansi: bool) -> String {
        let available = width.saturating_sub(1);
        let filename = clean(&source.file_name().unwrap().to_string_lossy());
        let filename_width = usize::from(filename.cell_width());
        if available < filename_width + 3 {
            return clipped_source(&filename, available);
        }
        let full_path = clean(&source.to_string_lossy());
        let path = clipped_source(&full_path, available - filename_width - 2);
        // Clipping preserves graphemes: a wide first grapheme may leave only
        // the ellipsis, even when an ASCII path prefix would fit.
        if path != full_path && usize::from(path.cell_width()) <= 1 {
            return filename;
        }
        if ansi {
            let theme = self.theme();
            format!(
                "{filename}  {}\x1b[2m{path}\x1b[22m{}",
                sgr_fg(theme.dim),
                sgr_fg(theme.text)
            )
        } else {
            format!("{filename}  {path}")
        }
    }

    fn search_prompt(
        &self,
        context: Context,
        query: &str,
        searching: bool,
        width: usize,
    ) -> String {
        if !query.is_empty() || searching {
            return format!("Search: {query}");
        }
        let text = format!(
            "Search: {} to search",
            self.keymap.display_labels(context, "open_search")
        );
        let pages = wrapped(&text, width.saturating_sub(4));
        if pages.len() > 1 {
            self.hint_more.set(true);
        }
        pages[self.hint_scroll.get() % pages.len()].clone()
    }
    fn error_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let error_line = |text: &str| {
            let text = clipped(text, width.saturating_sub(1));
            if ansi {
                let theme = self.theme();
                format!(
                    "\x1b[1m{}{text}\x1b[22m{}",
                    sgr_fg(theme.blocked),
                    sgr_fg(theme.text)
                )
            } else {
                text
            }
        };
        let detail_width = width.saturating_sub(if ansi && height <= 2 { 8 } else { 3 });
        let mut blocks = Vec::new();
        if ansi && (width < 80 || height < 15) {
            blocks.push((
                "Popup too small".into(),
                None,
                vec![
                    format!("need 80x15; actual {width}x{height}."),
                    "Enlarge popup and reopen.".into(),
                ],
            ));
        }
        blocks.extend(
            self.keymap
                .diagnostic_blocks()
                .into_iter()
                .map(|(heading, messages)| {
                    (heading, self.keymap.diagnostics.source.as_deref(), messages)
                }),
        );
        for diagnostics in [&self.status_errors, &self.theme_diagnostics] {
            blocks.extend(
                diagnostics
                    .blocks
                    .iter()
                    .cloned()
                    .map(|(heading, messages)| (heading, diagnostics.source.as_deref(), messages)),
            );
        }
        let mut details = Vec::new();
        let mut contents = Vec::new();
        for (at, (heading, source, diagnostics)) in blocks.iter().enumerate() {
            if at > 0 {
                let rule = "─".repeat(detail_width / 2);
                contents.push(rule);
            }
            contents.push(error_line(&format!("ERROR: {heading}")));
            if let Some(source) = source {
                contents.push(self.source_line(source, width, ansi));
            }
            for error in diagnostics {
                let rows = wrapped(error, detail_width);
                details.extend(rows.clone());
                contents.extend(rows);
            }
        }
        if ansi && height <= 2 {
            // At extreme heights diagnostics take precedence over title and legend.
            let start = self
                .error_scroll
                .get()
                .min(details.len().saturating_sub(height));
            self.error_scroll.set(start);
            return details
                .into_iter()
                .skip(start)
                .take(height)
                .map(|line| error_line(&format!("ERROR: {line}")))
                .collect();
        }
        let budget = if ansi { height - 2 } else { contents.len() };
        let start = if ansi {
            let start = self
                .error_scroll
                .get()
                .min(contents.len().saturating_sub(budget));
            self.error_scroll.set(start);
            start
        } else {
            0
        };
        let mut lines: Vec<_> = contents.into_iter().skip(start).take(budget).collect();
        if ansi {
            lines.resize(budget, String::new());
        }
        lines.push("─".repeat(width));
        lines.push(clipped(
            "↑/k: scroll up · ↓/j: scroll down · Enter/Esc/q: close",
            width.saturating_sub(1),
        ));
        lines
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

    pub(super) fn hint_rows(&self, context: Context, width: usize, height: usize) -> usize {
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

    pub(super) fn hints(
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
    fn notice_line(&self, width: usize, ansi: bool) -> Option<String> {
        let message = self.theme_notice()?;
        let error = self.theme_error.is_some();
        let text = clipped(
            &clean(&format!(
                "{} · {message}",
                if error { "ERROR" } else { "INFO" }
            )),
            width.saturating_sub(3),
        );
        if !ansi {
            return Some(format!("  {text}"));
        }
        let theme = self.theme();
        Some(format!(
            "  {}{}{text}{}{}",
            if error { "\x1b[1m" } else { "" },
            sgr_fg(if error { theme.blocked } else { theme.dim }),
            if error { "\x1b[22m" } else { "" },
            sgr_fg(theme.text)
        ))
    }

    pub(super) fn frame(&self) -> String {
        self.frame_width(self.width)
    }

    pub(super) fn frame_width(&self, width: usize) -> String {
        self.layout(width, usize::MAX, false).join("\n") + "\n"
    }

    fn too_small_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let messages = if height <= 2 {
            vec![
                format!("ERROR: {width}x{height}; need 80x15."),
                "Enlarge popup and reopen.".to_owned(),
            ]
        } else {
            vec![
                "ERROR: Crossroads needs minimum content size 80x15.".to_owned(),
                format!("  Your content size is: {width}x{height}."),
                "  Set larger size in herdr plugin manifest and reopen.".to_owned(),
                "  Press Enter/Esc/q to close.".to_owned(),
            ]
        };
        messages
            .into_iter()
            .take(height)
            .enumerate()
            .map(|(at, message)| {
                let text = clipped(&message, width.saturating_sub(1));
                if ansi && at == 0 {
                    let theme = self.theme();
                    format!(
                        "\x1b[1m{}{text}\x1b[22m{}",
                        sgr_fg(theme.blocked),
                        sgr_fg(theme.text)
                    )
                } else {
                    text
                }
            })
            .collect()
    }

    pub(super) fn terminal_frame(&self, width: usize, height: usize) -> String {
        let mut lines = self.layout(width, height, true);
        let theme = self.theme();
        if theme.background == Color::Reset {
            return lines.join("\r\n");
        }
        lines.resize(height, String::new());
        lines
            .into_iter()
            .map(|line| {
                // Layout emits only SGR styling. Count visible Unicode cells,
                // not escape bytes: EL at wrap-pending erases the final cell.
                let mut text = String::new();
                let mut sgr = false;
                for c in line.chars() {
                    if c == '\x1b' {
                        sgr = true;
                    } else if sgr {
                        sgr = c != 'm';
                    } else {
                        text.push(c);
                    }
                }
                let clear = if usize::from(text.cell_width()) < width {
                    "\x1b[K"
                } else {
                    ""
                };
                format!(
                    "{}{}{line}{}{clear}",
                    sgr_bg(theme.background),
                    sgr_fg(theme.text),
                    sgr_bg(theme.background)
                )
            })
            .collect::<Vec<_>>()
            .join("\r\n")
    }

    pub(super) fn layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        self.hint_more.set(false);
        if self.configuration_failed() {
            return self.error_layout(width, height, ansi);
        }
        if let Some((_, popup_height)) = self.popup_size {
            let height = if ansi { height } else { popup_height };
            if width < 80 || height < 15 {
                return self.too_small_layout(width, height, ansi);
            }
        }
        if self.mode == Mode::Help {
            return self.help_layout(width, height, ansi);
        }
        if self.mode == Mode::Themes && !self.theme_table_preview {
            return self.theme_layout(width, height, ansi);
        }
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
                super::theme::NAMES[self.theme_index],
                super::theme::NAMES[self.theme_cursor]
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

    fn help_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
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
            height.saturating_sub(3 + footer_rows)
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
        let mut lines = vec![aligned(&search), rule.clone()];
        for &(i, heading) in contents.iter().skip(start).take(available) {
            if let Some(i) = i {
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
        } else {
            lines.extend(self.hints(Context::Help, width, footer_rows, ansi));
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
        let mut selected_start = 0;
        let mut selected_len = 0;
        let mut category = None;
        for i in visible {
            let entry = &entries[i];
            if category != Some(entry.category) {
                contents.push(format!("  {}", entry.category));
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
        }
        let budget = height.saturating_sub(3 + footer_rows).max(1);
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
            rule.clone(),
        ];
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
        } else {
            lines.extend(self.hints(Context::Help, width, footer_rows, ansi));
        }
        if ansi {
            lines.truncate(height);
        }
        lines
    }

    fn theme_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let footer_rows = self.hint_rows(Context::Themes, width, height);
        let limit = width.saturating_sub(1);
        let aligned = |text: &str| format!("  {}", clipped(text, limit.saturating_sub(2)));
        let names = &super::theme::NAMES;
        let mut lines = vec![aligned(&format!(
            "Themes · selected: {} · preview: {}",
            names[self.theme_index], names[self.theme_cursor]
        ))];
        let available = if ansi {
            height
                .saturating_sub(1 + footer_rows + usize::from(self.theme_notice().is_some()))
                .max(1)
        } else {
            names.len()
        };
        let start = self
            .theme_cursor
            .saturating_sub(available / 2)
            .min(names.len().saturating_sub(available));
        for (index, name) in names.iter().enumerate().skip(start).take(available) {
            let line = aligned(&format!(
                "{} {}{}",
                if index == self.theme_cursor { ">" } else { " " },
                name,
                if index == self.theme_index {
                    " (saved)"
                } else {
                    ""
                }
            ));
            lines.push(if ansi && index == self.theme_cursor {
                format!("{}{}\x1b[0m", sgr_bg(self.theme().selection), line)
            } else {
                line
            });
        }
        if ansi {
            lines.truncate(
                height.saturating_sub(footer_rows + usize::from(self.theme_notice().is_some())),
            );
            lines.resize(
                height.saturating_sub(footer_rows + usize::from(self.theme_notice().is_some())),
                String::new(),
            );
        }
        if let Some(notice) = self.notice_line(width, ansi) {
            lines.push(notice);
        }
        lines.extend(self.hints(Context::Themes, width, footer_rows, ansi));
        lines
    }
}

fn sgr_fg(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
        Color::Blue => "\x1b[34m".into(),
        Color::Gray => "\x1b[37m".into(),
        Color::Green => "\x1b[32m".into(),
        Color::Yellow => "\x1b[33m".into(),
        Color::LightRed => "\x1b[91m".into(),
        Color::Cyan => "\x1b[36m".into(),
        _ => "\x1b[39m".into(),
    }
}

fn sgr_bg(color: Color) -> String {
    match color {
        Color::Indexed(index) => format!("\x1b[48;5;{index}m"),
        Color::Rgb(r, g, b) => format!("\x1b[48;2;{r};{g};{b}m"),
        _ => "\x1b[49;7m".into(),
    }
}

pub(super) fn terminal_size() -> (usize, usize) {
    let measured = Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let mut fields = s.split_whitespace();
            Some((
                fields.next()?.parse::<usize>().ok()?,
                fields.next()?.parse::<usize>().ok()?,
            ))
        });
    let width = measured
        .map(|(_, width)| width)
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok()))
        .filter(|n| *n > 0)
        .unwrap_or(80);
    (
        measured
            .map(|(height, _)| height)
            .filter(|n| *n > 0)
            .unwrap_or(24),
        width,
    )
}

pub(super) struct Terminal(pub(super) Option<String>, bool);
impl Terminal {
    pub(super) fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() {
            return Ok(Self(None, false));
        }
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()?;
        if !saved.status.success() {
            return Err(io::Error::other("stty -g failed"));
        }
        let state = String::from_utf8(saved.stdout).map_err(io::Error::other)?;
        let status = Command::new("stty")
            .args(["raw", "-echo", "min", "0", "time", "1"])
            .stdin(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(io::Error::other("stty raw failed"));
        }
        Ok(Self(Some(state.trim().to_owned()), false))
    }
    pub(super) fn query(&self) -> io::Result<()> {
        if self.0.is_some() {
            let mut out = io::stdout().lock();
            out.write_all(b"\x1b[?u")?;
            out.flush()?;
        }
        Ok(())
    }
    pub(super) fn enable_kitty(&mut self) -> io::Result<()> {
        // Mark ownership before writing, so even a partial write gets a best-effort pop.
        self.1 = true;
        let mut out = io::stdout().lock();
        out.write_all(b"\x1b[>1u")?;
        out.flush()
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(state) = &self.0 {
            let mut out = io::stdout().lock();
            if self.1 {
                let _ = out.write_all(b"\x1b[<u");
            }
            let _ = out.write_all(b"\x1b[?25h");
            let _ = out.flush();
            let _ = Command::new("stty")
                .arg(state)
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

pub(super) fn draw(ui: &Ui, terminal: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if terminal {
        let (width, height) = ui.popup_size.expect("interactive terminal size");
        let frame = ui.terminal_frame(width, height);
        if ui.theme().background != Color::Reset {
            write!(out, "\x1b[H\x1b[J{frame}")?;
            write!(out, "\x1b[0m")?;
        } else {
            write!(out, "\x1b[H\x1b[J")?;
            for (index, line) in frame.split("\r\n").enumerate() {
                if index > 0 {
                    write!(out, "\r\n")?;
                }
                write!(out, "{}{line}", sgr_fg(ui.theme().text))?;
            }
            write!(out, "\x1b[0m")?;
        }
        if (ui.mode == Mode::Search || (ui.mode == Mode::Help && ui.help.searching))
            && width >= 80
            && height >= 15
        {
            // Measure the unstyled line: themed frame prefixes contain ANSI bytes.
            let x = ui.layout(width, height, true)[0].cell_width() as usize;
            write!(
                out,
                "\x1b[1;{}H\x1b[?25h",
                x.min(width.saturating_sub(1)) + 1
            )?;
        } else {
            out.write_all(b"\x1b[?25l")?;
        }
    } else {
        out.write_all(ui.frame().as_bytes())?;
    }
    out.flush()
}
