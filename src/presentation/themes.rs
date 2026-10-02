use super::{clipped, sgr_bg};
use crate::keymap::Context;
use crate::selection::Ui;

impl Ui {
    pub(super) fn theme_layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let footer_rows = self.hint_rows(Context::Themes, width, height);
        let limit = width.saturating_sub(1);
        let aligned = |text: &str| format!("  {}", clipped(text, limit.saturating_sub(2)));
        let names = &crate::theme::NAMES;
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
