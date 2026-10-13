use crate::input::Key;
use crate::selection::Mode;
use ratatui::{buffer::CellWidth, style::Color};

use super::ui;

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
            let mut state = crate::test_support::ui();
            state.key(Key::Char('?'));
            state.help.query = query.into();
            let full = state.layout(width, height, false);
            let footer_len = state
                .hints(crate::keymap::Context::Help, width, 1, false)
                .len();
            let body = &full[3..full.len() - footer_len - 1];
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
            assert_eq!(lines[2], full[2]);
            let budget = state.hint_rows(crate::keymap::Context::Help, width, height);
            assert_eq!(
                lines[height - budget - 1],
                full[full.len() - footer_len - 1]
            );
            assert_eq!(
                lines[height - budget..],
                state.hints(crate::keymap::Context::Help, width, budget, true)
            );
            if headings.len() == 2 {
                let actions = lines.iter().position(|line| line == "  Actions").unwrap();
                assert!(lines[actions - 1].trim().is_empty(), "{lines:?}");
                assert!(!lines[actions - 2].trim().is_empty(), "{lines:?}");
            } else {
                // Remaining blank viewport rows are padding, not content spacers.
                assert!(
                    lines[3..3 + body.len()]
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
        let mut state = crate::test_support::ui();
        state.key(Key::Char('?'));
        let entry_count = state.help.entries.len();
        let budget = state.hint_rows(crate::keymap::Context::Help, width, height);
        for selected in 0..entry_count {
            let lines = state.layout(width, height, true);
            assert_eq!(state.help.selected, Some(selected));
            assert_eq!(lines.iter().filter(|line| line.contains("> ")).count(), 1);
            assert!(
                lines[3..height - budget - 1].iter().any(|line| {
                    line.contains("> ") && line.contains(state.help.entries[selected].meaning)
                }),
                "{lines:?}"
            );
            assert_eq!(lines[height - budget - 1], lines[2]);
            assert_eq!(
                lines[height - budget..],
                state.hints(crate::keymap::Context::Help, width, budget, true)
            );
            if selected + 1 < entry_count {
                state.key(Key::Down);
            }
        }
        let bottom = state.layout(width, height, true);
        assert_eq!(state.help.selected, Some(entry_count - 1));
        assert_eq!(
            state.help.viewport_start.get(),
            14usize.saturating_sub(height - 4 - budget)
        );
        if height == 15 {
            assert!(
                bottom[height - budget - 2].contains("close Crossroads"),
                "{bottom:?}"
            );
            state.help.viewport_start.set(9);
            state.help.selected = Some(6);
            state.key(Key::Up);
            // Exercise heading restoration with a smaller internal content budget.
            let lines = state.layout(width, 8, true);
            assert_eq!(lines[3], "  Actions", "{lines:?}");
            assert!(lines[4].contains("> Enter"), "{lines:?}");
            state.key(Key::Up);
            let lines = state.layout(width, 8, true);
            assert!(lines[3].contains("> → Tab | ← Shift+Tab"), "{lines:?}");
            assert!(lines[4].trim().is_empty(), "{lines:?}");
            assert_eq!(lines[5], "  Actions", "{lines:?}");
        }
        for _ in 0..state.help.selected.unwrap() {
            state.key(Key::Up);
            state.layout(width, height, true);
        }
        let top = state.layout(width, height, true);
        assert_eq!(state.help.selected, Some(0));
        assert_eq!(state.help.viewport_start.get(), 0);
        assert_eq!(top[3], "  Navigation");
        assert!(top[4].contains("> j/↓"));
    }
}

#[test]
fn help_fixed_chrome_scroll_alignment_and_theme_matches() {
    for theme_index in 0..crate::theme::NAMES.len() {
        let mut state = crate::test_support::ui();
        state.theme_index = theme_index;
        state.popup_size = Some((80, 15));
        state.key(Key::Char('?'));
        for (width, height) in [(80, 15), (109, 15), (110, 15), (162, 36)] {
            let first = state.layout(width, height, false);
            let meaning_columns = state
                .help
                .entries
                .iter()
                .map(|entry| {
                    let line = first
                        .iter()
                        .find(|line| {
                            line.get(4..).is_some_and(|s| s.starts_with(&entry.keys))
                                && line.ends_with(entry.meaning)
                        })
                        .unwrap();
                    usize::from(line.strip_suffix(entry.meaning).unwrap().cell_width())
                })
                .collect::<Vec<_>>();
            assert!(
                meaning_columns.iter().all(|&column| column == 25),
                "{first:?}: {meaning_columns:?}"
            );
            for _ in 1..state.help.entries.len() {
                let lines = state.layout(width, height, true);
                assert_eq!(lines.len(), height, "{lines:?}");
                assert_eq!(
                    lines[0],
                    format!("  Search: / to search{}Help", " ".repeat(width - 27))
                );
                assert_eq!(lines[1], "  Keys — Select");
                assert_eq!(lines[2], "  ".to_owned() + &"─".repeat(width - 4));
                let budget = state.hint_rows(crate::keymap::Context::Help, width, height);
                assert_eq!(lines[height - budget - 1], lines[2]);
                assert_eq!(
                    lines[height - budget..],
                    state.hints(crate::keymap::Context::Help, width, budget, true)
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
            for _ in 1..state.help.entries.len() {
                state.key(Key::Up);
                state.layout(width, height, true);
            }
            assert!(state
                .layout(width, height, true)
                .iter()
                .any(|line| line.contains("next pane")));
            assert_eq!(state.layout(width, height, true)[3], "  Navigation");
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
    let mut state = crate::test_support::ui();
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
    assert_eq!(lines[3], "  Navigation");
    assert!(lines.iter().any(|line| line == "  Actions"));
    state.help.query = "?".into();
    let lines = state.layout(80, 15, true);
    let show = lines
        .iter()
        .find(|line| line.contains("show keys"))
        .unwrap();
    assert!(show.contains(&format!("{accent}?{end}")), "{show}");
    assert_eq!(show.matches("\x1b[4m").count(), 1);
    assert_eq!(lines[3], "  Actions");
}
#[test]
fn directional_help_labels_keep_meaning_columns_aligned() {
    use ratatui::buffer::CellWidth;
    let mut state = crate::test_support::ui();
    state.key(crate::input::Key::Char('?'));
    let lines = state.layout(80, 15, false);
    let positions: Vec<_> = state
        .help
        .entries
        .iter()
        .map(|entry| {
            let line = lines
                .iter()
                .find(|line| {
                    (line.starts_with("    ") || line.starts_with("  > "))
                        && line.ends_with(entry.meaning)
                })
                .unwrap();
            line.strip_suffix(entry.meaning).unwrap().cell_width()
        })
        .collect();
    assert!(
        positions.iter().all(|column| *column == positions[0]),
        "{positions:?}: {lines:?}"
    );
}

fn assert_help_display_query_filters_and_highlights(query: char, custom: bool, width: usize) {
    use crate::keymap::Keymap;
    let mut state = crate::test_support::ui();
    if custom {
        let aliases = ["A", "Z"];
        let (map, errors) = Keymap::parse(
            &serde_json::json!({"select": {
                "next_row": ["/", "|"],
                "next_workspace": aliases,
                "open_search": ["s"]
            }})
            .to_string(),
        );
        assert!(errors.is_empty(), "{errors:?}");
        state.keymap = map;
    }
    state.key(Key::Char('?'));
    state.key(Key::Char('/'));
    state.key(Key::Char(query));
    let expected = match (custom, query) {
        (true, '|') => vec![0, 4],
        (true, '\'') => vec![0],
        (false, '/') => vec![0, 1, 2, 3, 4, 6, 10],
        (_, '→') => vec![3, 4],
        (_, '←') => vec![2, 4],
        (_, '|') => vec![4],
        _ => unreachable!(),
    };
    assert_eq!(state.help.visible(), expected, "custom={custom}, {query}");
    assert_eq!(state.help.selected_index(&expected), Some(expected[0]));
    let lines = state.layout(width, 15, true);
    let body = lines[2..lines.len() - 2].join("\n");
    assert!(!body.contains("No matching keys"), "{lines:?}");
    let highlighted = format!("\x1b[4m{query}\x1b[24m");
    assert_eq!(
        body.matches(&highlighted).count(),
        expected.len(),
        "custom={custom}, width={width}, {query}: {lines:?}"
    );
    let plain_body = body
        .split('\x1b')
        .enumerate()
        .map(|(at, part)| {
            if at == 0 {
                part
            } else {
                part.split_once('m').unwrap().1
            }
        })
        .collect::<String>();
    let plain_body = plain_body.split_whitespace().collect::<Vec<_>>().join(" ");
    let positions: Vec<_> = expected
        .iter()
        .map(|&i| {
            // Full meanings remain distinct even when several matches end in
            // "pane"; normalize styling and wrapped whitespace, not the labels.
            plain_body.find(state.help.entries[i].meaning).unwrap()
        })
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    if custom {
        assert_eq!(body.contains("Z"), expected.contains(&4), "{lines:?}");
        assert_eq!(
            body.contains(": "),
            width < 80,
            "normal/wrapped fixture: {lines:?}"
        );
    }
    state.key(Key::Enter);
    for &entry in expected.iter().skip(1) {
        state.key(Key::Down);
        assert_eq!(state.help.selected, Some(entry));
    }
    state.key(Key::Down); // Wrap to the first visible entry, including a single match.
    assert_eq!(state.help.selected, Some(expected[0]));
    state.key(Key::Up);
    assert_eq!(state.help.selected, expected.last().copied());
    for &entry in expected.iter().rev().skip(1) {
        state.key(Key::Up);
        assert_eq!(state.help.selected, Some(entry));
    }
    state.key(Key::Up);
    assert_eq!(state.help.selected, expected.last().copied());
    state.key(Key::Char('r'));
    assert!(state.help.query.is_empty());
    assert_eq!(state.help.visible(), (0..11).collect::<Vec<_>>());
    assert_eq!(state.help.selected, expected.last().copied());
}

#[test]
fn help_display_query_pipe_filters_and_highlights() {
    assert_help_display_query_filters_and_highlights('|', false, 80);
}

#[test]
fn help_display_query_right_arrow_filters_and_highlights() {
    assert_help_display_query_filters_and_highlights('→', false, 80);
}

#[test]
fn help_display_query_left_arrow_filters_and_highlights() {
    assert_help_display_query_filters_and_highlights('←', false, 80);
}

#[test]
fn help_display_query_single_slash_filters_and_highlights() {
    assert_help_display_query_filters_and_highlights('/', false, 80);
}

#[test]
fn help_display_query_custom_labels_filter_and_highlight() {
    for query in ['|', '→', '←', '\''] {
        assert_help_display_query_filters_and_highlights(query, true, 240);
    }
}

#[test]
fn help_display_query_wrapped_custom_labels_filter_and_highlight() {
    for query in ['|', '→', '←', '\''] {
        assert_help_display_query_filters_and_highlights(query, true, 50);
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
            frame.contains("j/↓")
                && frame.contains("h/←")
                && frame.contains("→ Tab | ← Shift+Tab")
                && frame.contains("next/previous workspace"),
            "{frame}"
        );
        assert!(
            frame.contains("Enter") && frame.contains("focus selected pane"),
            "{frame}"
        );
        assert!(
            frame.ends_with(
                &state
                    .hints(
                        crate::keymap::Context::Help,
                        width,
                        state.hint_rows(crate::keymap::Context::Help, width, 15),
                        true
                    )
                    .join("\r\n")
            ),
            "{frame}"
        );
        // The category spacer moves this shortcut below the initial 15-row viewport.
        for _ in 0..8 {
            state.key(Key::Down);
        }
        let scrolled = state.terminal_frame(width, 15);
        assert!(scrolled.contains("reset search"), "{scrolled}");
        assert!(scrolled.contains("choose theme"), "{scrolled}");
        assert!(scrolled.contains("> t"), "{scrolled}");
        assert_eq!(scrolled.split("\r\n").count(), 15);
        assert!(
            scrolled.ends_with(
                &state
                    .hints(
                        crate::keymap::Context::Help,
                        width,
                        state.hint_rows(crate::keymap::Context::Help, width, 15),
                        true
                    )
                    .join("\r\n")
            ),
            "{scrolled}"
        );
        for _ in 0..8 {
            state.key(Key::Up);
        }
        state.terminal_frame(width, 15);
    }
}
