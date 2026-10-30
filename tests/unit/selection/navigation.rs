use crate::input::{Key, Keys};
use crate::selection::{Mode, Ui};
use crate::test_support::*;
use crate::topology::rows;
use serde_json::Value;

#[test]
fn navigation_handles_empty_rows_and_filtered_linked_workspace_blocks() {
    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::BackTab,
        Key::Tab,
    ];
    let mut empty = Ui::with_theme_choice(Vec::new(), "catppuccin", None);
    for key in keys {
        assert!(empty.key(key).is_none());
        assert_eq!(empty.selected, None);
        assert!(!empty.closed);
    }
    let mut state = ui();
    for row in &mut state.rows {
        if row.workspace_id == "w2" {
            row.workspace = "↳ Same (feature/demo)".into();
        }
    }
    state.query = "feature/demo".into();
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        ["p-other", "p-other-untitled"]
    );
    state.selected = Some("missing-pane".into());
    assert!(state.key(Key::Up).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-other-untitled"));
    assert!(state.key(Key::Down).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    for key in [Key::Left, Key::Right, Key::BackTab, Key::Tab] {
        state.selected = Some("p-other-untitled".into());
        assert!(state.key(key).is_none());
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.selected = Some("missing-pane".into());
        assert!(state.key(key).is_none());
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        assert_eq!(state.mode, Mode::Select);
        assert!(!state.closed);
    }
    assert_eq!(state.query, "feature/demo");
}

#[test]
fn last_visible_pane_wraps_forward_to_first() {
    let mut state = ui();
    state.selected = state.rows.last().map(|row| row.id.clone());
    assert!(state.key(Key::Down).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
}

#[test]
fn last_visible_tab_wraps_forward_to_first_block() {
    let mut state = ui();
    state.selected = state.rows.last().map(|row| row.id.clone());
    assert!(state.key(Key::Right).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
}

#[test]
fn last_visible_workspace_wraps_forward_to_first_block() {
    let mut state = ui();
    state.selected = state.rows.last().map(|row| row.id.clone());
    assert!(state.key(Key::Tab).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
}

#[test]
fn visible_panes_wrap_in_both_directions_without_focus() {
    for query in ["", "pane", "Claude", "no-such-pane"] {
        for (previous, next) in [(Key::Up, Key::Down), (Key::Char('k'), Key::Char('j'))] {
            let mut state = ui();
            state.query = query.into();
            let visible = state.visible();
            let remembered = state.selected.clone();
            if let Some(&first) = visible.first() {
                let last = *visible.last().unwrap();
                state.selected = Some(state.rows[first].id.clone());
                assert!(state.key(previous).is_none());
                assert_eq!(
                    state.selected.as_deref(),
                    Some(state.rows[last].id.as_str())
                );
                assert!(state.key(next).is_none());
                assert_eq!(
                    state.selected.as_deref(),
                    Some(state.rows[first].id.as_str())
                );
                state.selected = None;
                assert!(state.key(previous).is_none());
                assert_eq!(
                    state.selected.as_deref(),
                    Some(state.rows[last].id.as_str())
                );
            } else {
                for key in [previous, next] {
                    assert!(state.key(key).is_none());
                    assert_eq!(state.selected, remembered);
                }
            }
            assert_eq!(state.query, query);
            assert_eq!(state.mode, Mode::Select);
            assert!(!state.closed);
        }
    }
}

#[test]
fn visible_tab_blocks_wrap_to_first_row_even_from_middle_of_last_block() {
    for query in ["", "pane", "Claude", "no-such-pane"] {
        for (previous, next) in [(Key::Left, Key::Right), (Key::Char('h'), Key::Char('l'))] {
            let mut state = ui();
            state.query = query.into();
            let visible = state.visible();
            let remembered = state.selected.clone();
            if let Some(&first) = visible.first() {
                let last = *visible.last().unwrap();
                let last_block = *visible
                    .iter()
                    .find(|&&i| state.rows[i].tab_id == state.rows[last].tab_id)
                    .unwrap();
                state.selected = Some(state.rows[first].id.clone());
                assert!(state.key(previous).is_none());
                assert_eq!(
                    state.selected.as_deref(),
                    Some(state.rows[last_block].id.as_str())
                );
                state.selected = Some(state.rows[last].id.clone());
                assert!(state.key(next).is_none());
                assert_eq!(
                    state.selected.as_deref(),
                    Some(state.rows[first].id.as_str())
                );
            } else {
                for key in [previous, next] {
                    assert!(state.key(key).is_none());
                    assert_eq!(state.selected, remembered);
                }
            }
            assert_eq!(state.query, query);
            assert_eq!(state.mode, Mode::Select);
            assert!(!state.closed);
        }
    }
}

#[test]
fn visible_workspace_blocks_wrap_to_first_row_even_from_middle_of_last_block() {
    for query in ["", "pane", "Claude", "no-such-pane"] {
        let mut state = ui();
        state.query = query.into();
        let visible = state.visible();
        let remembered = state.selected.clone();
        if let Some(&first) = visible.first() {
            let last = *visible.last().unwrap();
            let last_block = *visible
                .iter()
                .find(|&&i| state.rows[i].workspace_id == state.rows[last].workspace_id)
                .unwrap();
            state.selected = Some(state.rows[first].id.clone());
            assert!(state.key(Key::BackTab).is_none());
            assert_eq!(
                state.selected.as_deref(),
                Some(state.rows[last_block].id.as_str())
            );
            state.selected = Some(state.rows[last].id.clone());
            assert!(state.key(Key::Tab).is_none());
            assert_eq!(
                state.selected.as_deref(),
                Some(state.rows[first].id.as_str())
            );
        } else {
            for key in [Key::BackTab, Key::Tab] {
                assert!(state.key(key).is_none());
                assert_eq!(state.selected, remembered);
            }
        }
        assert_eq!(state.query, query);
        assert_eq!(state.mode, Mode::Select);
        assert!(!state.closed);
    }
}

#[test]
fn duplicate_visible_names_still_focus_selected_stable_pane_id() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["panes"][3]["label"] = Value::from("Claude");
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 76).unwrap(),
        "catppuccin",
        None,
    );
    let frame = state.terminal_frame(76, 12);
    assert_eq!(frame.matches("Claude").count(), 2, "{frame}");
    assert!(!frame.contains("p-b") && !frame.contains("p-a"), "{frame}");
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-a"));
}

#[test]
fn tab_bytes_navigate_visible_workspaces_without_focusing() {
    let mut keys = Keys::new();
    let mut state = ui();
    let tab = keys.feed(b'\t').unwrap();
    assert!(state.key(tab).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Down);
    assert!(keys.feed(27).is_none());
    assert!(keys.feed(b'[').is_none());
    let back = keys.feed(b'Z').expect("Shift+Tab must parse");
    assert!(state.key(back).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.query = "codex".into();
    state.key(tab);
    state.key(back);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Char('f'));
    state.key(tab);
    state.key(back);
    assert_eq!(state.mode, Mode::Search);
    assert_eq!(state.query, "codex");
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Esc);
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    assert!(keys.feed(27).is_none());
    assert!(matches!(keys.flush(), Some(Key::Esc)));
}

#[test]
fn workspace_jumps_skip_hidden_blocks_and_wrap_at_both_ends() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let data = &mut snapshot["result"]["snapshot"];
    data["workspaces"][0]["label"] = Value::from("Middle");
    data["tabs"][0]["label"] = Value::from("Middle");
    data["workspaces"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"workspace_id":"w3","number":3,"label":"Same Third"}));
    data["tabs"].as_array_mut().unwrap().push(
        serde_json::json!({"tab_id":"t4","workspace_id":"w3","number":1,"label":"Third tab"}),
    );
    data["panes"].as_array_mut().unwrap().push(
            serde_json::json!({"pane_id":"p-third","workspace_id":"w3","tab_id":"t4","label":"Third pane"}),
        );
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 150).unwrap(),
        "catppuccin",
        None,
    );
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.query = "Third".into();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.query = "Same".into();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.query.clear();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.key(Key::Char('?'));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    let help = state.terminal_frame(60, 15);
    assert!(
        help.contains("Tab")
            && help.contains("Shift+Tab")
            && help.contains("next workspace")
            && help.contains("previous workspace"),
        "{help}"
    );
    state.key(Key::Esc);
    let frame = state.terminal_frame(60, 15);
    assert_eq!(
        frame.lines().filter(|line| line.contains('┄')).count(),
        2,
        "{frame}"
    );
}

#[test]
fn query_above_list_and_help_describes_actual_keys() {
    let mut state = ui();
    state.key(Key::Char('f'));
    state.key(Key::Char('x'));
    let frame = state.terminal_frame(120, 20);
    assert!(
        frame.starts_with(&format!(
            "\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m  Search: x{}Search\x1b[48;2;24;24;37m\x1b[K\r\n", " ".repeat(101)
        )),
        "{frame}"
    );
    state.key(Key::Esc);
    assert!(state.terminal_frame(120, 20).contains(&format!(
        "  Search: x{}Select\x1b[48;2;24;24;37m\x1b[K\r\n",
        " ".repeat(101)
    )));
    state.key(Key::Char('?'));
    let help = state.terminal_frame(120, 20);
    assert!(
        help.contains("Enter") && help.contains("focus selected pane"),
        "{help}"
    );
    assert!(help.contains("Esc: close keys"), "{help}");
    assert!(!help.contains("Esc/q: close"), "{help}");
}

#[test]
fn navigation_uses_visible_rows_and_adjacent_tab_blocks() {
    let mut state = ui();
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Char('j'));
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Char('l'));
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Left);
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Up);
    assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
    state.key(Key::Char('h'));
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Char('k'));
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
}

#[test]
fn arrow_parser_and_empty_state() {
    let mut keys = Keys::new();
    assert!(keys.feed(27).is_none());
    assert!(keys.feed(b'[').is_none());
    assert!(matches!(keys.feed(b'B'), Some(Key::Down)));
    assert!(keys.feed(27).is_none());
    assert!(matches!(keys.flush(), Some(Key::Esc)));
    let empty = Ui::with_theme_choice(Vec::new(), "catppuccin", None);
    assert!(empty.frame().contains("No matching panes"));
}

#[test]
fn filtered_navigation_skips_hidden_rows_and_retains_topology_order() {
    let mut state = ui();
    state.query = "Same".into();
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "p-b",
            "p-a",
            "p-untitled",
            "p-untitled-2",
            "p-second-tab",
            "p-other",
            "p-other-untitled"
        ]
    );
    state.query = "pane".into();
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Right);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-other-untitled"));
    assert!(state.frame().lines().any(|line| line.contains("pane 2")));
}

#[test]
fn first_remaining_row_shows_its_workspace_and_tab_context() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from("Workshop");
    snapshot["result"]["snapshot"]["tabs"][1]["label"] = Value::from("Backend");
    snapshot["result"]["snapshot"]["panes"][3]["label"] = Value::from("Needle");
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 160).unwrap(),
        "catppuccin",
        None,
    );
    state.query = "Needle".into();
    let frame = state.frame();
    let lines: Vec<_> = frame.lines().collect();
    assert_eq!(
        lines[0],
        format!("  Search: Needle{}Select", " ".repeat(136))
    );
    assert!(lines[1].starts_with("  ─"));
    assert!(
        frame.lines().any(|line| line.contains("Workshop")
            && line.contains("Backend")
            && line.contains("Needle")),
        "{frame}"
    );
    assert!(frame
        .lines()
        .any(|line| line.starts_with("  > ") && line.contains("Needle")));
    assert!(!frame.contains("Claude") && !frame.contains("p-a"));
}
