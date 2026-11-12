#[path = "selection/agent.rs"]
mod agent;
#[path = "selection/help.rs"]
mod help;
#[path = "selection/navigation.rs"]
mod navigation;
#[path = "selection/themes.rs"]
mod themes;

use crate::input::Key;
use crate::selection::{Mode, TableView, Ui};
use crate::test_support::ui;

fn visible_ids(state: &Ui) -> Vec<&str> {
    state
        .visible()
        .iter()
        .map(|&i| state.rows[i].id.as_str())
        .collect()
}

#[test]
fn table_view_preserves_visible_identity_and_falls_back_without_focusing() {
    let mut state = ui();
    assert_eq!(state.table_view, TableView::Select);
    state.selected = Some("p-other".into());
    state.set_table_view(TableView::Agent);
    assert_eq!(visible_ids(&state), ["p-b", "p-other"]);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.set_table_view(TableView::Select);
    assert_eq!(state.selected.as_deref(), Some("p-other"));

    state.selected = Some("p-a".into());
    state.set_table_view(TableView::Agent);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert!(!state.closed);
    assert!(state.key(Key::Down).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert!(state.key(Key::Tab).is_none());
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
}

#[test]
fn table_view_reuses_search_and_is_independent_of_interaction_state() {
    let mut state = ui();
    state.query = "cdx".into();
    for mode in [Mode::Select, Mode::Search, Mode::Help, Mode::Themes] {
        state.mode = mode;
        state.set_table_view(TableView::Agent);
        assert_eq!(state.mode, mode);
        assert_eq!(state.query, "cdx");
        assert_eq!(visible_ids(&state), ["p-other"]);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.set_table_view(TableView::Select);
    }
    state.query = "pane 3".into();
    state.set_table_view(TableView::Agent);
    assert!(state.visible().is_empty());
    assert_eq!(state.selected, None);
    state.set_table_view(TableView::Select);
    assert_eq!(state.selected.as_deref(), Some("p-untitled"));
    state.query = "missing".into();
    state.set_table_view(TableView::Agent);
    state.mode = Mode::Select;
    assert_eq!(state.key(Key::Enter), None);
    assert!(!state.closed);

    let mut empty = Ui::with_theme_choice(Vec::new(), "catppuccin", None);
    empty.set_table_view(TableView::Agent);
    assert!(empty.visible().is_empty());
    assert_eq!(empty.selected, None);
}

#[test]
fn agent_membership_is_captured_before_row_metadata_changes() {
    let mut state = ui();
    for row in &mut state.rows {
        row.agent = if row.id == "p-a" {
            "New agent".into()
        } else {
            String::new()
        };
    }
    state.set_table_view(TableView::Agent);
    assert_eq!(visible_ids(&state), ["p-b", "p-other"]);
}
