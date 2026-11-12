use ratatui::{buffer::CellWidth, style::Style, text::Span};

use super::{clean, sgr_bg, sgr_fg};
use crate::search::match_positions;
use crate::selection::{Hit, Ui};

// Cards clip by complete terminal graphemes, not bytes or Unicode scalar count.
fn card_text(
    text: &str,
    width: usize,
    query: &str,
    accent: &str,
    reset: &str,
    ansi: bool,
) -> String {
    let text = clean(text);
    let clipped = text.cell_width() as usize > width;
    let budget = width.saturating_sub(usize::from(clipped));
    let matches = match_positions(&text, query);
    let source = Span::raw(&text);
    let mut result = String::new();
    let mut cells = 0;
    let mut position = 0;
    for grapheme in source.styled_graphemes(Style::default()) {
        let count = grapheme.symbol.chars().count();
        let size = grapheme.symbol.cell_width() as usize;
        if cells + size > budget {
            break;
        }
        let marked = ansi && (position..position + count).any(|i| matches.contains(&i));
        if marked {
            result.push_str(accent);
            result.push_str("\x1b[4m");
        }
        result.push_str(grapheme.symbol);
        if marked {
            result.push_str("\x1b[24m");
            result.push_str(reset);
        }
        cells += size;
        position += count;
    }
    if clipped && width > 0 {
        result.push('…');
    }
    result
}

impl Ui {
    pub(super) fn agent_cards(
        &self,
        width: usize,
        height: usize,
        ansi: bool,
        y: usize,
    ) -> Vec<String> {
        let visible = self.visible();
        let selected = self.selected_index(&visible);
        let theme = self.theme();
        let accent = sgr_fg(theme.accent);
        let reset = sgr_fg(theme.text);
        let extent = width.saturating_sub(4);
        let text_width = extent.saturating_sub(2);
        let mut cards = Vec::new();
        for &index in &visible {
            let row = &self.rows[index];
            let location = format!("{} › {} › {}", row.workspace, row.tab, row.pane);
            let mut details = vec![location.clone()];
            if Some(index) == selected {
                if let Some((_, name)) = &row.agent_metadata.repository {
                    details.push(format!("Repository: {name}"));
                }
                if let Some(branch) = &row.agent_metadata.branch {
                    details.push(format!("Branch: {branch}"));
                }
                if let Some(title) = row
                    .agent_metadata
                    .title
                    .as_deref()
                    .filter(|title| !title.trim().is_empty() && *title != location)
                {
                    details.push(format!("Titel: {title}"));
                }
            }
            let status = if row.status.is_empty() {
                "unknown"
            } else {
                &row.status
            };
            let status_width = (status.cell_width() as usize).min(text_width.saturating_sub(3));
            let name_width = text_width.saturating_sub(status_width + 3);
            let agent = if row.agent.is_empty() {
                "Agent"
            } else {
                &row.agent
            };
            let mut header = card_text(agent, name_width, &self.query, &accent, &reset, ansi);
            header.push_str(" | ");
            if ansi {
                header.push_str(&sgr_fg(self.status_colors.themed(status, theme)));
            }
            header.push_str(&card_text(status, status_width, "", &accent, &reset, false));
            if ansi {
                header.push_str(&sgr_fg(theme.text));
            }
            let mut card = vec![header];
            card.extend(
                details
                    .iter()
                    .map(|text| card_text(text, text_width, &self.query, &accent, &reset, ansi)),
            );
            cards.push(card);
        }
        let available = height.saturating_sub(2);
        let span = |start: usize, end: usize| {
            cards[start..=end]
                .iter()
                .map(|card| card.len() + 1)
                .sum::<usize>()
                .saturating_sub(1)
        };
        let mut start = if ansi {
            self.viewport_start.get().min(cards.len() - 1)
        } else {
            0
        };
        if ansi {
            let mut max = cards.len() - 1;
            while max > 0 && span(max - 1, cards.len() - 1) <= available {
                max -= 1;
            }
            self.mouse.borrow_mut().scroll_max = max;
            start = start.min(max);
            if self.follow_selection {
                if let Some(at) = visible.iter().position(|&index| Some(index) == selected) {
                    start = start.min(at);
                    while start < at && span(start, at) > available {
                        start += 1;
                    }
                }
            }
            self.viewport_start.set(start);
        }
        let rule = format!("  {}", "─".repeat(extent));
        let mut lines = vec![rule.clone()];
        for (at, card) in cards.iter().enumerate().skip(start) {
            if ansi && span(start, at) > available {
                break;
            }
            if at > start {
                lines.push(String::new());
            }
            let active = Some(visible[at]) == selected;
            for (line_at, text) in card.iter().enumerate() {
                self.hit(
                    2,
                    y + lines.len(),
                    extent,
                    Hit::Pane(self.rows[visible[at]].id.clone()),
                );
                let prefix = if active && line_at == 0 { "> " } else { "  " };
                lines.push(if ansi && active {
                    let plain: String = text
                        .split('\x1b')
                        .enumerate()
                        .map(|(at, part)| {
                            if at == 0 {
                                part
                            } else {
                                part.split_once('m').unwrap().1
                            }
                        })
                        .collect();
                    format!(
                        "  {}{prefix}{text}{}\x1b[0m",
                        sgr_bg(theme.selection),
                        " ".repeat(text_width.saturating_sub(plain.cell_width() as usize))
                    )
                } else {
                    format!("  {prefix}{text}")
                });
            }
        }
        if ansi {
            lines.resize(height.saturating_sub(1), String::new());
        }
        lines.push(rule);
        lines
    }
}
