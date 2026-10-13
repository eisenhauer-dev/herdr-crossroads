use crate::input::Key;
use crate::selection::Mode;
use crate::test_support::ui;

#[test]
fn help_search_keeps_letters_as_text_and_arrows_do_not_cycle() {
    let mut state = ui();
    state.key(Key::Char('?'));
    state.key(Key::Char('/'));
    for key in [Key::Up, Key::Down, Key::Char('j'), Key::Char('k')] {
        assert!(state.key(key).is_none());
        assert_eq!(state.help.selected, Some(0));
        assert!(state.help.searching);
        assert_eq!(state.mode, Mode::Help);
    }
    assert_eq!(state.help.query, "jk");
    assert!(!state.closed);
}

#[test]
fn last_help_entry_wraps_forward_to_first_match() {
    let mut state = ui();
    state.key(Key::Char('?'));
    state.help.query = "search".into();
    state.help.selected = Some(7);
    assert!(state.key(Key::Down).is_none());
    assert_eq!(state.help.selected, Some(6));
}

#[test]
fn help_entries_wrap_only_visible_matches_in_both_caller_contexts() {
    for themes in [false, true] {
        for query in ["", "theme", "search", "show keys", "not-a-key"] {
            for (previous, next) in [(Key::Up, Key::Down), (Key::Char('k'), Key::Char('j'))] {
                let mut state = ui();
                state.query = "Claude".into();
                state.viewport_start.set(3);
                state.last_selected_at.set(Some(4));
                if themes {
                    state.key(Key::Char('t'));
                    state.key(Key::Down);
                    state.key(Key::Char(' '));
                }
                let selected = state.selected.clone();
                let cursor = state.theme_cursor;
                state.key(Key::Char('?'));
                state.help.query = query.into();
                let visible = state.help.visible();
                let remembered = state.help.selected;
                if let Some(&first) = visible.first() {
                    let last = *visible.last().unwrap();
                    state.help.selected = Some(first);
                    assert!(state.key(previous).is_none());
                    assert_eq!(state.help.selected, Some(last));
                    state.terminal_frame(80, 15);
                    if !themes && query.is_empty() {
                        assert!(state.help.viewport_start.get() > 0);
                    }
                    assert!(state.key(next).is_none());
                    assert_eq!(state.help.selected, Some(first));
                    state.terminal_frame(80, 15);
                    assert_eq!(state.help.viewport_start.get(), 0);
                    state.help.selected = None;
                    assert!(state.key(previous).is_none());
                    assert_eq!(state.help.selected, Some(last));
                } else {
                    for key in [previous, next] {
                        assert!(state.key(key).is_none());
                        assert_eq!(state.help.selected, remembered);
                    }
                }
                assert_eq!(state.help.query, query);
                assert_eq!(state.mode, Mode::Help);
                assert!(!state.closed);
                state.key(Key::Esc);
                assert_eq!(state.mode, if themes { Mode::Themes } else { Mode::Select });
                assert_eq!(state.query, "Claude");
                assert_eq!(state.selected, selected);
                assert_eq!(state.viewport_start.get(), 3);
                assert_eq!(state.last_selected_at.get(), Some(4));
                assert_eq!(state.theme_cursor, cursor);
                assert_eq!(state.theme_index, 0);
                assert_eq!(state.theme_table_preview, themes);
            }
        }
    }
}

#[test]
fn help_has_independent_search_and_fixed_table_chrome() {
    let mut state = ui();
    state.query = "Claude".into();
    state.key(Key::Char('?'));
    let lines = state.layout(80, 15, false);
    assert!(lines[0].starts_with("  Search: / to search") && lines[0].ends_with("Help"));
    assert_eq!(lines[1], "  Keys — Select");
    assert!(lines[2].starts_with("  ─"), "{lines:?}");
    assert!(
        lines.iter().any(|line| line.contains("Navigation")),
        "{lines:?}"
    );
    assert!(!lines.iter().any(|line| line.contains("Claude")));
    state.key(Key::Char('/'));
    for c in "focus".chars() {
        state.key(Key::Char(c));
    }
    let frame = state.frame_width(80);
    assert!(frame.contains("Search: focus"), "{frame}");
    assert!(
        frame.contains("Actions") && !frame.contains("Navigation"),
        "{frame}"
    );
    assert!(
        frame.contains("Enter") && frame.contains("focus selected pane"),
        "{frame}"
    );
    assert_eq!(state.query, "Claude");
}

#[test]
fn help_filter_matches_columns_independently_in_original_order() {
    let mut state = ui();
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    let entries = &state.help.entries;
    assert_eq!(
        entries
            .iter()
            .map(|e| (e.keys.as_str(), e.meaning))
            .collect::<Vec<_>>(),
        vec![
            ("j/↓", "next pane"),
            ("k/↑", "previous pane"),
            ("h/←", "previous tab"),
            ("l/→", "next tab"),
            ("→ Tab | ← Shift+Tab", "next/previous workspace"),
            ("Enter", "focus selected pane"),
            ("/", "search"),
            ("r", "reset search"),
            ("t", "choose theme"),
            ("?", "show keys"),
            ("Esc/q", "close Crossroads")
        ]
    );
    for (query, expected) in [
        ("DN", vec![5]),
        ("PRVP", vec![1, 4]),
        ("search", vec![6, 7]),
        ("jnext", vec![]),
        ("Navigation", vec![]),
        ("Actions", vec![]),
        ("not-a-key", vec![]),
    ] {
        state.help.query = query.into();
        assert_eq!(state.help.visible(), expected, "{query}");
    }
    state.help.query = "not-a-key".into();
    for key in [
        Key::Down,
        Key::Up,
        Key::Char('j'),
        Key::Char('k'),
        Key::Enter,
    ] {
        assert!(state.key(key).is_none());
    }
    let empty = state.frame_width(80);
    assert!(empty.contains("No matching keys"), "{empty}");
    assert!(!empty.contains("Navigation") && !empty.contains("Actions"));
    assert!(!state.closed);
}

#[test]
fn help_search_input_reset_reopen_and_close_preserve_select_state() {
    for close in [Key::Esc, Key::Char('q')] {
        let mut state = ui();
        state.popup_size = Some((80, 15));
        state.query = "Claude".into();
        state.viewport_start.set(3);
        state.last_selected_at.set(Some(4));
        state.theme_index = 6;
        let selected = state.selected.clone();
        let visible = state.visible();
        let ids = state.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        state.key(Key::Char('?'));
        for _ in 1..state.help.entries.len() {
            state.key(Key::Down);
            state.terminal_frame(80, 15);
        }
        assert_eq!(state.help.selected, Some(10));
        assert!(state.help.viewport_start.get() > 0);
        assert!(state.key(Key::Enter).is_none());
        assert!(!state.closed && state.mode == Mode::Help);
        state.key(Key::Char('/'));
        for c in "qr?é".chars() {
            state.key(Key::Char(c));
        }
        assert_eq!(state.help.query, "qr?é");
        state.key(Key::Backspace);
        assert_eq!(state.help.query, "qr?");
        assert!(state.help.searching && !state.closed);
        let input = state.frame_width(80);
        assert!(input.contains("Enter/Esc: return to keys") && input.contains("Backspace: erase"));
        assert!(!input.contains("Esc: close keys"));
        state.key(Key::Esc);
        assert!(!state.help.searching && state.mode == Mode::Help);
        state.key(Key::Char('r'));
        assert!(state.help.query.is_empty());
        assert_eq!(state.help.selected, Some(10));
        state.key(Key::Char('/'));
        state.key(Key::Char('t'));
        state.key(Key::Enter);
        assert!(!state.help.searching && state.help.query == "t");
        state.terminal_frame(80, 15);
        state.key(close);
        assert_eq!(state.mode, Mode::Select);
        assert!(!state.closed);
        assert_eq!(state.query, "Claude");
        assert_eq!(state.selected, selected);
        assert_eq!(state.visible(), visible);
        assert_eq!(state.viewport_start.get(), 3);
        assert_eq!(state.last_selected_at.get(), Some(4));
        assert_eq!(state.theme_index, 6);
        assert_eq!(
            state.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            ids
        );
        state.key(Key::Char('?'));
        assert!(state.help.query.is_empty() && !state.help.searching);
        assert_eq!(state.help.selected, Some(0));
        assert_eq!(state.help.viewport_start.get(), 0);
    }
}

#[test]
fn help_navigation_wraps_reset_keeps_visible_entry_and_search_treats_question_as_text() {
    let mut state = ui();
    state.key(Key::Char('/'));
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Search);
    assert_eq!(state.query, "?");
    state.key(Key::Esc);
    state.key(Key::Char('t'));
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    state.key(Key::Esc);
    assert_eq!(state.mode, Mode::Themes);
    state.key(Key::Esc);
    state.key(Key::Char('?'));
    state.key(Key::Up);
    assert_eq!(state.help.selected, Some(10));
    state.help.query = "search".into();
    state.key(Key::Char('j'));
    assert_eq!(state.help.selected, Some(7));
    state.key(Key::Down);
    assert_eq!(state.help.selected, Some(6));
    state.key(Key::Char('k'));
    assert_eq!(state.help.selected, Some(7));
    state.key(Key::Up);
    assert_eq!(state.help.selected, Some(6));
    state.key(Key::Char('r'));
    assert_eq!(state.help.selected, Some(6));
    assert!(state.help.query.is_empty());
}

#[test]
fn themes_help_preserves_pending_preview_and_pane_state_without_saving() {
    for preview in [false, true] {
        for close in [Key::Esc, Key::Char('q')] {
            let mut state = ui();
            state.query = "Claude".into();
            state.viewport_start.set(3);
            state.last_selected_at.set(Some(4));
            state.theme_index = 6;
            state.key(Key::Char('t'));
            state.key(Key::Down);
            state.theme_table_preview = preview;
            let selected = state.selected.clone();
            let pending = state.theme_cursor;
            let theme = state.theme();
            assert!(state.key(Key::Char('?')).is_none());
            assert_eq!(state.mode, Mode::Help);
            assert_eq!(
                state.theme(),
                theme,
                "help retains the pending preview palette"
            );
            let lines = state.layout(80, 15, false);
            assert_eq!(lines[1], "  Keys — Themes");
            assert_eq!(
                state
                    .help
                    .entries
                    .iter()
                    .map(|e| (e.keys.as_str(), e.meaning))
                    .collect::<Vec<_>>(),
                vec![
                    ("j/↓", "next theme"),
                    ("k/↑", "previous theme"),
                    ("Space", "toggle list/table preview"),
                    ("Enter", "save theme"),
                    ("Esc/q", "cancel theme selection"),
                    ("?", "show keys")
                ]
            );
            state.key(Key::Char('/'));
            state.key(Key::Char('?'));
            assert_eq!(state.help.query, "?");
            state.terminal_frame(80, 15);
            state.key(Key::Esc);
            assert_eq!(state.mode, Mode::Help);
            assert!(!state.help.searching);
            assert!(state.key(close).is_none());
            assert_eq!(state.mode, Mode::Themes);
            assert!(!state.closed);
            assert_eq!(state.query, "Claude");
            assert_eq!(state.selected, selected);
            assert_eq!(state.viewport_start.get(), 3);
            assert_eq!(state.last_selected_at.get(), Some(4));
            assert_eq!(state.theme_index, 6);
            assert_eq!(state.theme_cursor, pending);
            assert_eq!(state.theme_table_preview, preview);
            assert!(state.theme_error.is_none());
        }
    }
}

#[test]
fn themes_help_uses_effective_caller_bindings_and_configured_help_controls() {
    use crate::keymap::Keymap;
    let mut state = ui();
    let (map, errors) = Keymap::parse(
        r#"{"themes":{"next_theme":["N","Right"],"previous_theme":["P"],"toggle_preview":["v"],"save":["s"],"cancel":["c"],"open_help":["H"]},"help":{"next_category":["n"],"open_search":["f"],"close":["x"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    state.key(Key::Char('t'));
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Themes);
    state.key(Key::Char('H'));
    assert_eq!(state.mode, Mode::Help);
    assert_eq!(
        state
            .help
            .entries
            .iter()
            .map(|e| e.keys.as_str())
            .collect::<Vec<_>>(),
        ["N/→", "P", "v", "s", "c", "H"]
    );
    state.key(Key::Char('n'));
    assert_eq!(state.help.selected, Some(1));
    state.key(Key::Char('f'));
    state.key(Key::Char('s'));
    assert_eq!(state.help.visible(), [1, 2, 3, 4, 5]);
    state.key(Key::Esc);
    state.key(Key::Char('x'));
    assert_eq!(state.mode, Mode::Themes);
}

#[cfg(unix)]
#[test]
fn help_status_updates_do_not_change_help_or_pane_navigation() {
    let mut state = ui();
    state.query = "Claude".into();
    let selected = state.selected.clone();
    state.key(Key::Char('?'));
    state.key(Key::Char('/'));
    state.key(Key::Char('r'));
    let before = state.terminal_frame(80, 15);
    assert!(state.status_event(
        &serde_json::json!({"event":"pane.agent_status_changed","data":{
        "pane_id":"p-b","workspace_id":"w1","agent_status":"blocked"}})
    ));
    assert_eq!(state.terminal_frame(80, 15), before);
    assert_eq!(state.selected, selected);
    assert_eq!(state.query, "Claude");
    state.key(Key::Esc);
    state.key(Key::Esc);
    assert!(state.terminal_frame(80, 15).contains("blocked"));
}
#[test]
fn help_header_names_select_caller() {
    let mut state = ui();
    state.key(Key::Char('?'));
    assert!(state
        .layout(80, 15, false)
        .iter()
        .any(|line| line == "  Keys — Select"));
}

#[test]
fn closing_nested_themes_help_returns_to_themes() {
    let mut state = ui();
    state.key(Key::Char('t'));
    state.key(Key::Char('?'));
    state.key(Key::Esc);
    assert_eq!(state.mode, Mode::Themes);
}
