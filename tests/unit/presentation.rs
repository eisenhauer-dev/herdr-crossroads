use super::test_support::*;
use crate::input::Key;
use crate::presentation::{clipped, marked, StatusColors};
use crate::selection::{Mode, Ui as ProductionUi};
use crate::topology::{annotate_worktrees, rows};
use ratatui::{
    buffer::CellWidth,
    style::{Color, Modifier},
};
use serde_json::Value;

// These layout assertions predate theme painting. Compare their original ANSI
// content while the theme frame test checks the actual painted output.
struct Ui(ProductionUi);

impl Ui {
    fn new(rows: Vec<crate::topology::Row>) -> Self {
        let mut state = ProductionUi::new(rows);
        state.set_auto_choice("catppuccin", None);
        Self(state)
    }

    fn terminal_frame(&self, width: usize, height: usize) -> String {
        let frame = self.0.terminal_frame(width, height);
        frame
            .replace("\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m", "")
            .replace("\x1b[48;2;24;24;37m\x1b[K", "")
            .replace("\x1b[38;2;205;214;244m", "\x1b[39m")
    }
}

impl std::ops::Deref for Ui {
    type Target = ProductionUi;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Ui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn ui() -> Ui {
    Ui(super::test_support::ui())
}

#[test]
fn help_category_spacing_tracks_only_visible_groups() {
    for (width, height) in [(80, 15), (120, 24)] {
        for (query, headings) in [
            ("", vec!["Navigation", "Actions"]),
            ("pane", vec!["Navigation", "Actions"]),
            ("previous", vec!["Navigation"]),
            ("focus", vec!["Actions"]),
            ("Navigation", vec![]),
            ("not-a-key", vec![]),
        ] {
            let mut state = super::test_support::ui();
            state.key(Key::Char('?'));
            state.help.query = query.into();
            let full = state.layout(width, height, false);
            let body = &full[2..full.len() - 2];
            assert!(!body.first().unwrap().trim().is_empty(), "{full:?}");
            assert!(!body.last().unwrap().trim().is_empty(), "{full:?}");
            assert_eq!(
                body.iter().filter(|line| line.trim().is_empty()).count(),
                headings.len().saturating_sub(1),
                "query={query:?}: {full:?}"
            );
            let actual_headings = body
                .iter()
                .filter(|line| matches!(line.trim(), "Navigation" | "Actions"))
                .map(|line| line.trim())
                .collect::<Vec<_>>();
            assert_eq!(actual_headings, headings, "{full:?}");
            if headings.len() == 2 {
                let actions = body.iter().position(|line| line == "  Actions").unwrap();
                assert!(body[actions - 1].trim().is_empty(), "{full:?}");
                assert!(!body[actions - 2].trim().is_empty(), "{full:?}");
            }
            let lines = state.layout(width, height, true);
            assert_eq!(lines.len(), height);
            assert_eq!(lines[0], full[0]);
            assert_eq!(lines[1], full[1]);
            assert_eq!(lines[height - 2], full[full.len() - 2]);
            assert_eq!(lines[height - 1], full[full.len() - 1]);
            if headings.len() == 2 {
                let actions = lines.iter().position(|line| line == "  Actions").unwrap();
                assert!(lines[actions - 1].trim().is_empty(), "{lines:?}");
                assert!(!lines[actions - 2].trim().is_empty(), "{lines:?}");
            } else {
                // Remaining blank viewport rows are padding, not content spacers.
                assert!(
                    lines[2..2 + body.len()]
                        .iter()
                        .all(|line| !line.trim().is_empty()),
                    "{lines:?}"
                );
            }
            if headings.is_empty() {
                assert_eq!(body, ["  No matching keys"]);
                assert!(!lines.iter().any(|line| line.contains("> ")));
            }
        }
    }
}

#[test]
fn help_category_spacer_counts_toward_scroll_budget_not_selection() {
    for (width, height) in [(80, 15), (120, 24)] {
        let mut state = super::test_support::ui();
        state.key(Key::Char('?'));
        let entry_count = state.help.context.entries().len();
        for selected in 0..entry_count {
            let lines = state.layout(width, height, true);
            assert_eq!(state.help.selected, Some(selected));
            assert_eq!(lines.iter().filter(|line| line.contains("> ")).count(), 1);
            assert!(
                lines[2..height - 2].iter().any(|line| {
                    line.contains("> ")
                        && line.contains(state.help.context.entries()[selected].meaning)
                }),
                "{lines:?}"
            );
            assert_eq!(lines[height - 2], lines[1]);
            assert!(lines[height - 1].contains("Esc/q/?: close keys"));
            state.key(Key::Down);
        }
        let bottom = state.layout(width, height, true);
        assert_eq!(state.help.selected, Some(entry_count - 1));
        assert_eq!(
            state.help.viewport_start.get(),
            14usize.saturating_sub(height - 4)
        );
        if height == 15 {
            assert!(
                bottom[height - 3].contains("close Crossroads"),
                "{bottom:?}"
            );
            state.help.viewport_start.set(9);
            state.help.selected = Some(6);
            state.key(Key::Up);
            // Exercise heading restoration with a smaller internal content budget.
            let lines = state.layout(width, 7, true);
            assert_eq!(lines[2], "  Actions", "{lines:?}");
            assert!(lines[3].contains("> Enter"), "{lines:?}");
            state.key(Key::Up);
            let lines = state.layout(width, 7, true);
            assert!(lines[2].contains("> Tab/Shift+Tab"), "{lines:?}");
            assert!(lines[3].trim().is_empty(), "{lines:?}");
            assert_eq!(lines[4], "  Actions", "{lines:?}");
        }
        for _ in 0..entry_count {
            state.key(Key::Up);
            state.layout(width, height, true);
        }
        let top = state.layout(width, height, true);
        assert_eq!(state.help.selected, Some(0));
        assert_eq!(state.help.viewport_start.get(), 0);
        assert_eq!(top[2], "  Navigation");
        assert!(top[3].contains("> j/Down"));
    }
}

#[test]
fn help_fixed_chrome_scroll_alignment_and_theme_matches() {
    for theme_index in 0..crate::theme::NAMES.len() {
        let mut state = super::test_support::ui();
        state.theme_index = theme_index;
        state.popup_size = Some((80, 15));
        state.key(Key::Char('?'));
        for (width, height) in [(80, 15), (109, 15), (110, 15), (162, 36)] {
            let first = state.layout(width, height, false);
            let meaning_columns = state
                .help
                .context
                .entries()
                .iter()
                .map(|entry| {
                    let line = first
                        .iter()
                        .find(|line| {
                            line.get(4..).is_some_and(|s| s.starts_with(entry.keys))
                                && line.ends_with(entry.meaning)
                        })
                        .unwrap();
                    line.len() - entry.meaning.len()
                })
                .collect::<Vec<_>>();
            assert!(
                meaning_columns.iter().all(|&column| column == 19),
                "{first:?}: {meaning_columns:?}"
            );
            for _ in 0..15 {
                let lines = state.layout(width, height, true);
                assert_eq!(lines.len(), height, "{lines:?}");
                assert_eq!(lines[0], "  Search: / to search");
                assert_eq!(lines[1], "  ".to_owned() + &"─".repeat(width - 4));
                assert_eq!(lines[height - 2], lines[1]);
                assert!(
                    lines[height - 1].contains("/: search")
                        && lines[height - 1].contains("r: reset")
                        && lines[height - 1].contains("Esc/q/?: close keys")
                );
                let active = lines
                    .iter()
                    .filter(|line| line.contains("> "))
                    .collect::<Vec<_>>();
                assert_eq!(active.len(), 1, "{lines:?}");
                assert!(active[0].contains("\x1b[48;") || active[0].contains("\x1b[49;7m"));
                assert!(!lines
                    .iter()
                    .any(|line| line.contains("Keys ·") || line.contains("Meaning")));
                let frame = state.terminal_frame(width, height);
                assert_eq!(frame.split("\r\n").count(), height);
                state.key(Key::Down);
            }
            assert!(state
                .layout(width, height, true)
                .iter()
                .any(|line| line.contains("close Crossroads")));
            for _ in 0..15 {
                state.key(Key::Up);
                state.layout(width, height, true);
            }
            assert!(state
                .layout(width, height, true)
                .iter()
                .any(|line| line.contains("next pane")));
            assert_eq!(state.layout(width, height, true)[2], "  Navigation");
        }
        for query in ["DN", "focus", "e"] {
            state.help.query = query.into();
            let frame = state.terminal_frame(80, 15);
            assert!(frame.contains("\x1b[4m"), "{frame}");
            let accent = if theme_index <= 1 {
                "\x1b[38;2;180;190;254m".to_owned()
            } else if state.theme().accent == Color::Blue {
                "\x1b[34m".to_owned()
            } else if let Color::Rgb(r, g, b) = state.theme().accent {
                format!("\x1b[38;2;{r};{g};{b}m")
            } else {
                panic!("unexpected accent")
            };
            assert!(frame.contains(&format!("{accent}\x1b[4m")), "{frame}");
        }
        state.help.query.clear();
        assert!(!state.terminal_frame(80, 15).contains("\x1b[4m"));
        state.popup_size = Some((79, 15));
        assert!(state
            .terminal_frame(79, 15)
            .contains("minimum content size 80x15"));
        state.key(Key::Char('/'));
        assert!(!state.help.searching);
        state.popup_size = Some((80, 15));
        assert!(state.terminal_frame(80, 15).contains("Search:"));
    }
}

#[test]
fn help_marks_first_positions_in_each_matching_column_only() {
    let mut state = super::test_support::ui();
    state.key(Key::Char('?'));
    state.help.query = "E".into();
    let lines = state.layout(80, 20, true);
    let accent = "\x1b[38;2;180;190;254m\x1b[4m";
    let end = "\x1b[24m\x1b[38;2;205;214;244m";
    let enter = lines.iter().find(|line| line.contains("focus s")).unwrap();
    assert!(enter.contains(&format!("{accent}E{end}nter")), "{enter}");
    assert!(
        enter.contains(&format!("focus s{accent}e{end}lected pane")),
        "{enter}"
    );
    assert_eq!(enter.matches("\x1b[4m").count(), 2, "{enter}");
    assert_eq!(lines[2], "  Navigation");
    assert!(lines.iter().any(|line| line == "  Actions"));
    state.help.query = "?".into();
    let lines = state.layout(80, 15, true);
    let show = lines
        .iter()
        .find(|line| line.contains("show keys"))
        .unwrap();
    assert!(show.contains(&format!("{accent}?{end}")), "{show}");
    assert_eq!(show.matches("\x1b[4m").count(), 1);
    assert_eq!(lines[2], "  Actions");
}

#[test]
fn marked_distinguishes_generated_from_source_ellipsis() {
    let generated = marked("abcde", 4, &[2], true);
    assert_eq!(
        generated
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "ab…"
    );
    assert!(generated
        .spans
        .iter()
        .all(|span| !span.style.add_modifier.contains(Modifier::UNDERLINED)));

    let source = marked("ab…", 4, &[2], true);
    assert!(source
        .spans
        .iter()
        .any(|span| span.content == "…" && span.style.add_modifier.contains(Modifier::UNDERLINED)));
}

#[test]
fn inline_status_does_not_mark_generated_ellipsis_but_marks_real_ellipsis() {
    let mut state = ui();
    state.rows.truncate(1);
    state.rows[0].agent = "abcccccccccccccccc".into();
    state.query = "abcccccccccccccccc".into();
    let generated = state.terminal_frame(76, 15);
    assert!(!generated.contains("…\x1b[39m\x1b[24m"), "{generated}");

    state.rows[0].agent = "ab…".into();
    state.query = "…".into();
    let source = state.terminal_frame(76, 15);
    assert!(source.contains("\x1b[4m…\x1b[39m\x1b[24m"), "{source}");
}

#[test]
fn marked_keeps_combining_and_joined_graphemes_in_single_spans() {
    for glyph in ["e\u{301}", "👩\u{200d}💻"] {
        for positions in [&[][..], &[0][..], &[1][..]] {
            let line = marked(glyph, 20, positions, true);
            assert_eq!(line.spans.len(), 1, "{glyph:?}: {line:?}");
            assert_eq!(line.spans[0].content, glyph);
            assert_eq!(
                line.spans[0]
                    .style
                    .add_modifier
                    .contains(Modifier::UNDERLINED),
                !positions.is_empty()
            );
        }
        assert_eq!(clipped(&format!("{glyph}abc"), 4), "…");
    }
}

#[test]
fn empty_search_keeps_width_dependent_headers_and_columns() {
    let mut state = ui();
    for width in [80, 109, 110, 150] {
        state.query = "Claude".into();
        let populated = state.terminal_frame(width, 15);
        state.query = "no-such-pane-or-workspace".into();
        let empty = state.terminal_frame(width, 15);
        let headers = |frame: &str| frame.split("\r\n").nth(2).unwrap().trim_start().to_owned();
        assert_eq!(
            headers(&empty),
            headers(&populated),
            "width={width}: {empty}"
        );
        assert!(empty.contains("No matching panes"), "{empty}");
        for label in ["Workspace", "Tab", "Agent/Pane Name"] {
            assert!(headers(&empty).contains(label), "width={width}: {empty}");
        }
        assert_eq!(headers(&empty).contains("Status"), width >= 110, "{empty}");
    }
}

#[test]
fn search_name_glyphs_have_accent_foreground_and_underline_with_status_intact() {
    let mut state = ui();
    state.query = "Claude".into();
    for width in [80, 150] {
        let frame = state.terminal_frame(width, 15);
        let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
        assert!(
            selected.contains("\x1b[38;2;180;190;254m\x1b[4mClaude\x1b[39m\x1b[24m")
                || selected.contains("\x1b[38;2;180;190;254m\x1b[4mC\x1b[39m\x1b[24m"),
            "width={width}: {selected}"
        );
        assert!(selected.contains("\x1b[48;5;236m"), "{selected}");
        assert!(
            selected.contains("\x1b[38;2;249;226;175mworking\x1b[39m"),
            "{selected}"
        );
        assert!(selected.ends_with("\x1b[0m"), "{selected}");
    }
    state.query = "Same".into();
    for width in [80, 150] {
        let frame = state.terminal_frame(width, 15);
        let unselected = frame
            .lines()
            .find(|line| {
                !selected_prefix(line)
                    && line.contains("\x1b[38;2;180;190;254m\x1b[4mSame\x1b[39m\x1b[24m")
            })
            .unwrap();
        assert!(!unselected.contains("\x1b[48;5;236m"), "{frame}");
    }
}

#[test]
fn search_workspace_and_tab_glyphs_have_accent_and_underline_in_both_layouts() {
    let mut state = ui();
    state
        .rows
        .retain(|row| row.id == "p-other" || row.id == "p-b");
    state.selected = Some("p-b".into());
    state.query = "Same".into();
    let marked_same = "\x1b[38;2;180;190;254m\x1b[4mSame\x1b[39m\x1b[24m";
    for width in [76, 160] {
        let frame = state.terminal_frame(width, 15);
        for (name, selected) in [("codex", false), ("Claude", true)] {
            let line = frame.lines().find(|line| line.contains(name)).unwrap();
            assert_eq!(selected_prefix(line), selected, "width={width}: {line}");
            let context = line.split(name).next().unwrap();
            assert_eq!(
                context.matches(marked_same).count(),
                2,
                "width={width}: {line}"
            );
        }
    }
}

#[test]
fn search_marks_leftmost_matches_in_each_displayed_field_without_marking_status() {
    let mut state = ui();
    state.rows.truncate(1);
    state.rows[0].workspace = "aAardvark".into();
    state.rows[0].tab = "aA tab".into();
    state.rows[0].agent = "aA agent".into();
    state.rows[0].pane = "aA pane".into();
    state.query = "aa".into();
    for width in [76, 160] {
        let frame = state.terminal_frame(width, 16);
        let plain_accent = without_name_accent(&frame);
        let line = plain_accent
            .lines()
            .find(|line| line.contains("ardvark"))
            .unwrap();
        assert!(line.contains("\x1b[4maA\x1b[24mardvark"), "{frame}");
        assert!(line.contains("\x1b[4maA\x1b[24m tab"), "{frame}");
        assert!(line.contains("\x1b[4maA\x1b[24m agent"), "{frame}");
        assert!(line.contains("(\x1b[4maA\x1b[24m"), "{frame}");
        if width == 160 {
            assert!(line.contains("\x1b[4maA\x1b[24m pane"), "{frame}");
        }
        assert!(!line.contains("\x1b[4mworking"), "{frame}");
        assert!(line.contains("\x1b[48;5;236m"), "{frame}");
    }
    state.query.clear();
    assert!(!state.terminal_frame(76, 16).contains("\x1b[4m"));
}

#[test]
fn search_marks_only_visible_matching_fields_and_clipped_glyphs() {
    let mut state = ui();
    state.rows.truncate(2);
    state.rows[0].workspace = "Wéé-long-workspace".into();
    state.rows[0].tab = "unmatched".into();
    state.rows[0].agent = "éé-long-agent".into();
    state.rows[0].pane = "not displayed".into();
    state.rows[1].workspace = "hidden".into();
    state.rows[1].tab = "hidden".into();
    state.rows[1].pane = "hidden".into();
    state.rows[1].agent = "hidden".into();
    state.query = "éé".into();
    assert_eq!(state.visible(), [0]);
    let frame = state.terminal_frame(40, 12);
    assert!(
        frame.contains("\x1b[38;2;180;190;254m\x1b[4méé\x1b[39m\x1b[24m"),
        "{frame}"
    );
    assert!(
        !frame.contains("hidden") && !frame.contains("\x1b[4munmatched"),
        "{frame}"
    );
    let narrow = state.terminal_frame(29, 12);
    assert!(!narrow.contains("workspace"), "{narrow}");
    assert_eq!(narrow.matches("\x1b[4m").count(), 2, "{narrow}");
    assert!(
        narrow.contains("\x1b[38;2;180;190;254m\x1b[4mé\x1b[39m\x1b[24m…"),
        "{narrow}"
    );
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
}

#[test]
fn search_marks_greedy_subsequence_not_later_occurrences() {
    let mut state = ui();
    state.rows.truncate(1);
    state.rows[0].workspace = "aabXx".into();
    state.rows[0].tab = "no match".into();
    state.rows[0].agent.clear();
    state.rows[0].pane = "aabXx".into();
    state.query = "ax".into();
    let frame = state.terminal_frame(120, 12);
    assert_eq!(
        without_name_accent(&frame)
            .matches("\x1b[4ma\x1b[24mab\x1b[4mX\x1b[24mx")
            .count(),
        2,
        "{frame}"
    );
    assert!(!frame.contains("\x1b[4mx"), "{frame}");
}

#[test]
fn search_marks_scrolled_selected_rows_but_not_hidden_pane_or_offscreen_rows() {
    let mut state = ui();
    state.query = "Same".into();
    let first = state.terminal_frame(120, 8);
    assert!(
        without_name_accent(&first).contains("\x1b[4mSame\x1b[24m"),
        "{first}"
    );
    assert!(!first.contains("An extremely long pane"), "{first}");
    state.selected = Some("p-a".into());
    let scrolled = state.terminal_frame(120, 8);
    assert!(
        without_name_accent(&scrolled).contains("\x1b[4mSame\x1b[24m"),
        "{scrolled}"
    );
    assert!(
        without_name_accent(&scrolled)
            .lines()
            .any(|line| line.contains("\x1b[48;5;236m") && line.contains("\x1b[4mSame\x1b[24m")),
        "{scrolled}"
    );

    state.rows[0].workspace = "Workshop".into();
    state.query = "Workshop".into();
    let context = state.terminal_frame(120, 16);
    assert!(
        without_name_accent(&context).contains("\x1b[4mWorkshop\x1b[24m"),
        "{context}"
    );
    assert!(
        context.contains("Claude") && !context.contains("(Same)"),
        "{context}"
    );
}

#[test]
fn undersized_popup_displays_only_notice_and_ignores_keys_without_losing_state() {
    let mut state = ui();
    state.theme_index = 3; // Terminal theme retains the four unpainted notice lines.
    state.key(Key::Down);
    state.key(Key::Char('/'));
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    state.key(Key::Enter);
    let selected = state.selected.clone();
    let query = state.query.clone();
    let ids: Vec<_> = state.rows.iter().map(|row| row.id.clone()).collect();
    for (width, height) in [(67, 12), (79, 15), (80, 14)] {
        state.popup_size = Some((width, height));
        let piped = state.frame_width(width);
        assert!(piped.contains("Search: codex") && !piped.contains("Crossroads needs"));
        let frame = state.terminal_frame(width, height);
        assert_eq!(
            frame.split("\r\n").collect::<Vec<_>>(),
            [
                "  Crossroads needs minimum content size 80x15.".to_owned(),
                format!("  Your content size is: {width}x{height}."),
                "  Set larger size in herdr plugin manifest.".to_owned(),
                "  Press Enter/Esc/q to close.".to_owned(),
            ],
            "{frame}"
        );
        assert!(
            !frame.contains("Search:") && !frame.contains("Claude") && !frame.contains("focus"),
            "{frame}"
        );
        for key in [Key::Char('/'), Key::Down, Key::Char('?')] {
            assert!(state.key(key).is_none());
            assert!(!state.closed);
        }
        assert_eq!(state.query, query);
        assert_eq!(state.selected, selected);
        assert_eq!(
            state
                .rows
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
    }
    state.popup_size = Some((80, 15));
    assert!(state.terminal_frame(80, 15).contains("Search: codex"));
    assert_eq!(state.query, query);
    assert_eq!(state.selected, selected);
    for key in [Key::Enter, Key::Esc, Key::Char('q')] {
        state.closed = false;
        state.popup_size = Some((79, 15));
        assert!(state.key(key).is_none(), "must not request focus");
        assert!(state.closed);
    }
    for width in [1, 2, 20] {
        state.popup_size = Some((width, 12));
        let frame = state.terminal_frame(width, 12);
        assert_eq!(frame.split("\r\n").count(), 4, "{frame}");
        assert!(frame
            .split("\r\n")
            .all(|line| usize::from(line.cell_width()) < width));
    }
}

#[test]
fn undersized_popup_enter_closes_without_requesting_focus() {
    let mut state = ui();
    state.popup_size = Some((79, 15));
    assert!(state.key(Key::Enter).is_none());
    assert!(state.closed);
}

#[test]
fn search_and_keyboard_footer_share_table_extent_in_both_modes() {
    use ratatui::buffer::CellWidth;
    for width in [12, 20, 60, 150] {
        let mut state = ui();
        for mode in [Mode::Select, Mode::Search] {
            state.mode = mode;
            state.query = "界".repeat(100);
            let frame = state.terminal_frame(width, 15);
            let lines: Vec<_> = frame.split("\r\n").collect();
            let border = lines[1].cell_width() as usize;
            for line in [lines[0], lines[14]] {
                assert!(line.starts_with("  "), "{frame}");
                assert!(line.cell_width() as usize <= border, "{frame}");
            }
            if width >= 20 {
                assert!(lines[0].starts_with("  Search:"), "{frame}");
            }
            assert!(!frame.contains("Filter:"), "{frame}");
        }
        state.mode = Mode::Select;
        state.query.clear();
        let frame = state.terminal_frame(width, 15);
        if width >= 24 {
            assert!(frame.starts_with("  Search: / to search"), "{frame}");
        }
        assert!(!frame.contains("Filter:"), "{frame}");
    }
}

#[test]
fn compact_popup_columns_grow_stepwise_without_losing_selection_or_status() {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    let mut previous = [0; 3];
    for width in 80..=109 {
        let frame = state.terminal_frame(width, 15);
        let header = frame.split("\r\n").nth(2).unwrap();
        let positions =
            ["Workspace", "Tab", "Agent/Pane Name"].map(|label| header.find(label).unwrap());
        let widths = [
            positions[1] - positions[0] - 1,
            positions[2] - positions[1] - 1,
            width - 8 - (positions[2] - positions[0] - 2) - 2,
        ];
        assert_eq!(widths.iter().sum::<usize>(), width - 10, "{width}: {frame}");
        assert!(
            widths
                .iter()
                .zip(previous)
                .all(|(now, before)| *now >= before),
            "{width}: {widths:?} < {previous:?}"
        );
        if let Some(expected) = [(80, [26, 16, 28]), (104, [35, 24, 35]), (109, [37, 25, 37])]
            .into_iter()
            .find_map(|(at, expected)| (at == width).then_some(expected))
        {
            assert_eq!(widths, expected, "{frame}");
        }
        assert!(!header.contains("Status"), "{frame}");
        assert!(
            frame.lines().any(|line| selected_prefix(line)
                && line.contains("Claude")
                && line.contains("working")),
            "{frame}"
        );
        previous = widths;
    }
    let roomy = state.terminal_frame(110, 15);
    assert!(roomy.lines().nth(2).unwrap().contains("Status"), "{roomy}");
    assert!(
        roomy.lines().nth(2).unwrap().contains("Agent/Pane Name"),
        "{roomy}"
    );
    assert!(state
        .terminal_frame(79, 15)
        .contains("Crossroads needs minimum content size"));

    let selected = state.rows.iter_mut().find(|row| row.id == "p-b").unwrap();
    selected.workspace = "W".repeat(60);
    selected.tab = "T".repeat(60);
    selected.agent = "A".repeat(60);
    selected.status.clear();
    for (width, widths) in [(80, [26, 16, 28]), (104, [35, 24, 35]), (109, [37, 25, 37])] {
        let frame = state.terminal_frame(width, 15);
        let line = frame.lines().find(|line| selected_prefix(line)).unwrap();
        for (letter, cell_width) in [('W', widths[0]), ('T', widths[1]), ('A', widths[2])] {
            assert!(
                line.contains(&format!("{}…", letter.to_string().repeat(cell_width - 2))),
                "{width}: {line}"
            );
        }
    }
}

#[test]
fn popup_switches_all_roomy_chrome_at_110_columns() {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    for width in [109, 110, 111] {
        let frame = state.terminal_frame(width, 20);
        let lines: Vec<_> = frame.split("\r\n").collect();
        let roomy = width >= 110;
        assert_eq!(lines.len(), 20, "{width}: {frame}");
        assert_eq!(lines[2].contains("Status"), roomy, "{width}: {frame}");
        assert_eq!(
            lines[3].starts_with("  \x1b[2m\u{2504}"),
            roomy,
            "{width}: {frame}"
        );
        assert!(lines[2].contains("Agent/Pane Name"), "{width}: {frame}");
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains("Claude")
                && line.contains("working")),
            "{width}: {frame}"
        );
        assert_eq!(
            lines[19].contains("j/k: \u{2195}pane"),
            roomy,
            "{width}: {frame}"
        );
        if !roomy {
            assert!(lines[18].contains("j/k: \u{2195}pane"), "{width}: {frame}");
        }
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
    }
    state.query = "codex".into();
    let filtered = state.terminal_frame(110, 20);
    assert!(
        filtered.lines().nth(2).unwrap().contains("Status"),
        "{filtered}"
    );
    assert!(
        filtered
            .lines()
            .any(|line| selected_prefix(line) && line.contains("codex") && line.contains("idle")),
        "{filtered}"
    );
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
}

#[test]
fn sixty_columns_allocate_workspace_tab_and_minimal_inline_agent_status() {
    let state = ui();
    let frame = state.terminal_frame(60, 15);
    let header = frame.split("\r\n").nth(2).unwrap();
    assert_eq!(
        ["Workspace", "Tab", "Agent/Pane"].map(|label| header.find(label).unwrap()),
        [4, 32, 45],
        "{frame}"
    );
    let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
    assert!(
        selected.contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[39m"),
        "{frame}"
    );
    assert!(!header.contains("Status"), "{frame}");
    assert!(
        frame
            .lines()
            .any(|line| line.contains("pane 3") && !line.contains('|')),
        "{frame}"
    );
    assert!(state.terminal_frame(30, 15).contains("Agent/Pane"));
    assert!(!state.terminal_frame(30, 15).contains("working"));
    let mut too_long = ui();
    too_long.rows[0].status = "busy_now".into();
    let frame = too_long.terminal_frame(60, 15);
    let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
    assert!(
        selected.contains("Claude") && !selected.contains("busy_now"),
        "{frame}"
    );
}

#[test]
fn compact_sixty_column_budget_preserves_workspace_and_status() {
    let state = ui();
    let frame = state.terminal_frame(60, 15);
    let header = frame.split("\r\n").nth(2).unwrap();
    assert_eq!(
        ["Workspace", "Tab", "Agent/Pane"].map(|s| header.find(s).unwrap()),
        [4, 32, 45],
        "{frame}"
    );
    assert!(
        frame
            .lines()
            .any(|line| line.contains("C \x1b[2m|") && line.contains("working")),
        "{frame}"
    );
    assert!(state.terminal_frame(40, 15).contains("Agent/Pane"));
    assert!(state.terminal_frame(150, 15).contains("Status"));
}

#[test]
fn ninety_percent_wide_terminal_header_rule_is_present_only_when_roomy_and_tall() {
    let state = ui();
    let frame = state.terminal_frame(162, 36);
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert!(lines[2].contains("Status"), "{frame}");
    assert_eq!(
        lines[3],
        format!("  \x1b[2m{}\x1b[22m", "┄".repeat(158)),
        "{frame}"
    );
    assert!(!state
        .terminal_frame(109, 36)
        .split("\r\n")
        .nth(3)
        .unwrap()
        .contains('┄'));
    assert!(!state.terminal_frame(162, 7).contains('┄'));
}

#[test]
fn titleless_layout_preserves_selection_filters_and_short_viewport() {
    for width in [60, 150] {
        let mut state = ui();
        for mode in [Mode::Select, Mode::Search, Mode::Help] {
            let search = mode == Mode::Search;
            let help = mode == Mode::Help;
            state.mode = mode;
            let frame = state.terminal_frame(width, 15);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), 15, "{frame}");
            assert!(
                lines[0].starts_with(if search || help {
                    "  Search: "
                } else {
                    "  Search: / to search"
                }),
                "{frame}"
            );
            assert!(!frame.contains("Herdr Crossroads"), "{frame}");
            if !help {
                assert!(lines[1].starts_with("  ─"), "{frame}");
                assert!(
                    lines[2].contains(if width == 60 {
                        "Agent/Pane"
                    } else {
                        "Agent/Pane Name"
                    }),
                    "{frame}"
                );
                assert!(
                    lines[if width == 150 { 4 } else { 3 }].contains(if width == 60 {
                        "C \x1b[2m|"
                    } else {
                        "Claude"
                    }),
                    "{frame}"
                );
            }
        }
        state.mode = Mode::Select;
        state.query = "nomatch".into();
        let empty = state.terminal_frame(width, 15);
        assert!(
            empty.contains("No matching panes") && !empty.contains("Herdr Crossroads"),
            "{empty}"
        );
        state.query.clear();
        state.selected = Some("p-other".into());
        let short = state.terminal_frame(width, 5);
        let lines: Vec<_> = short.split("\r\n").collect();
        assert_eq!(lines.len(), 5, "{short}");
        assert!(
            lines[0].starts_with("  Search:") && lines[1].starts_with("  ─"),
            "{short}"
        );
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{short}"
        );
        assert!(!short.contains('┄'), "{short}");
    }
}

#[test]
fn select_footer_uses_dot_groups_without_right_padding() {
    use ratatui::buffer::CellWidth;
    for width in [60, 76, 109, 110, 111, 119, 120, 121, 137, 138, 139, 150] {
        for height in [5, 15] {
            let frame = ui().terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            let last = lines[height - 1];
            let expected = if width >= 110 {
                "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit"
            } else {
                assert_eq!(
                    lines[height - 2],
                    "  j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace",
                    "{frame}"
                );
                "Enter: focus · Esc/q: exit"
            };
            assert_eq!(last, format!("  {expected} · ?: keys"), "{frame}");
            assert!((last.cell_width() as usize) < width, "{frame}");
            assert!(!last.contains('|'), "{frame}");
        }
    }
}

#[test]
fn roomy_header_rule_is_dim_unselectable_and_respects_viewport_and_filter() {
    let mut state = ui();
    let frame = state.terminal_frame(150, 45);
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert_eq!(lines.len(), 45);
    assert!(lines[2].contains("Status"), "{frame}");
    assert_eq!(
        lines[3],
        format!("  \x1b[2m{}\x1b[22m", "┄".repeat(146)),
        "{frame}"
    );
    assert!(
        selected_prefix(lines[4]) && lines[4].contains("Claude"),
        "{frame}"
    );
    assert!(!lines[3].contains("\x1b[48;5;236m"), "{frame}");
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-a"));

    let mut state = ui();
    let short = state.terminal_frame(150, 5);
    assert!(!short.contains("Status") && !short.contains('┄'), "{short}");
    state.query = "codex".into();
    let filtered = state.terminal_frame(150, 12);
    let lines: Vec<_> = filtered.split("\r\n").collect();
    assert!(
        lines[3].contains('┄') && lines[4].contains("codex"),
        "{filtered}"
    );
    assert_eq!(filtered.matches('┄').count(), 146, "{filtered}");
    state.query = "no matching pane".into();
    let empty = state.terminal_frame(150, 12);
    assert!(
        empty.contains("No matching panes") && empty.contains('┄'),
        "{empty}"
    );

    let mut state = ui();
    state.key(Key::Down);
    let one_pane = state.terminal_frame(150, 8);
    let lines: Vec<_> = one_pane.split("\r\n").collect();
    assert!(lines[3].contains('┄'), "{one_pane}");
    assert!(
        selected_prefix(lines[5]) && lines[5].contains("Same"),
        "{one_pane}"
    );
    assert!(one_pane.contains("Claude"), "{one_pane}");

    let mut state = ui();
    state.terminal_frame(150, 7);
    for _ in 0..4 {
        state.key(Key::Down);
        state.terminal_frame(150, 7);
    }
    let scrolled = state.terminal_frame(150, 7);
    assert!(
        scrolled
            .lines()
            .any(|line| selected_prefix(line) && line.contains("pane 1")),
        "{scrolled}"
    );
    assert!(!scrolled.contains('┄'), "{scrolled}");
}

#[test]
fn compact_and_roomy_footers_show_navigation_with_final_help_group() {
    use ratatui::buffer::CellWidth;
    for width in [60, 76, 109, 110, 111, 119, 120, 121, 137, 138, 139, 150] {
        let state = ui();
        let frame = state.terminal_frame(width, 15);
        let footer = frame.split("\r\n").last().unwrap();
        let navigation = if width >= 110 {
            "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit"
        } else {
            assert_eq!(
                frame.split("\r\n").nth(13),
                Some("  j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace"),
                "{frame}"
            );
            "Enter: focus · Esc/q: exit"
        };
        assert_eq!(
            footer.matches('·').count(),
            if width >= 110 { 5 } else { 2 },
            "{frame}"
        );
        assert!(!footer.contains('|'), "{frame}");
        assert!(footer.starts_with(&format!("  {navigation}")), "{frame}");
        assert_eq!(
            footer.matches('↕').count(),
            usize::from(width >= 110) * 3,
            "{frame}"
        );
        assert!(!footer.contains('↑') && !footer.contains('↓'), "{frame}");
        assert_eq!(footer, format!("  {navigation} · ?: keys"), "{frame}");
        assert!((footer.cell_width() as usize) < width, "{frame}");
        assert_eq!(frame.split("\r\n").count(), 15, "{frame}");
    }
}

#[test]
fn compact_status_shares_name_column_and_reclaims_workspace_width() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["workspaces"][1]["label"] =
        Value::from("Long workspace feature branch name");
    let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    let compact = state.frame_width(60);
    let header = compact.lines().nth(2).unwrap();
    assert!(
        header.contains("Workspace") && header.contains("Agent/Pane"),
        "{compact}"
    );
    assert!(!header.contains("Status"), "{compact}");
    assert!(
        compact
            .lines()
            .any(|line| line.contains("Long workspace") && line.contains("C | working")),
        "{compact}"
    );
    assert!(
        compact.lines().any(|line| line.contains("co… | idle")),
        "{compact}"
    );
    assert!(
        compact
            .lines()
            .any(|line| line.contains("pane 3") && !line.contains('|')),
        "{compact}"
    );
    let wide = state.frame_width(150);
    assert!(wide.lines().nth(2).unwrap().contains("Status"), "{wide}");
    assert!(
        wide.contains("Long workspace feature branch name"),
        "{wide}"
    );
    assert!(!wide.contains("Claude | working"), "{wide}");
}

#[test]
fn roomy_columns_allocate_workspace_tab_name_and_status_without_overflow() {
    use ratatui::buffer::CellWidth;
    for (width, expected) in [
        (110, [4, 48, 76, 94]),
        (139, [4, 60, 96, 123]),
        (150, [4, 65, 104, 134]),
    ] {
        let state = ui();
        let frame = state.terminal_frame(width, 15);
        let lines: Vec<_> = frame.split("\r\n").collect();
        let header = lines[2];
        let positions = ["Workspace", "Tab", "Agent/Pane Name", "Status"]
            .map(|label| header.find(label).expect("roomy column header"));
        assert_eq!(positions, expected, "{frame}");
        let selected = without_status_ansi(lines[4])
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[0m", "");
        assert!(selected.cell_width() as usize <= width - 1, "{frame}");
        assert!(selected[expected[2]..].contains("Claude"), "{frame}");
        assert!(selected[expected[3]..].contains("working"), "{frame}");
    }
}

#[test]
fn inline_status_survives_search_name_expansion_and_truncation_without_escape_injection() {
    let mut state = ui();
    state.query = "suffix".into();
    let frame = state.frame_width(60);
    assert!(frame.contains("co… | idle"), "{frame}");
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["agents"][0]["agent_status"] = Value::from("working\u{1b}[31m");
    let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    let frame = state.terminal_frame(60, 15);
    assert!(!frame.contains("\u{1b}[31m"), "{frame}");
    assert!(!frame.contains("\u{1b}[38;2;249;226;175m"), "{frame}");
}

#[test]
fn status_rgb_only_on_status_and_selected_background_survives_fg_reset() {
    for width in [60, 150] {
        let state = ui();
        let frame = state.terminal_frame(width, 15);
        let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
        assert!(selected_prefix(selected), "{selected}");
        assert!(
            selected.contains("\u{1b}[38;2;249;226;175mworking\u{1b}[39m"),
            "{selected}"
        );
        assert!(selected.ends_with("\u{1b}[0m"), "{selected}");
        assert!(
            !selected.contains("\u{1b}[0mworking")
                && !selected.contains("\u{1b}[38;2;249;226;175mClaude"),
            "{selected}"
        );
        let idle = frame.lines().find(|line| line.contains("idle")).unwrap();
        assert!(
            idle.contains("\u{1b}[38;2;166;227;161midle\u{1b}[39m"),
            "{idle}"
        );
        assert!(!state.frame_width(width).contains('\u{1b}'));
    }
}

#[test]
fn compact_pipe_is_dim_only_between_unstyled_spaces() {
    let mut state = ui();
    let frame = state.terminal_frame(60, 15);
    let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
    assert!(selected.starts_with("  \x1b[48;5;236m> "), "{selected}");
    assert!(
        selected.contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[39m"),
        "{selected}"
    );
    assert!(selected.ends_with("\x1b[0m"), "{selected}");
    let idle = frame.lines().find(|line| line.contains("idle")).unwrap();
    assert!(
        idle.contains("co… \x1b[2m|\x1b[22m \x1b[38;2;166;227;161midle\x1b[39m"),
        "{idle}"
    );
    state.key(Key::Char('/'));
    state.query = "Claude".into();
    let filtered = state.terminal_frame(60, 15);
    assert_eq!(filtered.lines().next().unwrap(), "  Search: Claude");
    assert!(
        without_match_marks(&filtered).contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking")
    );
    let wide = state.terminal_frame(150, 15);
    assert!(wide.lines().nth(2).unwrap().contains("Status"), "{wide}");
    let wide_selected = wide
        .lines()
        .find(|line| selected_prefix(line) && line.contains("Claude"))
        .unwrap();
    assert!(
        !wide_selected.contains("\x1b[2m") && !wide_selected.contains('|'),
        "{wide_selected}"
    );
    assert!(
        wide_selected.contains("\x1b[38;2;249;226;175mworking\x1b[39m"),
        "{wide_selected}"
    );
}

#[test]
fn compact_selected_background_matches_horizontal_table_borders() {
    for (width, height) in [
        (60, 15),
        (76, 15),
        (40, 15),
        (109, 15),
        (110, 15),
        (119, 15),
        (120, 15),
        (137, 15),
        (138, 15),
        (139, 15),
        (150, 15),
        (60, 5),
        (40, 5),
        (138, 5),
    ] {
        let mut state = ui();
        if height == 5 {
            state.selected = Some("p-other".into());
        }
        let frame = state.terminal_frame(width, height);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), height, "{frame}");
        let selected = lines.iter().find(|line| selected_prefix(line)).unwrap();
        let border: Vec<_> = lines[1]
            .chars()
            .enumerate()
            .filter_map(|(x, c)| (c == '─').then_some(x))
            .collect();
        assert!(!border.is_empty(), "{frame}");
        assert_eq!(border, (2..width - 2).collect::<Vec<_>>(), "{frame}");
        if height > 6 {
            let bottom: Vec<_> = lines[height - if width >= 110 { 2 } else { 3 }]
                .chars()
                .enumerate()
                .filter_map(|(x, c)| (c == '─').then_some(x))
                .collect();
            assert_eq!(border, bottom, "{frame}");
        }
        assert!(selected.starts_with("  \x1b[48;5;236m> "), "{selected:?}");
        assert!(selected.ends_with("\x1b[0m"), "{selected:?}");
        let painted = selected.strip_prefix("  \x1b[48;5;236m").unwrap();
        let painted = painted.strip_suffix("\x1b[0m").unwrap();
        let painted = without_status_ansi(painted)
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "");
        assert_eq!(
            (2..2 + painted.cell_width() as usize).collect::<Vec<_>>(),
            border,
            "{selected:?}"
        );
        let plain = without_status_ansi(selected)
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "")
            .replace("\x1b[0m", "");
        assert_eq!(
            plain.cell_width() as usize,
            2 + border.len(),
            "{selected:?}"
        );
        assert!(plain.starts_with("  > "), "{selected:?}");
        assert!(
            painted.ends_with(' '),
            "last painted cell should be blank: {selected:?}"
        );
        assert!(
            lines.iter().all(|line| {
                without_status_ansi(line)
                    .replace("\x1b[48;5;236m", "")
                    .replace("\x1b[2m", "")
                    .replace("\x1b[22m", "")
                    .replace("\x1b[0m", "")
                    .cell_width() as usize
                    <= width - 1
            }),
            "phantom terminal column: {frame}"
        );
        if height == 15 {
            let idle = lines.iter().find(|line| line.contains("idle")).unwrap();
            assert!(!idle.contains("\x1b[48;5;236m"), "{idle:?}");
            assert!(!without_status_ansi(idle).ends_with(' '), "{idle:?}");
            assert!(
                lines.iter().any(|line| line.starts_with("  \x1b[2m┄")),
                "{frame}"
            );
        }
    }
}

#[test]
fn all_status_defaults_and_partial_override_are_read_once() {
    let colors = StatusColors::default();
    for (status, expected) in [
        ("working", Color::Rgb(249, 226, 175)),
        ("blocked", Color::Rgb(243, 139, 168)),
        ("done", Color::Rgb(148, 226, 213)),
        ("idle", Color::Rgb(166, 227, 161)),
        ("unknown", Color::Rgb(108, 112, 134)),
        ("unrecognized", Color::Rgb(108, 112, 134)),
    ] {
        assert_eq!(colors.for_status(status), expected);
    }
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-palette-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("status-colors.json");
    assert_eq!(
        StatusColors::load(Some(&dir)).unwrap().working,
        colors.working
    );
    std::fs::write(&path, r##"{"working":"#123abc","unknown":"#010203"}"##).unwrap();
    let loaded = StatusColors::load(Some(&dir)).unwrap();
    assert_eq!(loaded.working, Color::Rgb(18, 58, 188));
    assert_eq!(loaded.unknown, Color::Rgb(1, 2, 3));
    assert_eq!(loaded.idle, colors.idle);
    let mut state = ui();
    state.status_colors = loaded;
    std::fs::write(&path, r##"{"working":"#FFFFFF"}"##).unwrap();
    for width in [60, 150] {
        let rendered = state.terminal_frame(width, 15);
        assert!(
            rendered.contains("\x1b[38;2;18;58;188mworking\x1b[39m"),
            "{rendered}"
        );
        assert!(!rendered.contains("\x1b[38;2;255;255;255m"), "{rendered}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn status_palette_renders_all_known_and_unknown_values_in_both_layouts() {
    for (status, rgb) in [
        ("working", "249;226;175"),
        ("blocked", "243;139;168"),
        ("done", "148;226;213"),
        ("idle", "166;227;161"),
        ("other", "108;112;134"),
    ] {
        let mut state = ui();
        state.rows[0].status = status.into();
        for width in [60, 110, 150] {
            let frame = state.terminal_frame(width, 15);
            assert!(
                frame.contains(&format!("\x1b[38;2;{rgb}m{status}\x1b[39m")),
                "{frame}"
            );
        }
    }
}

#[test]
fn workspace_separators_are_single_dim_rows_only_between_visible_blocks() {
    for (width, height) in [(60, 15), (150, 45)] {
        let mut state = ui();
        let frame = state.terminal_frame(width, height);
        let lines: Vec<_> = frame.split("\r\n").collect();
        let separators: Vec<_> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains('┄'))
            .collect();
        assert_eq!(
            separators.len(),
            if width == 150 { 2 } else { 1 },
            "{frame}"
        );
        let (at, line) = separators[separators.len() - 1];
        assert!(
            line.starts_with("  \x1b[2m┄") && line.ends_with("\x1b[22m"),
            "{frame}"
        );
        assert!(
            lines[at - 1].contains("pane 1")
                && lines[at + 1].contains(if width == 60 { "idle" } else { "codex" }),
            "{frame}"
        );
        assert!(!frame.contains('▏') && !frame.contains('│'), "{frame}");
        assert!(
            lines[1].starts_with("  ─")
                && lines[height - if width == 150 { 2 } else { 3 }].starts_with("  ─"),
            "{frame}"
        );
        state.query = "codex".into();
        let only = state.terminal_frame(width, height);
        assert_eq!(
            only.lines().filter(|line| line.contains('┄')).count(),
            usize::from(width == 150),
            "{only}"
        );
        assert!(!only.contains('▏'), "{only}");
        assert!(
            only.lines().any(|line| line.contains("Same")
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{only}"
        );
    }
}

#[test]
fn workspace_separator_consumes_viewport_height_without_hiding_selection() {
    for width in [60, 150] {
        let mut state = ui();
        state.terminal_frame(width, 10);
        for _ in 0..5 {
            state.key(Key::Down);
            state.terminal_frame(width, 10);
        }
        let frame = state.terminal_frame(width, 10);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 10, "{frame}");
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{frame}"
        );
        assert_eq!(
            frame.lines().filter(|line| line.contains('┄')).count(),
            if width == 150 { 2 } else { 1 },
            "{frame}"
        );
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        let short = state.terminal_frame(width, 5);
        assert_eq!(short.split("\r\n").count(), 5, "{short}");
        assert!(
            short.lines().any(|line| selected_prefix(line)
                && line.contains("Same")
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{short}"
        );
        assert!(!short.contains('┄'), "{short}");
        state.query = "codex".into();
        let filtered = state.terminal_frame(width, 5);
        assert!(
            !filtered.contains('┄')
                && filtered.contains(if width == 60 { "idle" } else { "codex" }),
            "{filtered}"
        );
    }
}

#[test]
fn outer_border_is_horizontal_and_workspace_rows_have_no_margin_marker() {
    use ratatui::buffer::CellWidth;
    for (width, height) in [(60, 15), (150, 45)] {
        let mut state = ui();
        for height in [5, height] {
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height, "{frame}");
            assert!(
                lines.iter().all(|line| without_status_ansi(line)
                    .replace("\x1b[48;5;236m", "")
                    .replace("\x1b[0m", "")
                    .replace("\x1b[2m", "")
                    .replace("\x1b[22m", "")
                    .cell_width()
                    < width as u16),
                "{frame}"
            );
            assert!(lines[1].starts_with("  ─"), "{frame}");
            if height > 6 {
                assert!(
                    lines[height - if width == 150 { 2 } else { 3 }].starts_with("  ─"),
                    "{frame}"
                );
            }
            assert!(!frame.contains('│') && !frame.contains('├'), "{frame}");
            assert!(
                selected_prefix(
                    lines[if height <= 6 {
                        2
                    } else if width == 150 {
                        4
                    } else {
                        3
                    }]
                ),
                "{frame}"
            );
            if height > 6 {
                let codex = lines
                    .iter()
                    .position(|line| line.contains(if width == 60 { "idle" } else { "codex" }))
                    .unwrap();
                assert!(lines[codex].starts_with("    "), "{frame}");
                assert!(lines[codex].contains("Same"), "{frame}");
                assert_eq!(
                    frame.lines().filter(|line| line.contains('┄')).count(),
                    if width == 150 { 2 } else { 1 },
                    "{frame}"
                );
            } else {
                assert!(!frame.contains('┄'), "{frame}");
            }
            assert!(!frame.contains('▏'), "{frame}");
        }
        state.query = "codex".into();
        let only = state.terminal_frame(width, height);
        assert!(!only.contains('▏'), "{only}");
        assert_eq!(
            only.lines().filter(|line| line.contains('┄')).count(),
            usize::from(width == 150),
            "{only}"
        );
        assert!(
            only.lines().any(|line| selected_prefix(line)
                && line.contains("Same")
                && line.contains(if width == 60 { "idle" } else { "codex" })
                && line.ends_with("\x1b[0m")),
            "{only}"
        );
    }
}

#[test]
fn roomy_workspace_column_shows_long_label_without_changing_compact_column() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let label = "Long workspace feature branch name";
    snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from(label);
    let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    let wide = state.terminal_frame(150, 16);
    assert!(
        wide.lines()
            .any(|line| line.contains(label) && line.contains("Claude")),
        "{wide}"
    );
    let compact = state.terminal_frame(60, 16);
    assert!(!compact.contains(label), "{compact}");
    assert!(
        compact
            .lines()
            .any(|line| line.contains("Long workspace feature br…")
                && line.contains("C \x1b[2m|")
                && line.contains("working")),
        "{compact}"
    );
}

#[test]
fn compact_workspace_blocks_have_one_separator_and_preserve_navigation() {
    let mut state = ui();
    let frame = state.frame_width(60);
    assert_eq!(
        frame.lines().filter(|line| line.contains('┄')).count(),
        1,
        "{frame}"
    );
    let lines: Vec<_> = frame.lines().collect();
    let first = lines
        .iter()
        .position(|line| line.contains("Second") && line.contains("pane 1"))
        .unwrap();
    let next = lines.iter().position(|line| line.contains("idle")).unwrap();
    assert_eq!(next, first + 2, "{frame}");
    assert!(lines[first + 1].starts_with("  ┄"), "{frame}");
    assert_eq!(lines.len(), 14, "{frame}");
    assert!(lines[next].starts_with("    "), "{frame}");
    assert!(
        lines[first].starts_with("  ") && !lines[first].contains('┄'),
        "{frame}"
    );
    assert!(lines[3].starts_with("  > "), "{frame}");

    let compact = state.terminal_frame(60, 15);
    let lines: Vec<_> = compact.split("\r\n").collect();
    assert_eq!(lines.len(), 15, "{compact}");
    assert_eq!(
        compact.lines().filter(|line| line.contains('┄')).count(),
        1,
        "{compact}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("    ") && line.contains("idle")),
        "{compact}"
    );
    state.query = "pane".into();
    let filtered = state.terminal_frame(60, 15);
    let plain_filtered = without_match_marks(&filtered);
    let lines: Vec<_> = plain_filtered.split("\r\n").collect();
    let first = lines
        .iter()
        .position(|line| line.contains("pane 1"))
        .unwrap();
    let next = lines.iter().position(|line| line.contains("idle")).unwrap();
    assert_eq!(next, first + 2, "{filtered}");
    assert_eq!(
        filtered.lines().filter(|line| line.contains('┄')).count(),
        1,
        "{filtered}"
    );
    state.query = "codex".into();
    let only = state.terminal_frame(60, 15);
    assert!(!only.contains('┄') && !only.contains('▏'), "{only}");
    assert!(
        only.lines()
            .any(|line| selected_prefix(line) && line.contains("Same")),
        "{only}"
    );
    state.query.clear();

    state.key(Key::Right);
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    let scrolled = state.terminal_frame(60, 10);
    assert_eq!(scrolled.split("\r\n").count(), 10, "{scrolled}");
    assert!(!scrolled.contains('▏'), "{scrolled}");
    assert!(
        scrolled.lines().any(|line| selected_prefix(line)
            && line.contains("idle")
            && line.ends_with("\x1b[0m")),
        "{scrolled}"
    );
    state.key(Key::Up);
    let previous = state.terminal_frame(60, 10);
    assert!(
        previous
            .lines()
            .any(|line| selected_prefix(line) && line.contains("pane 1")),
        "{previous}"
    );
    state.key(Key::Down);
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
}

#[test]
fn roomy_workspace_separator_is_one_physical_row() {
    use ratatui::buffer::CellWidth;
    let mut state = ui();
    let frame = state.terminal_frame(150, 20);
    let lines: Vec<_> = frame.split("\r\n").collect();
    let first = lines
        .iter()
        .position(|line| line.contains("Second") && line.contains("pane 1"))
        .unwrap();
    let second = lines
        .iter()
        .position(|line| line.contains("codex"))
        .unwrap();
    assert_eq!(second, first + 2, "{frame}");
    assert!(
        lines[second].starts_with("    ") && lines[second].contains("codex"),
        "{frame}"
    );
    assert!(lines[first + 1].starts_with("  \x1b[2m┄"), "{frame}");
    assert_eq!(
        frame.lines().filter(|line| line.contains('┄')).count(),
        2,
        "{frame}"
    );
    assert!(!lines[first].contains('┄'), "{frame}");
    assert!(lines[4].starts_with("  \x1b[48;5;236m> "), "{frame}");
    for line in &lines {
        let plain = without_status_ansi(line)
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[0m", "")
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "");
        assert!(plain.cell_width() < 150, "phantom column: {line:?}");
    }
    let plain = state.frame_width(150);
    assert_eq!(plain.lines().count(), 14, "{plain}");
    assert!(!plain.contains('\x1b'), "{plain}");
    assert!(
        plain
            .lines()
            .any(|line| line.starts_with("    ") && line.contains("codex")),
        "{plain}"
    );
    state.query = "pane".into();
    let filtered = state.terminal_frame(150, 20);
    let plain_filtered = without_match_marks(&filtered);
    let lines: Vec<_> = plain_filtered.split("\r\n").collect();
    let first = lines
        .iter()
        .position(|line| line.contains("pane 1"))
        .unwrap();
    let second = lines
        .iter()
        .position(|line| line.contains("codex"))
        .unwrap();
    assert_eq!(second, first + 2, "{filtered}");
    assert!(lines[first + 1].starts_with("  \x1b[2m┄"), "{filtered}");
    state.query = "codex".into();
    let only = state.frame_width(150);
    assert!(!only.contains('▏'), "{only}");
    assert_eq!(
        only.lines().filter(|line| line.contains('┄')).count(),
        1,
        "{only}"
    );
    assert!(
        only.lines()
            .any(|line| line.contains("Same") && line.contains("codex")),
        "{only}"
    );
    state.query.clear();
    state.key(Key::Right);
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
}

#[test]
fn scrolling_across_roomy_workspace_border_keeps_selected_pane_visible() {
    let mut state = ui();
    let width = 150;
    let height = 10;
    state.terminal_frame(width, height);
    for _ in 0..5 {
        state.key(Key::Down);
        state.terminal_frame(width, height);
    }
    let frame = state.terminal_frame(width, height);
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert_eq!(lines.len(), height, "{frame}");
    let selected = lines
        .iter()
        .position(|line| selected_prefix(line) && line.contains("codex"))
        .unwrap();
    assert!(selected > 4, "{frame}");
    assert!(lines[selected - 1].contains('┄'), "{frame}");
    assert!(lines[selected - 2].contains("pane 1"), "{frame}");
    assert!(lines[selected].starts_with("  \x1b[48;5;236m> "), "{frame}");
    assert!(lines[selected].ends_with("\x1b[0m"), "{frame}");
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    let short = state.terminal_frame(width, 5);
    let lines: Vec<_> = short.split("\r\n").collect();
    assert_eq!(lines.len(), 5, "{short}");
    assert!(
        lines
            .iter()
            .any(|line| selected_prefix(line) && line.contains("Same") && line.contains("codex")),
        "{short}"
    );
    state.key(Key::Up);
    let back = state.terminal_frame(width, height);
    assert!(
        back.lines()
            .any(|line| line.starts_with("  \x1b[48;5;236m> ")
                && line.contains("Second")
                && line.contains("pane 1")),
        "{back}"
    );
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-second-tab"));
}

#[test]
fn viewport_start_inside_workspace_repeats_context_without_selecting_it() {
    for (width, height) in [(60, 15), (150, 45)] {
        let mut state = ui();
        state.selected = Some("p-untitled".into());
        state.viewport_start.set(1);
        state.last_selected_at.set(Some(2));
        let frame = state.terminal_frame(width, height);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), height, "{frame}");
        assert_eq!(state.viewport_start.get(), 1, "{frame}");
        assert!(
            lines[if width == 150 { 4 } else { 3 }].starts_with("    Same")
                && lines[if width == 150 { 4 } else { 3 }].contains("Same"),
            "{frame}"
        );
        assert!(
            selected_prefix(lines[if width == 150 { 5 } else { 4 }])
                && lines[if width == 150 { 5 } else { 4 }].contains("pane 3"),
            "{frame}"
        );
        assert_eq!(
            frame.lines().filter(|line| line.contains('┄')).count(),
            if width == 150 { 2 } else { 1 },
            "{frame}"
        );
        assert!(!frame.contains('▏'), "{frame}");
    }
}

#[test]
fn wide_workspace_glyph_does_not_add_a_phantom_terminal_column() {
    use ratatui::buffer::CellWidth;
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from("↳ 界 (feature/demo)");
    let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    let frame = state.terminal_frame(76, 12);
    assert!(
        frame.contains("↳ 界 ("),
        "wide glyph acquired an extra cell: {frame}"
    );
    for line in frame.split("\r\n") {
        let plain = without_status_ansi(line)
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[0m", "")
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "");
        assert!(plain.cell_width() < 76, "phantom column: {plain:?}");
    }
}

#[test]
fn ordinary_workspace_label_with_link_marker_keeps_normal_separator() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["workspaces"][0]["label"] = Value::from("↳ Ordinary");
    let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 150).unwrap());
    for width in [60, 150] {
        for frame in [state.frame_width(width), state.terminal_frame(width, 20)] {
            let (dim, reset) = if frame.contains('\x1b') {
                ("\x1b[2m", "\x1b[22m")
            } else {
                ("", "")
            };
            let expected = format!("  {dim}{}{reset}", "┄".repeat(width - 4));
            let rule = frame
                .lines()
                .filter(|line| line.contains('┄'))
                .last()
                .unwrap();
            assert_eq!(rule, expected, "{frame}");
            assert!(frame.contains("↳ Ordinary"), "{frame}");
        }
    }
}

#[cfg(unix)]
#[test]
fn duplicate_linked_entries_mark_workspace_once_and_indent_its_separator() {
    use ratatui::buffer::CellWidth;
    use std::os::unix::fs::PermissionsExt;
    let fake = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-duplicate-{}", std::process::id()));
    let response = serde_json::json!({"result":{"type":"worktree_list","worktrees":[
        {"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"},
        {"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"}
    ]}});
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response)).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut items = rows(SNAPSHOT, Some("p-self"), 120).unwrap();
    annotate_worktrees(&mut items, fake.as_os_str(), false);
    std::fs::remove_file(fake).unwrap();

    for row in items.iter().filter(|row| row.workspace_id == "w2") {
        assert_eq!(row.workspace, "↳ Same (feature/demo)");
    }
    let mut state = Ui::new(items);
    for width in [60, 150] {
        let rule_width = width - 4;
        for frame in [state.frame_width(width), state.terminal_frame(width, 20)] {
            let (dim, reset) = if frame.contains('\x1b') {
                ("\x1b[2m", "\x1b[22m")
            } else {
                ("", "")
            };
            let expected = format!("      {dim}{}{reset}", "┄".repeat(rule_width - 4));
            // The roomy header rule is unchanged; only the rule before the linked row moves.
            let separators: Vec<_> = frame.lines().filter(|line| line.contains('┄')).collect();
            let linked_rule = separators.last().unwrap();
            let linked_row = frame.lines().find(|line| line.contains("↳ Same")).unwrap();
            let arrow_col = linked_row
                .split_once('↳')
                .unwrap()
                .0
                .replace("\x1b[48;5;236m", "")
                .cell_width();
            let rule_col = linked_rule
                .split_once('┄')
                .unwrap()
                .0
                .replace(dim, "")
                .cell_width();
            assert_eq!(rule_col, arrow_col + "↳ ".cell_width(), "{frame}");
            assert_eq!(
                linked_rule.replace(dim, "").replace(reset, "").cell_width(),
                (width - 2) as u16,
                "{frame}"
            );
            assert!(!linked_rule.contains('↳'), "{frame}");
            assert_eq!(*linked_rule, expected, "{frame}");
            if width == 150 {
                assert_eq!(
                    separators[0],
                    format!("  {dim}{}{reset}", "┄".repeat(rule_width)),
                    "{frame}"
                );
            }
            assert_eq!(separators.len(), if width == 150 { 2 } else { 1 });
            assert_eq!(frame.matches("↳ Same").count(), 1, "{frame}");
            assert!(
                frame.lines().any(|line| line.contains("↳ Same")
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{frame}"
            );
        }
        state.query = "pane".into();
        let filtered = state.terminal_frame(width, 20);
        assert!(
            filtered
                .lines()
                .any(|line| line.starts_with("      \x1b[2m┄")),
            "{filtered}"
        );
        state.query = "codex".into();
        let only = state.terminal_frame(width, 20);
        assert_eq!(
            only.lines().filter(|line| line.contains('┄')).count(),
            usize::from(width == 150),
            "{only}"
        );
        state.query.clear();
        state.key(Key::Tab);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        let scrolled = state.terminal_frame(width, 10);
        assert!(
            scrolled
                .lines()
                .any(|line| line.starts_with("      \x1b[2m┄")),
            "{scrolled}"
        );
        assert!(
            scrolled.lines().any(|line| selected_prefix(line)
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{scrolled}"
        );
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    }
}

#[test]
fn four_columns_deduplicate_context_and_repeat_it_at_viewport_and_filter_start() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["workspaces"][0]["label"] = Value::from("Studio");
    snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from("Workshop");
    snapshot["result"]["snapshot"]["tabs"][0]["label"] = Value::from("Ops");
    snapshot["result"]["snapshot"]["tabs"][1]["label"] = Value::from("Backend");
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    for width in [76, 140] {
        let frame = state.terminal_frame(width, 16);
        assert!(
            frame.contains("Workspace")
                && frame.contains("Tab")
                && frame.contains("Agent/Pane Name")
                && frame.lines().nth(2).unwrap().contains("Status") == (width == 140),
            "{frame}"
        );
        assert!(
            !frame.contains("Pane ID")
                && !frame.contains("p-b")
                && !frame.contains("p-a")
                && !frame.contains("W1/T1")
                && !frame.contains("popup"),
            "{frame}"
        );
        assert_eq!(frame.matches("Workshop").count(), 1, "{frame}");
        assert_eq!(frame.matches("Backend").count(), 1, "{frame}");
        assert_eq!(frame.matches("Studio").count(), 1, "{frame}");
        assert!(
            frame.lines().any(|line| line.contains("Workshop")
                && line.contains("Backend")
                && line.contains("Claude")
                && line.contains("working")),
            "{frame}"
        );
        assert!(
            frame.lines().any(|line| line.contains("Same")
                && !line.contains("working")
                && !line.contains("Backend")),
            "{frame}"
        );
        assert!(
            frame.lines().all(|line| without_status_ansi(line)
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "")
                .chars()
                .count()
                < width),
            "{frame}"
        );
        state.selected = Some("p-a".into());
        let scrolled = state.terminal_frame(width, 7);
        assert!(
            scrolled.lines().any(|line| line.contains("Workshop")
                && line.contains("Backend")
                && (line.contains("Claude") || line.contains("Same")))
                && scrolled
                    .lines()
                    .any(|line| selected_prefix(line) && line.contains("Same")),
            "{scrolled}"
        );
        assert!(!scrolled.contains("p-a"), "{scrolled}");
        state.query = "pane 3".into();
        let filtered = state.terminal_frame(width, 16);
        assert!(
            filtered.lines().any(|line| line.contains("Workshop")
                && line.contains("Backend")
                && line.contains("pane 3")),
            "{filtered}"
        );
        assert!(
            !filtered.contains("Claude") && !filtered.contains("p-untitled"),
            "{filtered}"
        );
        state.query.clear();
        state.selected = Some("p-b".into());
        state.viewport_start.set(0);
        state.last_selected_at.set(None);
    }
}

#[test]
fn ratatui_table_has_headers_border_and_full_row_selection() {
    let state = ui();
    let frame = state.terminal_frame(76, 12);
    assert!(
        frame.contains("Workspace")
            && frame.contains("Tab")
            && frame.contains("Agent/Pane Name")
            && !frame.lines().nth(2).unwrap().contains("Status"),
        "{frame}"
    );
    assert!(frame.lines().nth(1).unwrap().starts_with("  ─"), "{frame}");
    assert!(frame.lines().nth(9).unwrap().starts_with("  ─"), "{frame}");
    assert!(
        frame.contains("\x1b[48;5;236m")
            && frame.lines().any(|line| selected_prefix(line)
                && line.contains("Claude")
                && line.ends_with("\x1b[0m")),
        "{frame}"
    );
    assert!(
        !frame.contains("p-b") && !frame.contains("Pane ID"),
        "{frame}"
    );
}

#[test]
fn compact_layouts_keep_visible_context_and_inline_status() {
    let mut state = ui();
    for width in [76, 109] {
        let frame = state.terminal_frame(width, 20);
        assert!(
            frame.contains("Workspace")
                && frame.contains("Tab")
                && frame.contains("Agent/Pane Name")
                && !frame.lines().nth(2).unwrap().contains("Status"),
            "{frame}"
        );
        assert!(
            frame.lines().any(|line| selected_prefix(line)
                && line.contains("Claude")
                && line.contains("working")),
            "{frame}"
        );
        assert!(
            frame
                .lines()
                .any(|line| line.contains("codex") && line.contains("idle")),
            "{frame}"
        );
        assert!(
            !frame.contains("p-b") && !frame.contains("W1/T1") && !frame.contains(" / T"),
            "{frame}"
        );
    }
    assert!(!state.frame_width(76).contains('\x1b'));
    state.query = "Second".into();
    let filtered = state.terminal_frame(120, 20);
    assert!(
        filtered
            .lines()
            .any(|line| line.contains("Second") && line.contains("pane 1")),
        "{filtered}"
    );
    assert!(
        !filtered.contains("Claude") && !filtered.contains("Backend"),
        "{filtered}"
    );
}

#[test]
fn compact_status_uses_spare_columns_but_yields_to_name() {
    let state = ui();
    let roomy = state.terminal_frame(76, 20);
    assert!(
        roomy
            .lines()
            .any(|line| line.contains("Claude") && line.contains("working")),
        "{roomy}"
    );
    assert!(
        roomy
            .lines()
            .any(|line| line.contains("codex") && line.contains("idle")),
        "{roomy}"
    );
    assert!(
        roomy.lines().any(|line| line.contains("Same")
            && !line.contains("idle")
            && !line.contains("Claude")),
        "{roomy}"
    );
    let constrained = state.terminal_frame(29, 20);
    assert!(
        constrained.contains("Claude")
            && !constrained.contains("Status")
            && !constrained.contains("working"),
        "{constrained}"
    );
    assert!(!constrained.contains("p-b"), "{constrained}");
}

#[test]
fn table_block_uses_nearly_full_width_without_moving_chrome() {
    for width in [40, 86, 96, 109, 110, 119, 120, 137, 138, 140] {
        let state = ui();
        let frame = state.terminal_frame(width, 16);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines[0], "  Search: / to search");
        assert!(
            lines[15].starts_with(if width >= 110 {
                "  j/k: ↕pane"
            } else {
                "  Enter: focus"
            }),
            "{frame}"
        );
        assert!(lines[1].starts_with("  ─"), "{frame}");
        assert!(frame.lines().any(|line| line.contains("Claude")), "{frame}");
        assert_eq!(lines[1].chars().count(), width - 2, "{frame}");
        assert!(
            lines[if width >= 110 { 14 } else { 13 }].starts_with("  ─"),
            "{frame}"
        );
    }
}

#[test]
fn scrolled_context_stays_on_selected_row_in_short_view() {
    let mut state = ui();
    for _ in 0..5 {
        state.key(Key::Down);
    }
    for width in [86, 109, 110, 119, 120, 137, 138, 140] {
        let frame = state.terminal_frame(width, 5);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 5, "{frame}");
        assert!(
            lines[4].starts_with(if width >= 110 {
                "  j/k: ↕pane"
            } else {
                "  Enter: focus"
            }),
            "{frame}"
        );
        assert!(lines[1].starts_with("  ─"), "{frame}");
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.ends_with("\x1b[0m")
                && line.contains("codex")
                && line.contains("Same")),
            "{frame}"
        );
        assert!(!frame.contains("p-other"), "{frame}");
    }
}

#[test]
fn long_names_clip_within_wide_table_without_losing_status() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["agents"][0]["display_agent"] = Value::from("A".repeat(200));
    for width in [86, 110, 140] {
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), width).unwrap());
        let frame = state.terminal_frame(width, 8);
        let line = frame
            .split("\r\n")
            .find(|line| selected_prefix(line))
            .unwrap();
        let plain = without_status_ansi(line)
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[0m", "")
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "");
        assert!(plain.starts_with("  > "), "{frame}");
        assert!(plain.contains('A'), "{frame}");
        assert!(plain.contains('…'), "{frame}");
        assert!(
            plain.chars().count() < width && plain.contains("working"),
            "{frame}"
        );
        assert!(!plain.contains("p-b"), "{frame}");
    }
}

#[test]
fn wide_scroll_repeats_context_on_selected_row_in_short_viewport() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let panes = snapshot["result"]["snapshot"]["panes"]
        .as_array_mut()
        .unwrap();
    for n in 0..12 {
        panes.push(serde_json::json!({"pane_id": format!("p-extra-{n}"), "workspace_id": "w1", "tab_id": "t1", "label": "filler"}));
    }
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    state.selected = Some("p-extra-11".into());
    let frame = state.terminal_frame(109, 5);
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert_eq!(lines.len(), 5, "{frame}");
    assert!(lines[1].starts_with("  ─"), "{frame}");
    assert!(
        lines
            .iter()
            .any(|line| selected_prefix(line) && line.contains("Same") && line.contains("filler")),
        "{frame}"
    );
    assert_eq!(state.selected.as_deref(), Some("p-extra-11"));
    state.selected = Some("p-second-tab".into());
    let other = state.terminal_frame(109, 5);
    assert!(
        other
            .lines()
            .any(|line| line.contains("Second") && line.contains("pane 1")),
        "{other}"
    );
    assert!(
        !other.contains("filler") && !other.contains("p-second-tab"),
        "{other}"
    );
}

#[test]
fn wide_scroll_shows_only_current_tab_context_without_group_rows() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["panes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"pane_id": "p-second-2", "workspace_id": "w1", "tab_id": "t3", "label": "Second pane"}));
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    for (id, height) in [("p-second-tab", 7), ("p-second-2", 8)] {
        state.selected = Some(id.into());
        let frame = state.terminal_frame(120, height);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert!(lines.len() <= height, "{frame}");
        assert!(lines[1].starts_with("  ─"), "{frame}");
        assert!(
            !frame.contains(" / T") && !frame.contains("p-second"),
            "{frame}"
        );
        assert!(
            frame.lines().any(|line| selected_prefix(line)
                && line.contains("Second")
                && line.contains("pane")),
            "{frame}"
        );
        assert_eq!(state.selected.as_deref(), Some(id));
    }
}

#[test]
fn terminal_footer_stays_on_bottom_row_for_short_content() {
    for width in [76, 120] {
        let mut state = ui();
        for height in [5, 16] {
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height, "{frame}");
            assert!(
                lines[height - 1].starts_with(if width >= 110 {
                    "  j/k: ↕pane"
                } else {
                    "  Enter: focus"
                }),
                "{frame}"
            );
            assert!(
                lines.iter().all(|line| without_status_ansi(line)
                    .replace("\x1b[48;5;236m", "")
                    .replace("\x1b[0m", "")
                    .replace("\x1b[2m", "")
                    .replace("\x1b[22m", "")
                    .chars()
                    .count()
                    < width),
                "{frame}"
            );
        }
        state.query = "not found".into();
        let empty = state.terminal_frame(width, 16);
        assert!(empty.contains("No matching panes"), "{empty}");
        assert_eq!(empty.split("\r\n").count(), 16, "{empty}");
        state.mode = Mode::Help;
        let help = state.terminal_frame(width, 16);
        assert_eq!(help.split("\r\n").count(), 16, "{help}");
        assert!(
            help.ends_with("  j/k/Up/Down: navigate · /: search · r: reset · Esc/q/?: close keys"),
            "{help}"
        );
    }
}

#[test]
fn help_shows_theme_shortcut_without_losing_keys_at_minimum_and_wide_sizes() {
    let mut state = ui();
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    for width in [80, 120] {
        let frame = state.terminal_frame(width, 15);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 15, "{frame}");
        assert!(
            state
                .layout(width, 15, false)
                .iter()
                .all(|line| line.chars().count() < width),
            "{frame}"
        );
        assert!(
            frame.contains("j/Down")
                && frame.contains("h/Left")
                && frame.contains("Tab/Shift+Tab")
                && frame.contains("next/previous workspace"),
            "{frame}"
        );
        assert!(
            frame.contains("Enter")
                && frame.contains("focus selected pane")
                && frame.contains("reset search"),
            "{frame}"
        );
        assert!(
            frame.ends_with("  j/k/Up/Down: navigate · /: search · r: reset · Esc/q/?: close keys"),
            "{frame}"
        );
        // The category spacer moves this shortcut below the initial 15-row viewport.
        for _ in 0..8 {
            state.key(Key::Down);
        }
        let scrolled = state.terminal_frame(width, 15);
        assert!(scrolled.contains("choose theme"), "{scrolled}");
        assert!(scrolled.contains("> t"), "{scrolled}");
        assert_eq!(scrolled.split("\r\n").count(), 15);
        assert!(
            scrolled
                .ends_with("  j/k/Up/Down: navigate · /: search · r: reset · Esc/q/?: close keys"),
            "{scrolled}"
        );
        for _ in 0..8 {
            state.key(Key::Up);
        }
        state.terminal_frame(width, 15);
    }
}

#[test]
fn search_hint_appears_in_navigation_with_empty_query_not_input() {
    let mut state = ui();
    for width in [76, 120] {
        assert_eq!(
            state.terminal_frame(width, 8).split("\r\n").next(),
            Some("  Search: / to search")
        );
    }
    state.key(Key::Char('/'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: ")
    );
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: codex")
    );
    state.key(Key::Enter);
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: codex")
    );
    state.key(Key::Char('r'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: / to search")
    );
    state.key(Key::Char('?'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: / to search")
    );
    state.key(Key::Char('/'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some("  Search: ")
    );
}

#[test]
fn down_moves_marker_until_viewport_bottom_before_scrolling() {
    for (width, height) in [(76, 5), (120, 6)] {
        let mut state = ui();
        state.terminal_frame(width, height);
        state.key(Key::Down);
        let moved = state.terminal_frame(width, height);
        let lines: Vec<_> = moved.split("\r\n").collect();
        assert!(
            lines.iter().any(|line| line.contains("Claude")) == (height > 5),
            "{moved}"
        );
        assert!(
            lines
                .iter()
                .any(|line| selected_prefix(line) && line.contains("Same")),
            "{moved}"
        );
    }
    for (width, height) in [(76, 10), (120, 11)] {
        let mut state = ui();
        let initial = state.terminal_frame(width, height);
        assert!(
            initial
                .split("\r\n")
                .any(|line| selected_prefix(line) && line.contains("Claude")),
            "{initial}"
        );
        state.key(Key::Down);
        let next = state.terminal_frame(width, height);
        let lines: Vec<_> = next.split("\r\n").collect();
        assert!(lines.iter().any(|line| line.contains("Claude")), "{next}");
        assert!(
            lines
                .iter()
                .any(|line| selected_prefix(line) && line.contains("Same")),
            "{next}"
        );
        state.key(Key::Down);
        let third = state.terminal_frame(width, height);
        let lines: Vec<_> = third.split("\r\n").collect();
        assert!(lines.iter().any(|line| line.contains("Claude")), "{third}");
        assert!(
            lines
                .iter()
                .any(|line| selected_prefix(line) && line.contains("pane 3")),
            "{third}"
        );
        state.key(Key::Down);
        let fourth = state.terminal_frame(width, height);
        let lines: Vec<_> = fourth.split("\r\n").collect();
        assert!(lines.iter().any(|line| line.contains("Claude")), "{fourth}");
        assert!(
            lines
                .iter()
                .any(|line| selected_prefix(line) && line.contains("pane 4")),
            "{fourth}"
        );
        for _ in 0..2 {
            state.key(Key::Down);
            state.terminal_frame(width, height);
        }
        let scrolled = state.terminal_frame(width, height);
        assert!(
            scrolled.contains("> ") && scrolled.contains("codex"),
            "{scrolled}"
        );
        assert!(
            !scrolled.contains("Claude") && !scrolled.contains("p-other"),
            "{scrolled}"
        );
        assert_eq!(scrolled.split("\r\n").count(), height, "{scrolled}");
    }
}

#[test]
fn moving_up_inside_scrolled_window_does_not_jump_to_first_row() {
    for (width, height) in [(76, 6), (120, 7)] {
        let mut state = ui();
        state.terminal_frame(width, height);
        for _ in 0..3 {
            state.key(Key::Down);
            state.terminal_frame(width, height);
        }
        let before = state.terminal_frame(width, height);
        assert!(!before.contains("Claude"), "{before}");
        state.key(Key::Up);
        let after = state.terminal_frame(width, height);
        assert!(!after.contains("Claude"), "{after}");
        assert!(after.contains("> ") && after.contains("pane 3"), "{after}");
        assert_eq!(after.split("\r\n").count(), height);
    }
}

#[test]
fn crossing_tab_keeps_visible_rows_until_viewport_is_full() {
    let mut state = ui();
    state.terminal_frame(120, 12);
    for _ in 0..4 {
        state.key(Key::Down);
        state.terminal_frame(120, 12);
    }
    let frame = state.terminal_frame(120, 12);
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert!(lines[1].starts_with("  ─"), "{frame}");
    assert!(
        frame.contains("Same") && frame.contains("Second"),
        "{frame}"
    );
    assert!(
        frame.contains("pane 4") && frame.contains("Same"),
        "{frame}"
    );
    assert!(
        frame.lines().any(|line| selected_prefix(line)
            && line.contains("Second")
            && line.contains("pane 1")),
        "{frame}"
    );
}

#[test]
fn scrolling_across_tab_keeps_last_old_pane_until_new_row_needs_room() {
    let mut state = ui();
    for (width, height) in [(76, 9), (120, 10)] {
        state.viewport_start.set(0);
        state.selected = Some("p-b".into());
        state.terminal_frame(width, height);
        for _ in 0..3 {
            state.key(Key::Down);
            state.terminal_frame(width, height);
        }
        assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
        state.key(Key::Down);
        let frame = state.terminal_frame(width, height);
        assert!(frame.contains("pane 4"), "{frame}");
        assert!(
            frame.lines().any(|line| selected_prefix(line)
                && line.contains("Second")
                && line.contains("pane 1")),
            "{frame}"
        );
        assert!(
            !frame.contains("p-second-tab") && !frame.contains(" / T"),
            "{frame}"
        );
        assert_eq!(frame.split("\r\n").count(), height, "{frame}");
        for _ in 0..4 {
            state.key(Key::Up);
            state.terminal_frame(width, height);
        }
        let top = state.terminal_frame(width, height);
        assert!(top.contains("> ") && top.contains("Claude"), "{top}");
        state.key(Key::Down);
        let again = state.terminal_frame(width, height);
        assert!(
            again.contains("Claude") && again.contains("Same"),
            "{again}"
        );
    }
}

#[test]
fn tall_filled_viewport_scrolls_one_pane_per_down_and_resets_after_up() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    for n in 0..12 {
        snapshot["result"]["snapshot"]["panes"].as_array_mut().unwrap().push(serde_json::json!({"pane_id": format!("p-extra-{n}"), "workspace_id": "w1", "tab_id": "t1", "label": "filler"}));
    }
    for (width, height) in [(76, 10), (120, 11)] {
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        state.terminal_frame(width, height);
        let mut previous_first = 0;
        for step in 1..13 {
            state.key(Key::Down);
            let frame = state.terminal_frame(width, height);
            let first = state.viewport_start.get();
            assert!(first <= previous_first + 1, "step {step}: {frame}");
            previous_first = first;
            assert!(
                frame.contains("> ")
                    && frame.contains("Same")
                    && frame.split("\r\n").count() == height
                    && !frame.contains("p-extra"),
                "{frame}"
            );
        }
        assert!(previous_first > 0);
        for _ in 0..12 {
            state.key(Key::Up);
            state.terminal_frame(width, height);
        }
        let top = state.terminal_frame(width, height);
        assert!(top.contains("> ") && top.contains("Claude"), "{top}");
        state.key(Key::Down);
        assert!(state.terminal_frame(width, height).contains("Claude"));
    }
}

#[test]
fn hidden_pane_match_reveals_name_and_tiny_width_keeps_marker() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["panes"][2]["label"] = Value::from("Hidden Needle");
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    state.query = "Needle".into();
    let frame = state.terminal_frame(120, 20);
    let plain = without_match_marks(&frame);
    assert!(
        plain.contains("Claude (Hidden Nee…")
            && frame.contains("working")
            && frame.lines().nth(2).unwrap().contains("Status"),
        "{frame}"
    );
    let compact_boundary = state.terminal_frame(109, 20);
    assert!(
        without_match_marks(&compact_boundary).contains("Claude (Hidden Needle)")
            && compact_boundary.contains("working"),
        "{compact_boundary}"
    );
    let compact = state.terminal_frame(76, 20);
    assert!(
        without_match_marks(&compact).contains("Claude (Hidde… \x1b[2m|\x1b[22m "),
        "{compact}"
    );
    let tiny = state.terminal_frame(16, 5);
    assert!(tiny.contains("> ") && !tiny.contains("p-b"), "{tiny}");
    assert!(
        tiny.split("\r\n").all(|line| line
            .replace("\x1b[48;5;236m", "")
            .replace("\x1b[0m", "")
            .replace("\x1b[2m", "")
            .replace("\x1b[22m", "")
            .replace("\x1b[4m", "")
            .replace("\x1b[24m", "")
            .chars()
            .count()
            <= 15),
        "{tiny}"
    );
}
