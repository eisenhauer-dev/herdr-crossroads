use ratatui::{
    buffer::CellWidth,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::selection::{Mode, Ui};

mod diagnostics;
mod footer;
mod help;
mod table;
mod themes;

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
        self.begin_mouse_layout();
        let lines = self.layout_contents(width, height, ansi);
        self.finish_mouse_layout();
        lines
    }

    fn layout_contents(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        self.hint_more.set(false);
        if self.configuration_failed() {
            let lines = self.error_layout(width, height, ansi);
            self.fixed_mouse_controls(&lines);
            return lines;
        }
        if let Some((_, popup_height)) = self.popup_size {
            let height = if ansi { height } else { popup_height };
            if width < 80 || height < 15 {
                let lines = self.too_small_layout(width, height, ansi);
                self.fixed_mouse_controls(&lines);
                return lines;
            }
        }
        if self.mode == Mode::Help {
            return self.help_layout(width, height, ansi);
        }
        if self.mode == Mode::Themes && !self.theme_table_preview {
            return self.theme_layout(width, height, ansi);
        }
        self.table_layout(width, height, ansi)
    }

    fn fixed_mouse_controls(&self, lines: &[String]) {
        use crate::{input::Key, selection::Hit};
        for (y, line) in lines.iter().enumerate() {
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
            for (label, key) in [
                ("Enter/Esc/q: close", Key::Enter),
                ("↑/k: scroll up", Key::Char('k')),
                ("↓/j: scroll down", Key::Char('j')),
            ] {
                if let Some(at) = text.find(label) {
                    self.hit(
                        text[..at].cell_width() as usize,
                        y,
                        label.cell_width() as usize,
                        Hit::Key(key),
                    );
                }
            }
        }
    }
}

pub(super) fn sgr_fg(color: Color) -> String {
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
