use super::test_support::*;
use crate::input::{Key, Keys};
use crate::selection::{matches_field, Mode, Ui};
use crate::topology::rows;
use serde_json::Value;

#[test]
fn duplicate_visible_names_still_focus_selected_stable_pane_id() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["panes"][3]["label"] = Value::from("Claude");
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 76).unwrap());
    let frame = state.terminal_frame(76, 12);
    assert_eq!(frame.matches("Claude").count(), 2, "{frame}");
    assert!(!frame.contains("p-b") && !frame.contains("p-a"), "{frame}");
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-a"));
}

#[cfg(unix)]
#[test]
fn live_status_only_changes_matching_agent_row_not_navigation() {
    let mut state = ui();
    state.key(Key::Char('/'));
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    state.key(Key::Enter);
    let original_ids: Vec<_> = state.rows.iter().map(|row| row.id.clone()).collect();
    let selected = state.selected.clone();
    for (id, workspace) in [("p-other", "w1"), ("closed", "w2")] {
        assert!(!state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":id,"workspace_id":workspace,"agent_status":"blocked"}})));
    }
    assert!(state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-other","workspace_id":"w2","agent_status":"blocked"}})));
    assert_eq!(
        state
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        original_ids
    );
    assert_eq!(state.selected, selected);
    assert_eq!(state.query, "codex");
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    assert!(state.frame_width(65).contains("codex | blocked"));
    assert!(!state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-a","workspace_id":"w1","agent_status":"working","display_agent":"new agent"}})));
    state.query.clear();
    assert!(!state.frame_width(150).contains("new agent"));
    assert!(state
        .rows
        .iter()
        .find(|row| row.id == "p-a")
        .unwrap()
        .status
        .is_empty());
}

#[cfg(unix)]
#[test]
fn status_metadata_cannot_rename_filtered_agent_or_redirect_focus() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["agents"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"pane_id":"p-a","display_agent":"Claude Extra","agent_status":"idle"}));
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    state.query = "Claude".into();
    let visible_ids = |state: &Ui| {
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(visible_ids(&state), ["p-b", "p-a"]);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert!(state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","workspace_id":"w1","agent_status":"blocked","display_agent":"Renamed","agent":"Other"}})));
    assert_eq!(state.query, "Claude");
    assert_eq!(visible_ids(&state), ["p-b", "p-a"]);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert_eq!(
        state.rows.iter().find(|row| row.id == "p-b").unwrap().agent,
        "Claude"
    );
    assert!(state.frame_width(150).contains("Claude"));
    assert!(!state.frame_width(150).contains("Renamed"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
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
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Char('/'));
    state.key(tab);
    state.key(back);
    assert_eq!(state.mode, Mode::Search);
    assert_eq!(state.query, "codex");
    assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
    state.key(Key::Esc);
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    assert!(keys.feed(27).is_none());
    assert!(matches!(keys.flush(), Some(Key::Esc)));
}

#[test]
fn workspace_jumps_skip_hidden_blocks_and_stop_at_both_ends() {
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
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 150).unwrap());
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.query = "Third".into();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-third"));
    state.query = "Same".into();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.query.clear();
    state.key(Key::BackTab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.key(Key::Char('?'));
    state.key(Key::Tab);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    let help = state.terminal_frame(60, 15);
    assert!(help.contains("Tab/Shift+Tab: workspace"), "{help}");
    state.key(Key::Esc);
    let frame = state.terminal_frame(60, 15);
    assert_eq!(
        frame.lines().filter(|line| line.contains('┄')).count(),
        1,
        "{frame}"
    );
}

#[test]
fn query_above_list_and_help_describes_actual_keys() {
    let mut state = ui();
    state.key(Key::Char('/'));
    state.key(Key::Char('x'));
    let frame = state.terminal_frame(120, 20);
    assert!(frame.starts_with("  Search: x\r\n"), "{frame}");
    state.key(Key::Esc);
    assert!(state.terminal_frame(120, 20).contains("  Search: x\r\n"));
    state.key(Key::Char('?'));
    let help = state.terminal_frame(120, 20);
    assert!(help.contains("Enter: focus"), "{help}");
    assert!(help.contains("Esc/q/?: back"), "{help}");
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
    assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
    state.key(Key::Char('k'));
    assert_eq!(state.selected.as_deref(), Some("p-untitled"));
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
}

#[test]
fn search_retains_text_and_selection_and_reset_restores_id() {
    let mut state = ui();
    state.key(Key::Char('j'));
    state.key(Key::Char('/'));
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
    state.key(Key::Char('/'));
    assert_eq!(state.query, "codex");
    state.key(Key::Esc);
    state.key(Key::Char('r'));
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert!(state
        .frame()
        .lines()
        .any(|line| line.contains("  > ") && line.contains("Same")));
    state.key(Key::Char('/'));
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

#[test]
fn arrow_parser_and_empty_state() {
    let mut keys = Keys::new();
    assert!(keys.feed(27).is_none());
    assert!(keys.feed(b'[').is_none());
    assert!(matches!(keys.feed(b'B'), Some(Key::Down)));
    assert!(keys.feed(27).is_none());
    assert!(matches!(keys.flush(), Some(Key::Esc)));
    let empty = Ui::new(Vec::new());
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
    let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
    state.query = "Needle".into();
    let frame = state.frame();
    let lines: Vec<_> = frame.lines().collect();
    assert_eq!(lines[0], "  Search: Needle");
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
