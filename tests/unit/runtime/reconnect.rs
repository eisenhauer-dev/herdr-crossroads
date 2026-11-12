use super::*;
use crate::selection::TableView;
use crate::test_support::{ui, SNAPSHOT};
use crate::topology::rows;
use std::os::unix::net::UnixStream;

#[test]
fn reconnect_queued_blocked_status_appends_after_survivors_without_moving_selection() {
    for mode in [
        crate::selection::Mode::Select,
        crate::selection::Mode::Help,
        crate::selection::Mode::Themes,
    ] {
        let mut state = ui();
        assert!(state.status_event(&serde_json::json!({
            "event":"pane.agent_status_changed",
            "data":{"pane_id":"p-other", "workspace_id":"w2", "agent_status":"blocked"}
        })));
        state.set_table_view(TableView::Agent);
        state.selected = Some("p-b".into());
        state.mode = mode;
        let mut staged = rows(SNAPSHOT, Some("p-self"), 160).unwrap();
        staged
            .iter_mut()
            .find(|row| row.id == "p-other")
            .unwrap()
            .status = "blocked".into();
        let (socket, _peer) = UnixStream::pair().unwrap();
        let (tx, rx) = mpsc::sync_channel(2);
        tx.send(StatusMessage::Event(serde_json::json!({
            "event":"pane.agent_status_changed",
            "data":{"pane_id":"p-b", "workspace_id":"w1", "agent_status":"blocked"}
        })))
        .unwrap();
        let mut live = LiveStatus::new(false, None);
        live.pending = Some((StatusSubscription { socket, rx }, staged));
        live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
            .unwrap();
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|&i| state.rows[i].id.as_str())
                .collect::<Vec<_>>(),
            ["p-other", "p-b"]
        );
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        assert_eq!(state.mode, mode);
        assert!(!state.closed);
    }
}

#[test]
fn reconnect_reconciles_disappeared_selection_before_drawing() {
    let mut state = ui();
    state.selected = Some("p-other".into());
    state.query = "Same".into();
    let mut staged = rows(SNAPSHOT, Some("p-self"), 160).unwrap();
    staged.retain(|row| row.id != "p-other");
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let mut live = LiveStatus::new(false, None);
    live.pending = Some((StatusSubscription { socket, rx }, staged));

    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();

    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert_eq!(state.query, "Same");
    assert!(!state.rows.iter().any(|row| row.id == "p-other"));
    assert!(!state.closed);
}

#[test]
fn reconnect_keeps_opening_agent_membership_and_applies_queued_status() {
    let mut state = ui();
    state.selected = Some("p-other".into());
    state.set_table_view(TableView::Agent);
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let data = &mut snapshot["result"]["snapshot"];
    data["panes"]
        .as_array_mut()
        .unwrap()
        .retain(|pane| pane["pane_id"] != "p-other");
    data["panes"][2]["agent"] = Value::from("New assignment");
    data["panes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "pane_id":"p-new", "workspace_id":"w1", "tab_id":"t1", "agent":"New pane"
        }));
    let staged = rows(&snapshot.to_string(), Some("p-self"), 160).unwrap();
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (tx, rx) = mpsc::sync_channel(2);
    tx.send(StatusMessage::Event(serde_json::json!({
        "event":"pane.agent_status_changed",
        "data":{"pane_id":"p-b", "workspace_id":"w1", "agent_status":"blocked"}
    })))
    .unwrap();
    let mut live = LiveStatus::new(false, None);
    live.pending = Some((StatusSubscription { socket, rx }, staged));
    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();

    assert_eq!(state.table_view, TableView::Agent);
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        ["p-b"]
    );
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert_eq!(state.rows[state.visible()[0]].status, "blocked");
    tx.send(StatusMessage::Event(serde_json::json!({
        "event":"pane.agent_status_changed",
        "data":{"pane_id":"p-b", "workspace_id":"w1", "agent_status":"done"}
    })))
    .unwrap();
    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();
    assert_eq!(state.rows[state.visible()[0]].status, "done");
    assert!(!state.closed);

    let mut reopened = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 160).unwrap(),
        "catppuccin",
        None,
    );
    reopened.set_table_view(TableView::Agent);
    assert_eq!(
        reopened
            .visible()
            .iter()
            .map(|&i| reopened.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        ["p-b", "p-a", "p-new"]
    );
}

#[test]
fn reconnect_unknown_filter_keeps_opening_agent_without_name_or_status() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    state.key(crate::input::Key::Char('u'));
    assert!(state.visible().is_empty());
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let pane = &mut snapshot["result"]["snapshot"]["panes"][0];
    pane.as_object_mut().unwrap().remove("agent");
    pane.as_object_mut().unwrap().remove("agent_status");
    let staged = rows(&snapshot.to_string(), Some("p-self"), 160).unwrap();
    let select_ids: Vec<_> = staged.iter().map(|row| row.id.clone()).collect();
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let mut live = LiveStatus::new(false, None);
    live.pending = Some((StatusSubscription { socket, rx }, staged));
    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();

    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        ["p-other"]
    );
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    let survivor = &state.rows[state.visible()[0]];
    assert!(survivor.agent.is_empty());
    assert!(survivor.status.is_empty());
    assert!(state.frame_width(160).contains("Agent | unknown"));
    state.set_table_view(TableView::Select);
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.clone())
            .collect::<Vec<_>>(),
        select_ids
    );
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert!(!state.closed);
}

#[test]
fn reconnect_empty_agent_view_has_no_selection_and_rejects_disconnected_staging() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let mut live = LiveStatus::new(false, None);
    live.pending = Some((StatusSubscription { socket, rx }, Vec::new()));
    tx.send(StatusMessage::Disconnected).unwrap();
    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();
    assert_eq!(state.visible().len(), 2);
    assert_eq!(state.selected.as_deref(), Some("p-b"));

    let (socket, _peer) = UnixStream::pair().unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let staged = rows(SNAPSHOT, Some("p-self"), 160)
        .unwrap()
        .into_iter()
        .filter(|row| row.agent.is_empty())
        .collect();
    live.pending = Some((StatusSubscription { socket, rx }, staged));
    live.poll(&mut state, false, OsStr::new("unused-fake-herdr"), 160)
        .unwrap();
    assert!(state.visible().is_empty());
    assert_eq!(state.selected, None);
    assert_eq!(state.key(crate::input::Key::Enter), None);
    assert!(!state.closed);
    state.set_table_view(TableView::Select);
    assert_eq!(state.selected.as_deref(), Some("p-a"));
}
