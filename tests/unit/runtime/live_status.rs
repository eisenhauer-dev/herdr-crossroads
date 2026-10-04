use crate::input::Key;
use crate::selection::Ui;
use crate::test_support::*;
use crate::topology::rows;
use serde_json::Value;

#[cfg(unix)]
#[test]
fn row_updates_validate_events_without_ui_state_or_metadata_changes() {
    use crate::runtime::live_status::apply_status;
    let mut staged = rows(SNAPSHOT, Some("p-self"), 160).unwrap();
    let original = |rows: &[crate::topology::Row]| {
        rows.iter()
            .map(|row| {
                (
                    row.id.clone(),
                    row.workspace_id.clone(),
                    row.workspace.clone(),
                    row.tab_id.clone(),
                    row.tab.clone(),
                    row.pane.clone(),
                    row.agent.clone(),
                    row.linked_worktree,
                )
            })
            .collect::<Vec<_>>()
    };
    let metadata = original(&staged);
    for event in [
        serde_json::json!({"event":"other","data":{"pane_id":"p-b","workspace_id":"w1","agent_status":"blocked"}}),
        serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","agent_status":"blocked"}}),
        serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","workspace_id":"w1","agent_status":"invalid"}}),
        serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","workspace_id":"w2","agent_status":"blocked"}}),
        serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-a","workspace_id":"w1","agent_status":"blocked"}}),
    ] {
        let statuses = staged
            .iter()
            .map(|row| row.status.clone())
            .collect::<Vec<_>>();
        assert!(!apply_status(&mut staged, &event));
        assert_eq!(
            staged
                .iter()
                .map(|row| row.status.clone())
                .collect::<Vec<_>>(),
            statuses
        );
    }
    for status in ["working", "blocked", "done", "idle", "unknown"] {
        assert!(apply_status(
            &mut staged,
            &serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","workspace_id":"w1","agent_status":status,"display_agent":"renamed"}})
        ));
        assert_eq!(
            staged.iter().find(|row| row.id == "p-b").unwrap().status,
            status
        );
        assert_eq!(original(&staged), metadata);
    }
    assert!(!apply_status(&mut [], &serde_json::json!({})));
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
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 160).unwrap(),
        "catppuccin",
        None,
    );
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
