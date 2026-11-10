use ratatui::buffer::CellWidth;

use crate::input::{Key, Mouse, MouseButton};
use crate::keymap::{Action, Keymap};
use crate::selection::{Hit, Mode, Ui};
use crate::test_support::ui;

fn render(state: &Ui) -> Vec<String> {
    let (w, h) = state.popup_size.unwrap();
    state.layout(w, h, true)
}

fn location(state: &Ui, target: &Hit) -> (usize, usize) {
    state
        .mouse
        .borrow()
        .hits
        .iter()
        .find_map(|(x, y, _, hit)| (hit == target).then_some((*x, *y)))
        .unwrap_or_else(|| panic!("no hit for {target:?}"))
}

fn click(state: &mut Ui, at: (usize, usize)) -> Option<String> {
    let result = state.mouse_input(Mouse {
        button: MouseButton::Left,
        x: at.0,
        y: at.1,
    });
    render(state);
    result
}

fn interactive() -> Ui {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    render(&state);
    state
}

fn hover(state: &mut Ui, at: (usize, usize)) {
    assert_eq!(
        state.key(Key::Mouse(Mouse {
            button: MouseButton::Motion,
            x: at.0,
            y: at.1
        })),
        None
    );
    render(state);
    assert!(!state.closed);
}

#[test]
fn mouse_hover_highlights_without_activation_in_every_view_and_search() {
    for mode in [Mode::Select, Mode::Search, Mode::Help, Mode::Themes] {
        for searching in [false, true] {
            let mut state = interactive();
            state.popup_size = Some((120, 30));
            state.mode = mode;
            state.help.searching = mode == Mode::Help && searching;
            state.theme_table_preview = mode == Mode::Themes && searching;
            state.query = "Same".into();
            if state.help.searching {
                state.help.query = "pane".into();
            }
            render(&state);
            let target = match mode {
                Mode::Help => Hit::Help(1),
                Mode::Themes if !searching => Hit::Theme(1),
                _ => Hit::Pane("p-a".into()),
            };
            let at = location(&state, &target);
            hover(&mut state, at);
            match target {
                Hit::Help(i) => assert_eq!(state.help.selected, Some(i)),
                Hit::Theme(i) => assert_eq!(state.theme_cursor, i),
                Hit::Pane(id) => assert_eq!(state.selected.as_deref(), Some(id.as_str())),
                _ => unreachable!(),
            }
            assert_eq!(state.mode, mode);
            assert_eq!(state.help.searching, mode == Mode::Help && searching);
            assert_eq!(state.query, "Same");
            assert_eq!(
                state.help.query,
                if state.help.searching { "pane" } else { "" }
            );
            assert_eq!(state.theme_index, 0);
            assert!(state.theme_error.is_none());
            for target in [Hit::Search, Hit::Action(Action::Close)] {
                let at = state
                    .mouse
                    .borrow()
                    .hits
                    .iter()
                    .find_map(|(x, y, _, hit)| (*hit == target).then_some((*x, *y)));
                if let Some(at) = at {
                    hover(&mut state, at);
                }
                assert_eq!(state.mode, mode);
            }
        }
    }
}

#[test]
fn fixed_mouse_help_rows_are_informational_on_click_and_enter() {
    for themes in [false, true] {
        let mut state = interactive();
        if themes {
            state.key(Key::Char('t'));
        }
        state.key(Key::Char('?'));
        let fixed: Vec<_> = state
            .help
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| entry.action.is_none().then_some(i))
            .collect();
        assert_eq!(fixed.len(), 3);
        for i in fixed {
            state.help.selected = Some(i);
            render(&state);
            let at = location(&state, &Hit::Help(i));
            assert_eq!(click(&mut state, at), None);
            assert_eq!(state.key(Key::Enter), None);
            assert_eq!(state.mode, Mode::Help);
            assert_eq!(state.help.selected, Some(i));
            assert_eq!(state.theme_cursor, 0);
            assert!(!state.closed);
        }
    }
}

fn search_footer(context: Mode) -> (Ui, String, &'static str) {
    let mut state = interactive();
    let return_label = match context {
        Mode::Select => "Enter/Esc: select",
        Mode::Help => {
            state.key(Key::Char('?'));
            "Enter/Esc: return to keys"
        }
        Mode::Themes => {
            state.key(Key::Char('t'));
            state.key(Key::Char(' '));
            render(&state);
            click(&mut state, (4, 1));
            "Enter/Esc: preview"
        }
        Mode::Search => unreachable!(),
    };
    if context != Mode::Themes {
        state.key(Key::Char('f'));
    }
    state.key(Key::Char('x'));
    state.key(Key::Char('y'));
    let line = render(&state).pop().unwrap();
    (state, line, return_label)
}

fn footer_hit(state: &Ui, x: usize) -> Option<Hit> {
    let y = state.popup_size.unwrap().1 - 1;
    state
        .mouse
        .borrow()
        .hits
        .iter()
        .rev()
        .find_map(|(left, row, width, hit)| {
            (*row == y && x >= *left && x - left < *width).then(|| hit.clone())
        })
}

#[test]
fn mouse_search_backspace_hits_exact_rendered_label_cells() {
    for context in [Mode::Select, Mode::Help, Mode::Themes] {
        let (state, line, _) = search_footer(context);
        let byte = line.find("Backspace: erase").unwrap();
        let start = line[..byte].cell_width() as usize;
        let end = start + "Backspace: erase".cell_width() as usize;
        for x in [start, end - 1] {
            assert_eq!(
                footer_hit(&state, x),
                Some(Hit::Key(Key::Backspace)),
                "{context:?}: x={x}, {line:?}"
            );
            let (mut state, _, _) = search_footer(context);
            click(&mut state, (x, 14));
            assert_eq!(
                if context == Mode::Help {
                    &state.help.query
                } else {
                    &state.query
                },
                "x"
            );
            assert!(state.mode == Mode::Search || state.help.searching);
        }
        for x in [start - 1, end] {
            assert_eq!(
                footer_hit(&state, x),
                None,
                "adjacent separator/blank must miss: {context:?} x={x}"
            );
        }
    }
}

#[test]
fn mouse_search_return_hits_only_its_label_not_separator_cells() {
    for context in [Mode::Select, Mode::Help, Mode::Themes] {
        let (state, line, label) = search_footer(context);
        let badge = match context {
            Mode::Select => "MODE: SELECT",
            Mode::Help => "MODE: HELP",
            Mode::Themes => "MODE: THEMES",
            Mode::Search => unreachable!(),
        };
        assert!(render(&state)[0].ends_with(&format!("\x1b[7m{badge}\x1b[27m")));
        assert!(state.mode == Mode::Search || state.help.searching);
        let start = line[..line.find(label).unwrap()].cell_width() as usize;
        let end = start + label.cell_width() as usize;
        let erase = line[..line.find("Backspace").unwrap()].cell_width() as usize;
        for x in start..end {
            assert_eq!(footer_hit(&state, x), Some(Hit::Key(Key::Enter)));
        }
        for x in end..erase {
            assert_eq!(
                footer_hit(&state, x),
                None,
                "return region overruns its label: {context:?} x={x}"
            );
        }
        for x in [start, end - 1] {
            let (mut state, _, _) = search_footer(context);
            click(&mut state, (x, 14));
            assert_eq!(state.mode, context);
            assert!(!state.help.searching);
            assert_eq!(
                if context == Mode::Help {
                    &state.help.query
                } else {
                    &state.query
                },
                "xy"
            );
        }
    }
}

#[test]
fn mouse_footer_preserves_special_and_literal_arrow_action_identity() {
    use crate::keymap::Context;

    for width in [80, 240] {
        for (mode, preview, context, special, literal, units) in [
            (
                Mode::Select,
                false,
                Context::Select,
                Action::NextRow,
                Action::PreviousTab,
                [
                    ("↓: ↓pane", Action::NextRow),
                    ("↓: ↑tab", Action::PreviousTab),
                ],
            ),
            (
                Mode::Help,
                false,
                Context::Help,
                Action::NextCategory,
                Action::PreviousCategory,
                [
                    ("↓: ↓move", Action::NextCategory),
                    ("↓: ↑move", Action::PreviousCategory),
                ],
            ),
            (
                Mode::Themes,
                false,
                Context::Themes,
                Action::NextTheme,
                Action::PreviousTheme,
                [
                    ("↓: ↓theme", Action::NextTheme),
                    ("↓: ↑theme", Action::PreviousTheme),
                ],
            ),
            (
                Mode::Themes,
                true,
                Context::Themes,
                Action::NextTheme,
                Action::PreviousTheme,
                [
                    ("↓: ↓theme", Action::NextTheme),
                    ("↓: ↑theme", Action::PreviousTheme),
                ],
            ),
        ] {
            let prepare = || {
                let mut state = interactive();
                let (map, errors) = Keymap::parse(
                    r#"{"select":{"next_row":["Down"],"previous_tab":["↓"]},"help":{"next_category":["Down"],"previous_category":["↓"]},"themes":{"next_theme":["Down"],"previous_theme":["↓"]}}"#,
                );
                assert!(errors.is_empty(), "{errors:?}");
                assert_eq!(map.action(context, Key::Down), Some(special));
                assert_eq!(map.action(context, Key::Char('↓')), Some(literal));
                state.keymap = map;
                state.popup_size = Some((width, 15));
                if mode == Mode::Help {
                    state.key(Key::Char('?'));
                } else if mode == Mode::Themes {
                    state.key(Key::Char('t'));
                    state.theme_table_preview = preview;
                }
                state
            };
            for (label, action) in units {
                let mut state = prepare();
                let lines = render(&state);
                let (y, line) = lines
                    .iter()
                    .enumerate()
                    .find(|(_, line)| line.contains(label))
                    .unwrap();
                let start = line[..line.find(label).unwrap()].cell_width() as usize;
                let end = start + label.cell_width() as usize;
                for x in start - 1..=end {
                    let hit = state.mouse.borrow().hits.iter().rev().find_map(
                        |(left, row, width, hit)| {
                            (*row == y && x >= *left && x - left < *width).then(|| hit.clone())
                        },
                    );
                    assert_eq!(
                        hit,
                        (start..end).contains(&x).then_some(Hit::Action(action)),
                        "{context:?} preview={preview} width={width} x={x}: {line}"
                    );
                }
                let mut expected = prepare();
                let key = if action == special {
                    Key::Down
                } else {
                    Key::Char('↓')
                };
                assert_eq!(click(&mut state, (start, y)), expected.key(key));
                assert_eq!(
                    (
                        state.selected,
                        state.help.selected,
                        state.theme_cursor,
                        state.mode
                    ),
                    (
                        expected.selected,
                        expected.help.selected,
                        expected.theme_cursor,
                        expected.mode
                    )
                );
            }
        }
    }
}

#[test]
fn mouse_help_footer_custom_wide_key_uses_rendered_cell_extent() {
    let mut state = interactive();
    let (map, errors) = Keymap::parse(r#"{"help":{"next_category":["界"]}}"#);
    assert!(errors.is_empty());
    state.keymap = map;
    state.key(Key::Char('?'));
    let line = render(&state).pop().unwrap();
    let label = "界: ↓move";
    let start = line[..line.find(label).unwrap()].cell_width() as usize;
    let end = start + label.cell_width() as usize;
    for x in [start, start + 1, end - 1] {
        assert_eq!(
            footer_hit(&state, x),
            Some(Hit::Action(Action::NextCategory))
        );
    }
    for x in [start - 1, end, end + 1, end + 2] {
        assert_eq!(footer_hit(&state, x), None);
    }
}

#[test]
fn mouse_search_bar_opens_pane_and_independent_help_search() {
    let mut state = interactive();
    assert_eq!(click(&mut state, (4, 0)), None);
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Char('x'));
    state.key(Key::Enter);
    state.key(Key::Char('?'));
    render(&state);
    click(&mut state, (4, 0));
    assert!(state.help.searching);
    state.key(Key::Char('y'));
    assert_eq!(state.query, "x");
    assert_eq!(state.help.query, "y");
}

#[test]
fn mouse_theme_preview_search_returns_to_pending_preview_without_saving() {
    let mut state = interactive();
    state.key(Key::Char('t'));
    state.key(Key::Char('j'));
    state.key(Key::Char(' '));
    render(&state);
    click(&mut state, (4, 1));
    assert_eq!(state.mode, Mode::Search);
    assert!(render(&state)[0].contains("MODE: THEMES"));
    state.key(Key::Char('c'));
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    assert_eq!(state.theme_cursor, 1);
    assert_eq!(state.theme_index, 0);
    assert_eq!(state.query, "c");
}

#[test]
fn search_tail_tracks_typing_backspace_and_return_without_changing_the_query() {
    for context in [Mode::Select, Mode::Help, Mode::Themes] {
        let (mut state, _, _) = search_footer(context);
        let (_, available) = state.search_prompt_geometry(80);
        let budget = available - 9;
        for _ in 0..100 {
            state.key(Key::Char('a'));
        }
        for (key, suffix, expected_query) in [
            (
                Key::Char('Z'),
                format!("{}Z", "a".repeat(budget - 1)),
                format!("xy{}Z", "a".repeat(100)),
            ),
            (
                Key::Backspace,
                "a".repeat(budget),
                format!("xy{}", "a".repeat(100)),
            ),
            (
                Key::Char('界'),
                format!("{}界", "a".repeat(budget - 2)),
                format!("xy{}界", "a".repeat(100)),
            ),
            (
                Key::Backspace,
                "a".repeat(budget),
                format!("xy{}", "a".repeat(100)),
            ),
        ] {
            state.key(key);
            let query = if context == Mode::Help {
                &state.help.query
            } else {
                &state.query
            };
            assert_eq!(query, &expected_query);
            assert_eq!(state.search_text(query, 80), format!("Search: {suffix}"));
            assert!(render(&state)[0].contains(&format!("Search: {suffix} ")));
            assert!(state.mode == Mode::Search || state.help.searching);
        }
        state.key(Key::Enter);
        assert_eq!(state.mode, context);
        assert!(!state.help.searching);
        if context != Mode::Themes {
            state.key(Key::Char('r'));
            assert!(render(&state)[0].contains("Search: f to search"));
        }
    }
}

#[test]
fn mouse_theme_single_click_immediately_saves_clicked_theme() {
    let mut state = interactive();
    state.key(Key::Char('t'));
    render(&state);
    let at = location(&state, &Hit::Theme(3));
    click(&mut state, at);
    assert_eq!(state.theme_cursor, 3);
    assert!(
        state.theme_error.is_some(),
        "save must run (no config directory is available in this unit fixture)"
    );
}

#[test]
fn mouse_theme_selection_reuses_auto_notice_and_actions_clear_save_errors() {
    let mut state = interactive();
    state.set_auto_choice("catppuccin", Some("custom".into()));
    state.key(Key::Char('t'));
    state.key(Key::Char('j'));
    render(&state);
    let at = location(&state, &Hit::Theme(0));
    hover(&mut state, at);
    assert!(
        state.auto_toast.is_some(),
        "selecting Auto must retain its existing unsupported-theme notice"
    );
    state.theme_error = Some("save failed".into());
    render(&state);
    let at = location(&state, &Hit::Action(Action::NextTheme));
    click(&mut state, at);
    assert!(state.theme_error.is_none());
}

#[test]
fn mouse_help_single_click_executes_in_origin() {
    let mut state = interactive();
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Char('?'));
    render(&state);
    let at = location(&state, &Hit::Help(0));
    click(&mut state, at);
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    assert_eq!(state.theme_cursor, 1);
    assert_eq!(state.theme_index, 0);
}

#[test]
fn mouse_help_single_click_runs_filtered_action_during_independent_search() {
    for themes in [false, true] {
        let mut state = interactive();
        state.query = "Same".into();
        if themes {
            state.key(Key::Char('t'));
        }
        state.key(Key::Char('?'));
        state.key(Key::Char('f'));
        for c in if themes { "next theme" } else { "next pane" }.chars() {
            state.key(Key::Char(c));
        }
        render(&state);
        let at = location(&state, &Hit::Help(0));
        let before = state.selected.clone();
        assert_eq!(click(&mut state, at), None);
        assert_eq!(state.mode, if themes { Mode::Themes } else { Mode::Select });
        assert_eq!(state.query, "Same");
        assert_eq!(state.theme_index, 0);
        if themes {
            assert_eq!(state.theme_cursor, 1);
        } else {
            assert_ne!(state.selected, before);
        }
        assert!(!state.closed);
    }
}

#[test]
fn mouse_help_and_enter_dispatch_identity_not_custom_physical_keys() {
    let mut state = interactive();
    let (map, errors) = Keymap::parse(r#"{"select":{"next_row":["J"],"focus_pane":["F"]}}"#);
    assert!(errors.is_empty());
    state.keymap = map;
    state.key(Key::Char('?'));
    render(&state);
    let before = state.selected.clone();
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Select);
    assert_ne!(state.selected, before);
    assert_eq!(
        state
            .keymap
            .labels(crate::keymap::Context::Select, "next_row"),
        "J"
    );
    state.key(Key::Char('?'));
    state.help.selected = state
        .help
        .entries
        .iter()
        .position(|e| e.action == Some(Action::FocusPane));
    assert_eq!(state.key(Key::Enter), state.selected);
    assert!(state.closed);
}

#[test]
fn help_enter_matches_every_origin_action_including_close_reopen_save_and_preview() {
    for themes in [false, true] {
        for preview in [false, true] {
            let mut catalogue = interactive();
            if themes {
                catalogue.key(Key::Char('t'));
            }
            catalogue.key(Key::Char('?'));
            let actions: Vec<_> = catalogue
                .help
                .entries
                .iter()
                .filter_map(|e| e.action)
                .collect();
            for (index, action) in actions.into_iter().enumerate() {
                let prepare = || {
                    let mut state = interactive();
                    state.query = "Same".into();
                    state.selected = Some("p-a".into());
                    state.viewport_start.set(2);
                    if themes {
                        state.key(Key::Char('t'));
                        state.theme_cursor = 2;
                        state.theme_table_preview = preview;
                    }
                    state
                };
                let mut expected = prepare();
                let context = if themes {
                    crate::keymap::Context::Themes
                } else {
                    crate::keymap::Context::Select
                };
                let key = expected
                    .keymap
                    .bindings
                    .iter()
                    .find(|b| b.context == context && b.action == action)
                    .unwrap()
                    .keys[0];
                let expected_result = expected.key(key);
                let mut actual = prepare();
                actual.key(Key::Char('?'));
                actual.help.selected = Some(index);
                assert_eq!(actual.key(Key::Enter), expected_result, "{action:?}");
                assert_eq!(
                    (
                        actual.mode,
                        actual.closed,
                        actual.selected,
                        actual.query,
                        actual.theme_cursor,
                        actual.theme_index,
                        actual.theme_table_preview,
                        actual.viewport_start.get(),
                        actual.theme_error
                    ),
                    (
                        expected.mode,
                        expected.closed,
                        expected.selected,
                        expected.query,
                        expected.theme_cursor,
                        expected.theme_index,
                        expected.theme_table_preview,
                        expected.viewport_start.get(),
                        expected.theme_error
                    ),
                    "{action:?}"
                );
            }
        }
    }
}

#[test]
fn mouse_pane_single_click_activates_clicked_row_in_select_search_and_preview() {
    for mode in [Mode::Select, Mode::Search, Mode::Themes] {
        let mut state = interactive();
        state.popup_size = Some((120, 30));
        state.mode = mode;
        state.theme_table_preview = mode == Mode::Themes;
        render(&state);
        let at = location(&state, &Hit::Pane("p-a".into()));
        assert_eq!(click(&mut state, at).as_deref(), Some("p-a"));
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert!(state.closed);
    }
}

#[test]
fn mouse_help_documents_fixed_controls_separately() {
    for themes in [false, true] {
        let mut state = interactive();
        if themes {
            state.key(Key::Char('t'));
        }
        state.key(Key::Char('?'));
        for label in ["Hover", "Left click", "Wheel"] {
            assert!(state
                .help
                .entries
                .iter()
                .any(|entry| entry.category == "Mouse (fixed)" && entry.keys == label));
        }
    }
}

#[test]
fn mouse_hits_follow_filtered_scrolled_rows_not_header_separators_or_blank_space() {
    let mut state = interactive();
    state.query = "codex".into();
    let lines = render(&state);
    let at = location(&state, &Hit::Pane("p-other".into()));
    assert!(lines[at.1].contains("codex"));
    assert!(!state
        .mouse
        .borrow()
        .hits
        .iter()
        .any(|(_, _, _, hit)| *hit == Hit::Pane("p-a".into())));
    click(&mut state, (0, at.1));
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    hover(&mut state, at);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.query.clear();
    for n in 0..40 {
        let mut row = crate::topology::rows(crate::test_support::SNAPSHOT, Some("p-self"), 80)
            .unwrap()
            .remove(0);
        row.id = format!("scroll-{n}");
        row.agent = format!("界-{n}");
        state.rows.push(row);
    }
    state.selected = Some("scroll-39".into());
    let lines = render(&state);
    assert!(state.viewport_start.get() > 0);
    let at = location(&state, &Hit::Pane("scroll-38".into()));
    assert!(lines[at.1].contains("界-38"));
    hover(&mut state, at);
    assert_eq!(state.selected.as_deref(), Some("scroll-38"));
    let selected = state.selected.clone();
    for at in [(3, 1), (3, 2), (3, 3), (79, 6), (1, 12)] {
        click(&mut state, at);
        assert_eq!(state.selected, selected);
        assert!(!state.closed);
    }
}

#[test]
fn mouse_wheel_preserves_selection_even_with_rebindings() {
    let mut state = interactive();
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["J"],"previous_row":["K"]},"help":{"next_category":["J"]},"themes":{"next_theme":["J"]}}"#,
    );
    assert!(errors.is_empty());
    state.keymap = map;
    let before = state.selected.clone();
    state.mouse_input(Mouse {
        button: MouseButton::WheelDown,
        x: 5,
        y: 5,
    });
    assert_eq!(state.selected, before);
    render(&state);
    state.key(Key::Char('f'));
    render(&state);
    state.mouse_input(Mouse {
        button: MouseButton::WheelDown,
        x: 5,
        y: 5,
    });
    assert_eq!(state.query, "");
    state.key(Key::Enter);
    state.key(Key::Char('?'));
    render(&state);
    state.mouse_input(Mouse {
        button: MouseButton::WheelDown,
        x: 5,
        y: 5,
    });
    assert_eq!(state.help.selected, Some(0));
    state.key(Key::Char('q'));
    state.key(Key::Char('t'));
    render(&state);
    state.mouse_input(Mouse {
        button: MouseButton::WheelDown,
        x: 5,
        y: 5,
    });
    assert_eq!(state.theme_cursor, 0);
}

fn wheel(state: &mut Ui, button: MouseButton) {
    render(state);
    assert_eq!(state.key(Key::Mouse(Mouse { button, x: 5, y: 5 })), None);
    render(state);
    assert!(!state.closed);
}

#[test]
fn mouse_wheel_scrolls_overflow_clamps_and_keyboard_resumes_selection_following() {
    for mode in [Mode::Select, Mode::Search, Mode::Help, Mode::Themes] {
        for preview in [false, true] {
            let mut state = interactive();
            state.rows = (0..40)
                .map(|n| {
                    let mut row = ui().rows.remove(0);
                    row.id = format!("scroll-{n}");
                    row.agent = format!("pane-{n}");
                    row
                })
                .collect();
            state.selected = Some("scroll-0".into());
            if mode == Mode::Help {
                state.key(Key::Char('?'));
                if preview {
                    // Force wrapped Help entries, including a multi-line selected entry.
                    state.help.entries[0].keys = "Long binding ".repeat(90);
                }
            } else if mode == Mode::Themes {
                state.key(Key::Char('t'));
                state.key(Key::Down);
                state.theme_table_preview = preview;
            } else if mode == Mode::Search {
                if preview {
                    state.key(Key::Char('t'));
                    state.key(Key::Down);
                    state.theme_table_preview = true;
                    render(&state);
                    click(&mut state, (4, 1));
                } else {
                    state.key(Key::Char('f'));
                }
                state.query = "pane".into();
            }
            let selected = state.selected.clone();
            let help_selected = state.help.selected;
            let theme_cursor = state.theme_cursor;
            render(&state);
            let start = if mode == Mode::Help {
                state.help.viewport_start.get()
            } else if mode == Mode::Themes && !preview {
                state.theme_viewport_start.get()
            } else {
                state.viewport_start.get()
            };
            wheel(&mut state, MouseButton::WheelDown);
            let offset = || {
                if mode == Mode::Help {
                    state.help.viewport_start.get()
                } else if mode == Mode::Themes && !preview {
                    state.theme_viewport_start.get()
                } else {
                    state.viewport_start.get()
                }
            };
            assert_eq!(offset(), start + 1, "{mode:?} preview={preview}");
            let frame = render(&state);
            assert_eq!(render(&state), frame, "render must not restore selection");
            for _ in 0..100 {
                wheel(&mut state, MouseButton::WheelDown);
            }
            let bottom = render(&state);
            wheel(&mut state, MouseButton::WheelDown);
            assert_eq!(render(&state), bottom, "bottom must clamp");
            assert_eq!(state.selected, selected);
            assert_eq!(state.help.selected, help_selected);
            assert_eq!(state.theme_cursor, theme_cursor);
            assert_eq!(state.theme_index, 0);
            if mode == Mode::Search {
                assert_eq!(state.query, "pane");
                assert_eq!(state.mode, Mode::Search);
                state.key(Key::Enter);
                if preview {
                    state.key(Key::Esc);
                }
            }
            state.key(Key::Down);
            render(&state);
            let target = match mode {
                Mode::Help => Hit::Help(state.help.selected.unwrap()),
                Mode::Themes if !preview => Hit::Theme(state.theme_cursor),
                _ => Hit::Pane(state.selected.clone().unwrap()),
            };
            // In table preview keyboard still navigates themes, bringing the selected pane back.
            location(&state, &target);
            for _ in 0..100 {
                wheel(&mut state, MouseButton::WheelUp);
            }
            assert_eq!(
                if mode == Mode::Help {
                    state.help.viewport_start.get()
                } else if mode == Mode::Themes && !preview {
                    state.theme_viewport_start.get()
                } else {
                    state.viewport_start.get()
                },
                0
            );
            let top = render(&state);
            wheel(&mut state, MouseButton::WheelUp);
            assert_eq!(render(&state), top, "top must clamp");
            // Hover must use the newly scrolled layout, not the pre-wheel row mapping.
            let hit = state
                .mouse
                .borrow()
                .hits
                .iter()
                .find(|(_, _, _, hit)| matches!(hit, Hit::Pane(_) | Hit::Help(_) | Hit::Theme(_)))
                .cloned()
                .unwrap();
            hover(&mut state, (hit.0, hit.1));
            match hit.3 {
                Hit::Pane(id) => assert_eq!(state.selected, Some(id)),
                Hit::Help(index) => assert_eq!(state.help.selected, Some(index)),
                Hit::Theme(index) => assert_eq!(state.theme_cursor, index),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn mouse_wheel_after_taller_resize_uses_reconciled_keyboard_viewport() {
    for (count, height, grouped, initial_start, resized_start) in [
        (9, 16, false, 1, 0),
        (18, 19, false, 10, 6),
        (9, 17, true, 2, 0),
        (18, 19, true, 10, 7),
    ] {
        for button in [MouseButton::WheelDown, MouseButton::WheelUp] {
            let mut state = interactive();
            state.rows = (0..count)
                .map(|n| {
                    let mut row = ui().rows.remove(0);
                    row.id = format!("resize-{n}");
                    if grouped && n >= count / 2 {
                        row.workspace_id = "other-workspace".into();
                        row.workspace = "Other workspace".into();
                    }
                    row
                })
                .collect();
            state.selected = Some("resize-0".into());
            render(&state);
            for _ in 1..count {
                state.key(Key::Down);
                render(&state);
            }
            let selected = state.selected.clone();
            assert_eq!(state.viewport_start.get(), initial_start);
            assert!(state.follow_selection);
            state.popup_size = Some((80, height));
            let resized = render(&state);
            assert_eq!(state.mouse.borrow().scroll_max, resized_start);
            assert_eq!(
                state.viewport_start.get(),
                resized_start,
                "resize must reconcile bounds before {button:?}: count={count}, grouped={grouped}"
            );
            location(&state, &Hit::Pane(selected.clone().unwrap()));
            assert_eq!(render(&state), resized);
            wheel(&mut state, button);
            assert_eq!(state.selected, selected);
            assert_eq!(
                state.viewport_start.get(),
                if button == MouseButton::WheelDown {
                    resized_start
                } else {
                    resized_start.saturating_sub(1)
                }
            );
            if resized_start == 0 || button == MouseButton::WheelDown {
                assert_eq!(render(&state), resized);
            }
        }
    }
}

#[test]
fn mouse_wheel_empty_exact_fit_and_single_overflow_tables_clamp() {
    for count in [0, 8, 9] {
        let mut state = interactive();
        state.rows = (0..count)
            .map(|n| {
                let mut row = ui().rows.remove(0);
                row.id = format!("fit-{n}");
                row
            })
            .collect();
        state.selected = state.rows.first().map(|row| row.id.clone());
        let frame = render(&state);
        if count > 0 {
            assert_eq!(
                state
                    .mouse
                    .borrow()
                    .hits
                    .iter()
                    .filter(|(_, _, _, hit)| matches!(hit, Hit::Pane(_)))
                    .count(),
                count.min(8)
            );
        }
        for button in [MouseButton::WheelDown, MouseButton::WheelUp] {
            wheel(&mut state, button);
            let expected = usize::from(count == 9 && button == MouseButton::WheelDown);
            assert_eq!(state.viewport_start.get(), expected);
            assert_eq!(
                state.selected.as_deref(),
                if count == 0 { None } else { Some("fit-0") }
            );
            if expected == 0 {
                assert_eq!(render(&state), frame);
            } else {
                location(&state, &Hit::Pane("fit-8".into()));
            }
        }
    }
}

#[test]
fn mouse_wheel_help_search_scrolls_wrapped_matches_without_editing() {
    let mut state = interactive();
    state.key(Key::Char('?'));
    state.help.entries[0].keys = "Long binding ".repeat(90);
    state.key(Key::Char('f'));
    state.key(Key::Char('p'));
    let selected = state.help.selected;
    render(&state);
    let start = state.help.viewport_start.get();
    wheel(&mut state, MouseButton::WheelDown);
    assert_eq!(state.help.viewport_start.get(), start + 1);
    assert_eq!(state.help.selected, selected);
    assert_eq!(state.help.query, "p");
    assert!(state.help.searching);
    state.key(Key::Char('j'));
    assert_eq!(state.help.query, "pj");
}

#[test]
fn mouse_wheel_fitting_help_and_theme_list_are_inert() {
    for mode in [Mode::Help, Mode::Themes] {
        let mut state = interactive();
        state.popup_size = Some((120, 30));
        state.key(Key::Char(if mode == Mode::Help { '?' } else { 't' }));
        let frame = render(&state);
        assert_eq!(state.mouse.borrow().scroll_max, 0);
        for button in [MouseButton::WheelDown, MouseButton::WheelUp] {
            wheel(&mut state, button);
            assert_eq!(render(&state), frame);
        }
    }
}

#[test]
fn mouse_theme_list_keeps_the_remembered_pane_viewport() {
    let mut state = interactive();
    state.query = "Same".into();
    state.selected = Some("p-a".into());
    render(&state);
    let before = render(&state);
    let start = state.viewport_start.get();
    state.theme_index = 7;
    state.key(Key::Char('t'));
    render(&state);
    wheel(&mut state, MouseButton::WheelDown);
    assert_eq!(state.viewport_start.get(), start);
    state.key(Key::Esc);
    // Theme changes affect colors, but not the pane layout or viewport.
    state.theme_index = 0;
    assert_eq!(render(&state), before);
}

#[test]
fn mouse_wheel_retains_diagnostic_viewport_scrolling() {
    let mut state = interactive();
    state.status_errors.blocks = vec![(
        "Status colors invalid values".into(),
        (0..40).map(|n| format!("invalid color {n}")).collect(),
    )];
    let selected = state.selected.clone();
    render(&state);
    let top = render(&state);
    wheel(&mut state, MouseButton::WheelDown);
    assert_eq!(state.error_scroll.get(), 1);
    assert_ne!(render(&state), top);
    wheel(&mut state, MouseButton::WheelUp);
    assert_eq!(state.error_scroll.get(), 0);
    assert_eq!(render(&state), top);
    assert_eq!(state.selected, selected);
}

#[test]
fn mouse_wheel_search_preserves_filtered_selection_without_editing_or_leaving_preview() {
    for preview in [false, true] {
        let mut state = interactive();
        state.rows[0].agent = "codex".into();
        let (map, errors) = Keymap::parse(
            r#"{"select":{"next_row":["J"],"previous_row":["K"],"next_tab":["j"],"previous_tab":["k"]},"themes":{"next_theme":["J"],"previous_theme":["K"],"save":["j"],"cancel":["k"]}}"#,
        );
        assert!(errors.is_empty());
        state.keymap = map;
        if preview {
            state.key(Key::Char('t'));
            state.key(Key::Char('J'));
            state.key(Key::Char(' '));
            render(&state);
            click(&mut state, (4, 1));
        } else {
            state.key(Key::Char('f'));
        }
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        let visible = state.visible();
        assert_eq!(visible.len(), 2);
        state.selected = Some(state.rows[visible[0]].id.clone());
        for button in [
            MouseButton::WheelDown,
            MouseButton::WheelUp,
            MouseButton::WheelUp,
            MouseButton::WheelDown,
        ] {
            let before = state.selected.clone();
            wheel(&mut state, button);
            assert_eq!(state.query, "codex");
            assert_eq!(state.mode, Mode::Search);
            assert_eq!(state.selected, before);
            assert!(visible
                .iter()
                .any(|&i| Some(&state.rows[i].id) == state.selected.as_ref()));
            assert_eq!(state.theme_index, 0);
            assert_eq!(state.theme_cursor, usize::from(preview));
            assert_eq!(state.theme_table_preview, preview);
            assert!(state.theme_error.is_none());
        }
        for c in ['j', 'k'] {
            state.key(Key::Char(c));
        }
        assert_eq!(state.query, "codexjk");
    }
}

#[test]
fn search_wheel_preserves_selection_through_theme_roundtrip_and_keyboard_or_help_reset() {
    let directory =
        std::env::temp_dir().join(format!("crossroads-mouse-reset-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for help_reset in [false, true] {
        let mut state = interactive();
        state.popup_size = Some((120, 30));
        state.config_dir = Some(directory.clone());
        render(&state);
        click(&mut state, (4, 0));
        for c in "pane".chars() {
            state.key(Key::Char(c));
        }
        assert_eq!(state.visible().len(), 5);
        for (button, id) in [
            (MouseButton::WheelUp, "p-b"),
            (MouseButton::WheelDown, "p-b"),
            (MouseButton::WheelDown, "p-b"),
            (MouseButton::WheelUp, "p-b"),
        ] {
            wheel(&mut state, button);
            assert_eq!(state.selected.as_deref(), Some(id));
        }
        for _ in 0..4 {
            state.key(Key::Backspace);
        }
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        state.key(Key::Enter);
        assert_eq!(state.mode, Mode::Select);
        assert_eq!(state.query, "codex");
        assert_eq!(state.visible().len(), 1);
        let visible = state.visible();
        assert_eq!(
            state.rows[state.selected_index(&visible).unwrap()].id,
            "p-other"
        );
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        render(&state);
        click(&mut state, (14, 29));
        hover(&mut state, (4, 15));
        assert_eq!(state.mode, Mode::Help);
        click(&mut state, (4, 15));
        assert_eq!(state.mode, Mode::Themes);
        let at = location(&state, &Hit::Action(Action::OpenHelp));
        click(&mut state, at);
        hover(&mut state, (4, 4));
        state.key(Key::Enter);
        assert_eq!(state.mode, Mode::Themes);
        assert_eq!(state.theme_cursor, 1);
        render(&state);
        let at = location(&state, &Hit::Action(Action::TogglePreview));
        click(&mut state, at);
        click(&mut state, (4, 1));
        state.key(Key::Enter);
        assert!(state.theme_table_preview);
        assert_eq!(state.query, "codex");
        render(&state);
        let at = location(&state, &Hit::Action(Action::TogglePreview));
        click(&mut state, at);
        click(&mut state, (4, 4));
        assert_eq!(state.mode, Mode::Select);
        assert_eq!(crate::theme::NAMES[state.theme_index], "terminal");
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        if help_reset {
            click(&mut state, (14, 29));
            hover(&mut state, (4, 14));
            assert_eq!(state.mode, Mode::Help);
            let entry = state.help.selected.unwrap();
            assert_eq!(state.help.entries[entry].action, Some(Action::ResetSearch));
            state.key(Key::Enter);
        } else {
            state.key(Key::Char('r'));
        }
        assert_eq!(state.mode, Mode::Select);
        assert_eq!(state.query, "");
        assert_eq!(state.visible(), (0..7).collect::<Vec<_>>());
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        assert_eq!(state.selected_index(&state.visible()), Some(0));
        wheel(&mut state, MouseButton::WheelDown);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        assert_eq!(state.selected_index(&state.visible()), Some(0));
        click(&mut state, (2, 29));
        assert!(state.closed);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn mouse_wheel_help_search_uses_independent_filtered_entries_in_every_origin() {
    for (themes, preview) in [(false, false), (true, false), (true, true)] {
        let mut state = interactive();
        let (map, errors) = Keymap::parse(
            r#"{"help":{"next_category":["J"],"previous_category":["K"],"open_search":["j"],"reset_search":["k"]}}"#,
        );
        assert!(errors.is_empty());
        state.keymap = map;
        state.query = "pane filter".into();
        if themes {
            state.key(Key::Char('t'));
            if preview {
                state.key(Key::Char(' '));
            }
        }
        state.key(Key::Char('?'));
        state.key(Key::Char('j'));
        let query = if themes { "theme" } else { "pane" };
        for c in query.chars() {
            state.key(Key::Char(c));
        }
        let visible = state.help.visible();
        assert!(visible.len() >= 2);
        state.help.selected = Some(visible[0]);
        for (button, expected) in [
            (MouseButton::WheelUp, visible[0]),
            (MouseButton::WheelDown, visible[0]),
            (MouseButton::WheelDown, visible[0]),
        ] {
            wheel(&mut state, button);
            assert_eq!(state.help.query, query);
            assert_eq!(state.query, "pane filter");
            assert_eq!(state.help.selected, Some(expected));
            assert_eq!(state.mode, Mode::Help);
            assert!(state.help.searching);
            assert_eq!(state.theme_index, 0);
        }
        state.key(Key::Char('j'));
        state.key(Key::Char('k'));
        assert_eq!(state.help.query, format!("{query}jk"));
    }
}

#[test]
fn mouse_wheel_search_preserves_empty_single_and_overflow_queries() {
    for help in [false, true] {
        for query in [
            if help { "next pane" } else { "codex" },
            "no matches",
            &"x".repeat(200),
        ] {
            let mut state = interactive();
            state.key(Key::Char(if help { '?' } else { 'f' }));
            if help {
                state.key(Key::Char('f'));
                state.help.query = query.into();
            } else {
                state.query = query.into();
            }
            let visible = if help {
                state.help.visible()
            } else {
                state.visible()
            };
            assert_eq!(
                visible.len(),
                usize::from(query == "next pane" || query == "codex")
            );
            let selected = state.selected.clone();
            let help_selected = state.help.selected;
            for button in [MouseButton::WheelDown, MouseButton::WheelUp] {
                wheel(&mut state, button);
                assert_eq!(
                    if help {
                        &state.help.query
                    } else {
                        &state.query
                    },
                    query
                );
                assert!(state.mode == Mode::Search || state.help.searching);
                assert_eq!(state.selected, selected);
                assert_eq!(state.help.selected, help_selected);
            }
        }
    }
}

#[test]
fn mouse_wheel_themes_preserves_unsaved_choice_in_list_and_table() {
    for preview in [false, true] {
        let mut state = interactive();
        let (map, errors) = Keymap::parse(
            r#"{"themes":{"next_theme":["J"],"previous_theme":["K"],"save":["j"],"cancel":["k"]}}"#,
        );
        assert!(errors.is_empty());
        state.keymap = map;
        state.key(Key::Char('t'));
        state.key(Key::Char('J'));
        state.theme_table_preview = preview;
        for (button, expected) in [
            (MouseButton::WheelUp, 1),
            (MouseButton::WheelDown, 1),
            (MouseButton::WheelDown, 1),
        ] {
            wheel(&mut state, button);
            assert_eq!(state.theme_cursor, expected);
            assert_eq!(state.theme_index, 0);
            assert_eq!(state.mode, Mode::Themes);
            assert_eq!(state.theme_table_preview, preview);
            assert!(state.theme_error.is_none());
        }
    }
}

#[test]
fn mouse_opt_out_stale_layout_resize_and_misses_do_not_activate() {
    let mut state = interactive();
    let at = location(&state, &Hit::Pane("p-a".into()));
    state.keymap.mouse_enabled = false;
    click(&mut state, at);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.keymap.mouse_enabled = true;
    state.query = "no matches".into();
    assert_eq!(
        state.mouse_input(Mouse {
            button: MouseButton::Left,
            x: at.0,
            y: at.1
        }),
        None
    );
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.popup_size = Some((60, 12));
    render(&state);
    assert!(!state
        .mouse
        .borrow()
        .hits
        .iter()
        .any(|(_, _, _, hit)| matches!(hit, Hit::Pane(_))));
}

#[test]
fn mouse_motion_click_and_wheel_reject_disabled_stale_resized_and_outside_input() {
    for button in [
        MouseButton::Motion,
        MouseButton::Left,
        MouseButton::WheelDown,
    ] {
        for guard in [
            "disabled", "query", "status", "viewport", "resize", "outside", "miss",
        ] {
            let mut state = interactive();
            let mut at = location(&state, &Hit::Pane("p-a".into()));
            match guard {
                "disabled" => state.keymap.mouse_enabled = false,
                "query" => state.query = "codex".into(),
                "status" => state.rows[0].status = "done".into(),
                "viewport" => state.viewport_start.set(1),
                "resize" => state.popup_size = Some((120, 30)),
                "outside" => at = (80, 15),
                "miss" if button != MouseButton::WheelDown => at = (0, 2),
                "miss" => continue, // The wheel scrolls throughout the popup.
                _ => unreachable!(),
            }
            let selected = state.selected.clone();
            assert_eq!(
                state.mouse_input(Mouse {
                    button,
                    x: at.0,
                    y: at.1
                }),
                None,
                "{guard}"
            );
            assert_eq!(state.selected, selected, "{guard}");
            assert!(!state.closed, "{guard}");
        }
    }
    let mut state = interactive();
    let at = location(&state, &Hit::Pane("p-a".into()));
    assert_eq!(
        state.mouse_input(Mouse {
            button: MouseButton::Release,
            x: at.0,
            y: at.1
        }),
        None
    );
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert!(!state.closed);
}
