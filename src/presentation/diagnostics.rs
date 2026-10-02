use std::path::Path;

use ratatui::{buffer::CellWidth, style::Style, text::Span};

use super::{clean, clipped, sgr_fg, wrapped};
use crate::selection::Ui;

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

    pub(super) fn error_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
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
}
