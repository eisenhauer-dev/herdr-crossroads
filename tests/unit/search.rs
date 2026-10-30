use crate::input::{Key, Keys};
use crate::search::matches_field;
use crate::selection::Mode;
use crate::test_support::ui;

#[test]
fn alt_text_is_ignored_in_both_searches() {
    for help in [false, true] {
        let mut state = ui();
        if help {
            state.key(Key::Char('?'));
        }
        state.key(Key::Char('f'));
        let mut keys = Keys::new();
        for byte in b"\x1bx" {
            if let Some(key) = keys.feed(*byte) {
                state.key(key);
            }
        }
        assert!(
            if help {
                state.help.searching && state.help.query.is_empty()
            } else {
                state.mode == Mode::Search && state.query.is_empty()
            },
            "Alt+x must be ignored, not Esc or text"
        );
    }
}

#[test]
fn ctrl_j_and_h_are_not_search_enter_or_backspace() {
    for help in [false, true] {
        let mut state = ui();
        if help {
            state.key(Key::Char('?'));
        }
        state.key(Key::Char('f'));
        state.key(Key::Char('x'));
        let mut keys = Keys::new();
        for byte in [10, 8] {
            if let Some(key) = keys.feed(byte) {
                state.key(key);
            }
        }
        if help {
            assert!(state.help.searching, "Ctrl+j must not end help search");
            assert_eq!(state.help.query, "x");
        } else {
            assert_eq!(state.mode, Mode::Search, "Ctrl+j must not end pane search");
            assert_eq!(state.query, "x");
        }
    }
}

#[test]
fn search_retains_text_and_selection_and_reset_restores_id() {
    let mut state = ui();
    state.key(Key::Char('j'));
    state.key(Key::Char('f'));
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(state.visible().len(), 1);
    assert!(state
        .frame()
        .lines()
        .any(|line| line.contains("> ") && line.contains("codex")));
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(state.query, "codex");
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    state.key(Key::Char('f'));
    assert_eq!(state.query, "codex");
    state.key(Key::Esc);
    state.key(Key::Char('r'));
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert!(state
        .frame()
        .lines()
        .any(|line| line.contains("  > ") && line.contains("Same")));
    state.key(Key::Char('f'));
    for c in "rq".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(state.query, "rq");
    assert!(!state.closed);
    assert!(state.frame().contains("No matching panes"));
    state.key(Key::Backspace);
    assert_eq!(state.query, "r");
    state.key(Key::Esc);
    assert!(!state.closed);
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    state.key(Key::Esc);
    state.key(Key::Char('q'));
    assert!(state.closed);
}

#[test]
fn subsequence_is_case_insensitive_and_never_spans_fields() {
    assert!(matches_field("Moped Frontend", "operont"));
    assert!(matches_field("Moped Frontend", "OPERONT"));
    let mut state = ui();
    state.query = "amesame".to_owned();
    assert!(state.visible().is_empty());
    state.query = "SECOND".to_owned();
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        vec!["p-second-tab"]
    );
    state.query = "codex".to_owned();
    assert_eq!(state.visible().len(), 1);
}
