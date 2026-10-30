use ratatui::buffer::CellWidth;
use std::time::{Duration, Instant};

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

fn click(state: &mut Ui, at: (usize, usize), now: Instant) -> Option<String> {
    let result = state.mouse_input(
        Mouse {
            button: MouseButton::Left,
            x: at.0,
            y: at.1,
        },
        now,
    );
    render(state);
    result
}

fn interactive() -> Ui {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    render(&state);
    state
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
            click(&mut state, (4, 1), Instant::now());
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
            click(&mut state, (x, 14), Instant::now());
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
            click(&mut state, (x, 14), Instant::now());
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
    let now = Instant::now();
    assert_eq!(click(&mut state, (4, 0), now), None);
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Char('x'));
    state.key(Key::Enter);
    state.key(Key::Char('?'));
    render(&state);
    click(&mut state, (4, 0), now);
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
    click(&mut state, (4, 1), Instant::now());
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Char('c'));
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    assert_eq!(state.theme_cursor, 1);
    assert_eq!(state.theme_index, 0);
    assert_eq!(state.query, "c");
}

#[test]
fn mouse_theme_double_click_survives_changed_preview_heading_but_not_item_location() {
    let mut state = interactive();
    state.key(Key::Char('t'));
    render(&state);
    let at = location(&state, &Hit::Theme(3));
    let now = Instant::now();
    click(&mut state, at, now);
    assert_eq!(state.theme_cursor, 3);
    assert!(state.theme_error.is_none());
    click(&mut state, at, now + Duration::from_millis(100));
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
    click(&mut state, at, Instant::now());
    assert!(
        state.auto_toast.is_some(),
        "selecting Auto must retain its existing unsupported-theme notice"
    );
    state.theme_error = Some("save failed".into());
    render(&state);
    let at = location(&state, &Hit::Action(Action::NextTheme));
    click(&mut state, at, Instant::now());
    assert!(state.theme_error.is_none());
}

#[test]
fn mouse_help_first_click_selects_delayed_repeat_executes_in_origin() {
    let mut state = interactive();
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Char('?'));
    render(&state);
    let at = location(&state, &Hit::Help(0));
    let now = Instant::now();
    click(&mut state, at, now);
    assert_eq!(state.mode, Mode::Help);
    assert_eq!(state.help.selected, Some(0));
    click(&mut state, at, now + Duration::from_secs(20));
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    assert_eq!(state.theme_cursor, 1);
    assert_eq!(state.theme_index, 0);
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
        .position(|e| e.action == Action::FocusPane);
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
            let actions: Vec<_> = catalogue.help.entries.iter().map(|e| e.action).collect();
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
fn mouse_pane_double_click_requires_same_item_location_and_400ms() {
    for delay in [399, 400, 401] {
        let mut state = interactive();
        let at = location(&state, &Hit::Pane("p-a".into()));
        let now = Instant::now();
        assert_eq!(click(&mut state, at, now), None);
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert!(!state.closed);
        let result = click(&mut state, at, now + Duration::from_millis(delay));
        assert_eq!(result.as_deref(), (delay <= 400).then_some("p-a"));
    }
    let mut state = interactive();
    let at = location(&state, &Hit::Pane("p-a".into()));
    let now = Instant::now();
    click(&mut state, at, now);
    assert_eq!(click(&mut state, (at.0 + 1, at.1), now), None);
    state.key(Key::Char('j'));
    state.key(Key::Char('k'));
    render(&state);
    assert_eq!(click(&mut state, at, now), None);
    state.rows[0].status = "done".into();
    render(&state);
    assert_eq!(click(&mut state, at, now), None);
    state.popup_size = Some((90, 15));
    render(&state);
    assert_eq!(click(&mut state, at, now), None);
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
    click(&mut state, (0, at.1), Instant::now());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    click(&mut state, at, Instant::now());
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
    click(&mut state, at, Instant::now());
    assert_eq!(state.selected.as_deref(), Some("scroll-38"));
    let selected = state.selected.clone();
    for at in [(3, 1), (3, 2), (3, 3), (79, 6), (1, 12)] {
        click(&mut state, at, Instant::now());
        assert_eq!(state.selected, selected);
        assert!(!state.closed);
    }
}

#[test]
fn mouse_wheel_uses_standard_j_k_for_current_mode_even_with_rebindings() {
    let mut state = interactive();
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["J"],"previous_row":["K"]},"help":{"next_category":["J"]},"themes":{"next_theme":["J"]}}"#,
    );
    assert!(errors.is_empty());
    state.keymap = map;
    let before = state.selected.clone();
    state.mouse_input(
        Mouse {
            button: MouseButton::WheelDown,
            x: 5,
            y: 5,
        },
        Instant::now(),
    );
    assert_ne!(state.selected, before);
    render(&state);
    state.key(Key::Char('f'));
    render(&state);
    state.mouse_input(
        Mouse {
            button: MouseButton::WheelDown,
            x: 5,
            y: 5,
        },
        Instant::now(),
    );
    assert_eq!(state.query, "j");
    state.key(Key::Enter);
    state.key(Key::Char('?'));
    render(&state);
    state.mouse_input(
        Mouse {
            button: MouseButton::WheelDown,
            x: 5,
            y: 5,
        },
        Instant::now(),
    );
    assert_eq!(state.help.selected, Some(1));
    state.key(Key::Char('q'));
    state.key(Key::Char('t'));
    render(&state);
    state.mouse_input(
        Mouse {
            button: MouseButton::WheelDown,
            x: 5,
            y: 5,
        },
        Instant::now(),
    );
    assert_eq!(state.theme_cursor, 1);
}

#[test]
fn mouse_opt_out_stale_layout_resize_and_misses_do_not_activate() {
    let mut state = interactive();
    let now = Instant::now();
    let at = location(&state, &Hit::Pane("p-a".into()));
    state.keymap.mouse_enabled = false;
    click(&mut state, at, now);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.keymap.mouse_enabled = true;
    state.query = "no matches".into();
    assert_eq!(
        state.mouse_input(
            Mouse {
                button: MouseButton::Left,
                x: at.0,
                y: at.1
            },
            now
        ),
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
