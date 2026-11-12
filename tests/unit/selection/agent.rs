use crate::input::{Key, Mouse, MouseButton};
use crate::keymap::Keymap;
use crate::selection::{Hit, Mode, TableView};
use crate::test_support::ui;

fn ordered_agents(blocked: &[usize]) -> crate::selection::Ui {
    let rows = (0..5)
        .map(|i| {
            let mut row = ui().rows.remove(0);
            row.id = format!("agent-{i}");
            row.workspace_id = format!("workspace-{}", i / 2);
            row.tab_id = format!("tab-{i}");
            row.agent = format!("Agent {i}");
            row.status = if blocked.contains(&i) {
                "blocked"
            } else {
                "working"
            }
            .into();
            row
        })
        .collect();
    crate::selection::Ui::with_theme_choice(rows, "catppuccin", None)
}

fn ordered_ids(state: &crate::selection::Ui) -> Vec<&str> {
    state
        .visible()
        .iter()
        .map(|&i| state.rows[i].id.as_str())
        .collect()
}

#[test]
fn agent_status_filters_intersect_search_and_reset_independently() {
    let mut state = ordered_agents(&[1, 3]);
    state.set_table_view(TableView::Agent);
    state.selected = Some("agent-3".into());
    state.key(Key::Char('b'));
    assert_eq!(ordered_ids(&state), ["agent-1", "agent-3"]);
    assert_eq!(state.selected.as_deref(), Some("agent-3"));
    state.query = "Agent 1".into();
    assert_eq!(ordered_ids(&state), ["agent-1"]);
    state.key(Key::Char('a'));
    assert_eq!(state.query, "Agent 1");
    assert_eq!(ordered_ids(&state), ["agent-1"]);
    state.key(Key::Char('b'));
    state.key(Key::Char('r'));
    assert_eq!(ordered_ids(&state), ["agent-1", "agent-3"]);
    for (key, value) in [
        ('d', "done"),
        ('w', "working"),
        ('i', "idle"),
        ('u', "unknown"),
    ] {
        status(&mut state, 0, value);
        state.key(Key::Char(key));
        assert!(state
            .visible()
            .iter()
            .all(|&i| state.rows[i].status == value));
        assert!(ordered_ids(&state).contains(&"agent-0"));
        assert!(state
            .frame_width(160)
            .contains(&format!("MODE: AGENT [{value}]")));
    }
    state.key(Key::Char('f'));
    for c in "ab dwiur".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(state.query, "ab dwiur");
    state.key(Key::Esc);
    state.key(Key::Char('r'));
    assert_eq!(ordered_ids(&state), ["agent-0"]);
    assert_eq!(state.selected.as_deref(), Some("agent-0"));
    state.key(Key::Char('s'));
    assert_eq!(state.visible().len(), 5);
    state.key(Key::Char('a'));
    assert_eq!(ordered_ids(&state), ["agent-0"]);
    assert!(ordered_agents(&[])
        .frame_width(160)
        .contains("MODE: SELECT"));
}

#[test]
fn agent_unknown_filter_includes_empty_status_without_changing_select() {
    let mut state = ordered_agents(&[]);
    state.rows[0].status.clear();
    state.rows[1].status = "unknown".into();
    state.rows[2].status = "unrecognized".into();
    state.set_table_view(TableView::Agent);
    state.key(Key::Char('u'));
    assert_eq!(ordered_ids(&state), ["agent-0", "agent-1"]);
    assert_eq!(state.selected.as_deref(), Some("agent-0"));
    state.query = "Agent 0".into();
    assert_eq!(ordered_ids(&state), ["agent-0"]);
    state.query.clear();
    state.key(Key::Char('w'));
    assert_eq!(ordered_ids(&state), ["agent-3", "agent-4"]);
    state.key(Key::Char('u'));
    state.set_table_view(TableView::Select);
    assert_eq!(
        ordered_ids(&state),
        ["agent-0", "agent-1", "agent-2", "agent-3", "agent-4"]
    );
    assert!(state.rows[0].status.is_empty());
}

#[test]
fn agent_metadata_search_is_optional_per_field_and_select_is_unchanged() {
    let mut state = ordered_agents(&[1]);
    state.rows[1].agent_metadata.title = Some("Unique task title".into());
    state.rows[1].agent_metadata.repository = Some(("repo-identity".into(), "Crossroads".into()));
    state.rows[1].agent_metadata.branch = Some("feature/status".into());
    for query in ["uqtkt", "CROSSROADS", "ftr/sts"] {
        state.query = query.into();
        assert!(
            state.visible().is_empty(),
            "Select must not search metadata"
        );
        state.set_table_view(TableView::Agent);
        assert_eq!(ordered_ids(&state), ["agent-1"], "{query}");
        state.key(Key::Char('w'));
        assert!(state.visible().is_empty());
        state.key(Key::Char('a'));
        state.set_table_view(TableView::Select);
    }
    state.set_table_view(TableView::Agent);
    state.query = "Crossroadsfeature".into();
    assert!(
        state.visible().is_empty(),
        "query must match within one field"
    );
    state.query = "repo-identity".into();
    assert!(
        state.visible().is_empty(),
        "repository identity is not a display name"
    );
}

#[test]
fn agent_live_filters_reconcile_selection_in_modal_views_and_reconnect() {
    for mode in [Mode::Select, Mode::Search, Mode::Help, Mode::Themes] {
        let mut state = ordered_agents(&[1, 3]);
        state.set_table_view(TableView::Agent);
        state.key(Key::Char('b'));
        state.selected = Some("agent-3".into());
        state.mode = mode;
        status(&mut state, 0, "blocked");
        assert_eq!(state.selected.as_deref(), Some("agent-3"));
        status(&mut state, 3, "done");
        assert_eq!(state.selected.as_deref(), Some("agent-1"));
        status(&mut state, 1, "done");
        status(&mut state, 0, "done");
        assert_eq!(state.selected, None);
        assert!(state.visible().is_empty());
        status(&mut state, 4, "blocked");
        assert_eq!(state.selected.as_deref(), Some("agent-4"));
        assert_eq!(state.mode, mode);
        state.replace_rows(ordered_agents(&[2]).rows);
        assert_eq!(ordered_ids(&state), ["agent-2"]);
        assert_eq!(state.selected.as_deref(), Some("agent-2"));
        assert!(!state.closed);
    }
}

#[test]
fn agent_filter_help_remapping_and_theme_preview_keep_origin_and_order() {
    let mut state = ordered_agents(&[1, 3]);
    let (map, errors) = Keymap::parse(r#"{"agent":{"filter_blocked":["B","!"]}}"#);
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    state.set_table_view(TableView::Agent);
    state.key(Key::Char('b'));
    assert_eq!(state.visible().len(), 5);
    state.key(Key::Char('?'));
    state.help.selected = state.help.entries.iter().position(|entry| {
        entry.action.is_some_and(|a| a.name() == "filter_blocked") && entry.keys == "B/!"
    });
    assert!(state.help.selected.is_some());
    state.key(Key::Enter);
    assert_eq!(ordered_ids(&state), ["agent-1", "agent-3"]);
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Char('?'));
    state.key(Key::Esc);
    state.popup_size = Some((160, 30));
    state.layout(160, 30, true);
    let mut panes: Vec<_> = state
        .mouse
        .borrow()
        .hits
        .iter()
        .filter_map(|(_, _, _, hit)| {
            if let Hit::Pane(id) = hit {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect();
    panes.dedup();
    assert_eq!(panes, ["agent-1", "agent-3"]);
    state.key(Key::Esc);
    assert_eq!(state.table_view, TableView::Agent);
    assert!(state.frame_width(160).contains("MODE: AGENT [blocked]"));
    for config in [
        r#"{"agent":{"filter_done":["b"]}}"#,
        r#"{"agent":{"filter_all":["Enter"]}}"#,
        r#"{"select":{"filter_idle":["I"]}}"#,
    ] {
        assert!(!Keymap::parse(config).1.is_empty(), "{config}");
    }
}

#[test]
fn agent_empty_status_result_explains_independent_resets_with_effective_keys() {
    let mut state = ordered_agents(&[]);
    state.set_table_view(TableView::Agent);
    let (map, errors) = Keymap::parse(r#"{"agent":{"filter_all":["A"],"reset_search":["R"]}}"#);
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    state.key(Key::Char('d'));
    let frame = state.frame_width(160);
    assert!(frame.contains("A: all statuses"), "{frame}");
    assert!(frame.contains("R: reset search"), "{frame}");
    assert!(state.selected.is_none());
    assert_eq!(state.key(Key::Enter), None);
    assert!(!state.closed);
    state.key(Key::Char('A'));
    assert_eq!(state.visible().len(), 5);
    let mut reopened = ordered_agents(&[]);
    reopened.set_table_view(TableView::Agent);
    assert!(reopened.frame_width(160).contains("MODE: AGENT [all]"));
}

fn status(state: &mut crate::selection::Ui, i: usize, value: &str) {
    assert!(state.status_event(&serde_json::json!({
        "event": "pane.agent_status_changed",
        "data": {"pane_id": format!("agent-{i}"),
                 "workspace_id": format!("workspace-{}", i / 2), "agent_status": value}
    })));
}

#[test]
fn blocked_agents_have_stable_transition_order_without_changing_select_or_selection() {
    let mut state = ordered_agents(&[1, 3]);
    let topology = ["agent-0", "agent-1", "agent-2", "agent-3", "agent-4"];
    assert_eq!(ordered_ids(&state), topology);
    state.selected = Some("agent-4".into());
    state.set_table_view(TableView::Agent);
    assert_eq!(
        ordered_ids(&state),
        ["agent-1", "agent-3", "agent-0", "agent-2", "agent-4"]
    );
    status(&mut state, 0, "blocked");
    status(&mut state, 1, "blocked");
    assert_eq!(
        ordered_ids(&state),
        ["agent-1", "agent-3", "agent-0", "agent-2", "agent-4"]
    );
    status(&mut state, 1, "done");
    assert_eq!(
        ordered_ids(&state),
        ["agent-3", "agent-0", "agent-1", "agent-2", "agent-4"]
    );
    status(&mut state, 1, "blocked");
    assert_eq!(
        ordered_ids(&state),
        ["agent-3", "agent-0", "agent-1", "agent-2", "agent-4"]
    );
    status(&mut state, 3, "idle");
    status(&mut state, 0, "unknown");
    status(&mut state, 1, "working");
    assert_eq!(ordered_ids(&state), topology);
    assert_eq!(state.selected.as_deref(), Some("agent-4"));
    assert!(!state.closed);
    state.set_table_view(TableView::Select);
    assert_eq!(ordered_ids(&state), topology);
}

#[test]
fn blocked_transitions_are_tracked_in_select_help_themes_and_hidden_search() {
    for view in [TableView::Select, TableView::Agent] {
        for mode in [Mode::Select, Mode::Search, Mode::Help, Mode::Themes] {
            let mut state = ordered_agents(&[]);
            state.set_table_view(view);
            state.mode = mode;
            state.theme_table_preview = true;
            state.query = "no-match".into();
            let selected = state.selected.clone();
            status(&mut state, 3, "blocked");
            status(&mut state, 1, "blocked");
            status(&mut state, 3, "blocked");
            assert!(state.visible().is_empty());
            assert_eq!(state.selected, selected);
            assert_eq!(state.mode, mode);
            state.query.clear();
            state.set_table_view(TableView::Agent);
            assert_eq!(
                ordered_ids(&state),
                ["agent-3", "agent-1", "agent-0", "agent-2", "agent-4"]
            );
            let frame = state.frame_width(240);
            if mode != Mode::Help {
                assert!(frame.find("Agent 3").unwrap() < frame.find("Agent 1").unwrap());
            }
            assert!(!state.closed);
        }
    }
}

#[test]
fn blocked_agent_navigation_uses_display_blocks_not_topology_indices() {
    for (key, expected) in [
        (Key::Tab, "agent-2"),
        (Key::BackTab, "agent-3"),
        (Key::Right, "agent-2"),
        (Key::Left, "agent-3"),
        (Key::Down, "agent-2"),
    ] {
        let mut state = ordered_agents(&[3, 1]);
        state.set_table_view(TableView::Agent);
        status(&mut state, 3, "working");
        status(&mut state, 3, "blocked");
        // Display order is 1, 3, 0, 2, 4; pane 0 is a later workspace block.
        state.selected = Some("agent-0".into());
        assert_eq!(state.key(key), None);
        assert_eq!(state.selected.as_deref(), Some(expected), "{key:?}");
        assert!(!state.closed);
    }
}

#[test]
fn blocked_agent_help_actions_and_preview_mouse_targets_follow_display_order() {
    let mut state = ordered_agents(&[3]);
    state.set_table_view(TableView::Agent);
    state.selected = Some("agent-2".into());
    state.key(Key::Char('?'));
    state.help.selected = state.help.entries.iter().position(|entry| {
        entry
            .action
            .is_some_and(|action| action.name() == "next_row")
    });
    assert_eq!(state.key(Key::Enter), None);
    assert_eq!(state.selected.as_deref(), Some("agent-4"));
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.popup_size = Some((240, 30));
    state.layout(240, 30, true);
    let hits = state.mouse.borrow();
    let y = |id: &str| {
        hits.hits
            .iter()
            .find(|(_, _, _, hit)| *hit == Hit::Pane(id.into()))
            .unwrap()
            .1
    };
    assert!(y("agent-3") < y("agent-0"));
    assert!(y("agent-0") < y("agent-1"));
    assert!(!state.closed);
}

#[test]
fn blocked_agent_empty_and_all_blocked_boundaries_keep_topology_order() {
    let mut state = ordered_agents(&[0, 1, 2, 3, 4]);
    state.set_table_view(TableView::Agent);
    assert_eq!(
        ordered_ids(&state),
        ["agent-0", "agent-1", "agent-2", "agent-3", "agent-4"]
    );
    state.replace_rows(Vec::new());
    assert!(state.visible().is_empty());
    assert_eq!(state.selected, None);
    assert_eq!(state.key(Key::Down), None);
    assert_eq!(state.key(Key::Enter), None);
    assert!(!state.closed);
}

#[test]
fn blocked_agent_reconnect_preserves_survivors_appends_new_blocks_and_keeps_membership() {
    let mut state = ordered_agents(&[3]);
    status(&mut state, 1, "blocked");
    state.set_table_view(TableView::Agent);
    state.selected = Some("agent-4".into());
    let mut replacement = ordered_agents(&[0, 1, 2, 3]).rows;
    let mut newcomer = ordered_agents(&[0]).rows.remove(0);
    newcomer.id = "new-agent".into();
    replacement.insert(0, newcomer);
    replacement.reverse();
    state.replace_rows(replacement);
    assert_eq!(
        ordered_ids(&state),
        ["agent-3", "agent-1", "agent-2", "agent-0", "agent-4"]
    );
    assert_eq!(state.selected.as_deref(), Some("agent-4"));
    let mut replacement = std::mem::take(&mut state.rows);
    replacement.retain(|row| row.id != "agent-3");
    replacement
        .iter_mut()
        .find(|row| row.id == "agent-1")
        .unwrap()
        .status = "done".into();
    state.replace_rows(replacement);
    assert_eq!(
        ordered_ids(&state),
        ["agent-2", "agent-0", "agent-4", "agent-1"]
    );
    status(&mut state, 1, "blocked");
    assert_eq!(
        ordered_ids(&state),
        ["agent-2", "agent-0", "agent-1", "agent-4"]
    );
    state.set_table_view(TableView::Select);
    assert_eq!(
        ordered_ids(&state),
        ["agent-4", "agent-2", "agent-1", "agent-0", "new-agent"]
    );
    assert!(!state.closed);
}

#[test]
fn agent_switches_share_search_without_focusing_and_use_visible_selection() {
    let mut state = ui();
    state.selected = Some("p-a".into());
    assert_eq!(state.key(Key::Char('a')), None);
    assert_eq!(state.table_view, TableView::Agent);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    assert!(!state.closed);
    assert!(state.frame_width(160).contains("MODE: AGENT"));
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    state.key(Key::Char('f'));
    assert!(state.frame_width(160).contains("Enter/Esc: agent"));
    for c in "asr".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(state.query, "asr");
    assert_eq!(state.table_view, TableView::Agent);
    assert!(state.frame_width(160).contains("MODE: AGENT"));
    assert_eq!(state.key(Key::Enter), None);
    assert_eq!(state.key(Key::Enter), None);
    assert!(!state.closed);
    state.key(Key::Char('s'));
    assert_eq!(state.table_view, TableView::Select);
    assert_eq!(state.query, "asr");
    assert_eq!(state.selected, None);
    state.key(Key::Char('r'));
    assert!(state.query.is_empty());
    state.key(Key::Char('f'));
    state.key(Key::Char('a'));
    state.key(Key::Char('s'));
    assert_eq!(state.query, "as");
    assert_eq!(state.table_view, TableView::Select);
    state.key(Key::Esc);
    state.key(Key::Char('r'));
    state.key(Key::Char('a'));
    state.query = "cdx".into();
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
}

#[test]
fn agent_keymaps_help_and_footer_use_effective_context_bindings() {
    let mut state = ui();
    let (map, errors) = Keymap::parse(
        r#"{"select":{"open_agent":["A"]},"agent":{"open_select":["S","V"],"next_row":["J"],"open_search":["/"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    let frame = state.frame_width(240);
    assert!(frame.contains("A: agent"), "{frame}");
    state.key(Key::Char('a'));
    assert_eq!(state.table_view, TableView::Select);
    state.key(Key::Char('A'));
    assert_eq!(state.table_view, TableView::Agent);
    state.key(Key::Char('J'));
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    let frame = state.frame_width(240);
    for text in ["S: select", "J: ↓pane", "Search: / to search"] {
        assert!(frame.contains(text), "{text}: {frame}");
    }
    state.key(Key::Char('?'));
    assert!(state.frame_width(240).contains("Keys — Agent"));
    let switch = state
        .help
        .entries
        .iter()
        .position(|e| e.action.is_some_and(|a| a.name() == "open_select") && e.keys == "S/V")
        .unwrap();
    state.help.selected = Some(switch);
    assert_eq!(state.key(Key::Enter), None);
    assert_eq!(state.table_view, TableView::Select);
    assert!(!state.closed);
    state.key(Key::Char('A'));
    state.key(Key::Char('s'));
    assert_eq!(state.table_view, TableView::Agent);
    state.key(Key::Char('V'));
    assert_eq!(state.table_view, TableView::Select);
    for text in [
        r#"{"agent":{"next_row":["s"]}}"#,
        r#"{"agent":{"open_select":["Enter"]}}"#,
        r#"{"agent":{"open_agent":["X"]}}"#,
    ] {
        assert!(!Keymap::parse(text).1.is_empty(), "{text}");
    }
}

#[test]
fn agent_help_themes_and_preview_search_keep_the_calling_table_filter() {
    let mut state = ui();
    state.key(Key::Char('a'));
    assert_eq!(state.table_view, TableView::Agent);
    state.selected = Some("p-other".into());
    state.query = "cdx".into();
    state.key(Key::Char('?'));
    state.key(Key::Char('f'));
    state.key(Key::Char('a'));
    state.key(Key::Char('s'));
    assert_eq!(state.help.query, "as");
    state.key(Key::Esc);
    state.key(Key::Esc);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(state.table_view, TableView::Agent);
    assert_eq!(state.query, "cdx");
    state.key(Key::Char('r'));
    assert!(state.query.is_empty());
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Char('?'));
    state.key(Key::Esc);
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    state.popup_size = Some((160, 30));
    state.layout(160, 30, true);
    assert!(!state
        .mouse
        .borrow()
        .hits
        .iter()
        .any(|(_, _, _, h)| *h == Hit::Pane("p-a".into())));
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, h)| *h == Hit::Search)
        .unwrap()
        .clone();
    state.mouse_input(Mouse {
        button: MouseButton::Left,
        x,
        y,
    });
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Char('c'));
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    state.key(Key::Esc);
    assert_eq!(state.table_view, TableView::Agent);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert!(!state.closed);
}

#[test]
fn table_switch_discards_old_mouse_hits_even_when_selection_is_unchanged() {
    let mut state = ui();
    state.popup_size = Some((160, 30));
    state.layout(160, 30, true);
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, h)| *h == Hit::Pane("p-a".into()))
        .unwrap()
        .clone();
    state.set_table_view(TableView::Agent);
    assert_eq!(
        state.mouse_input(Mouse {
            button: MouseButton::Left,
            x,
            y
        }),
        None
    );
    assert!(!state.closed);
    assert_eq!(state.selected.as_deref(), Some("p-b"));
    state.layout(160, 30, true);
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, h)| *h == Hit::Pane("p-other".into()))
        .unwrap()
        .clone();
    assert_eq!(
        state
            .mouse_input(Mouse {
                button: MouseButton::Left,
                x,
                y
            })
            .as_deref(),
        Some("p-other")
    );
}

#[test]
fn agent_navigation_reuses_visible_blocks_and_custom_bindings() {
    for action in [
        "next_row",
        "previous_row",
        "next_tab",
        "previous_tab",
        "next_workspace",
        "previous_workspace",
    ] {
        let mut state = ui();
        state.key(Key::Char('a'));
        let (map, errors) =
            Keymap::parse(&serde_json::json!({"agent": {action: ["N"]}}).to_string());
        assert!(errors.is_empty(), "{errors:?}");
        state.keymap = map;
        assert_eq!(state.key(Key::Char('N')), None);
        assert_eq!(state.selected.as_deref(), Some("p-other"), "{action}");
        state.key(Key::Char('N'));
        assert_eq!(state.selected.as_deref(), Some("p-b"), "{action}");
        assert!(!state.closed);
    }
}

#[test]
fn agent_mouse_switch_and_pane_activation_reuse_rendered_visible_targets() {
    for mode in [Mode::Select, Mode::Search, Mode::Themes] {
        let mut state = ui();
        state.popup_size = Some((240, 30));
        state.layout(240, 30, true);
        let (x, y, _, _) = state
            .mouse
            .borrow()
            .hits
            .iter()
            .find(|(_, _, _, h)| matches!(h, Hit::Action(a) if a.name() == "open_agent"))
            .unwrap()
            .clone();
        assert_eq!(
            state.mouse_input(Mouse {
                button: MouseButton::Left,
                x,
                y
            }),
            None
        );
        assert_eq!(state.table_view, TableView::Agent);
        assert!(state.mouse.borrow().hits.is_empty());
        match mode {
            Mode::Search => {
                state.key(Key::Char('f'));
            }
            Mode::Themes => {
                state.key(Key::Char('t'));
                state.key(Key::Char(' '));
            }
            _ => {}
        }
        state.layout(240, 30, true);
        let (x, y, _, _) = state
            .mouse
            .borrow()
            .hits
            .iter()
            .find(|(_, _, _, h)| *h == Hit::Pane("p-other".into()))
            .unwrap()
            .clone();
        let selected = state.selected.clone();
        assert_eq!(
            state.mouse_input(Mouse {
                button: MouseButton::WheelDown,
                x,
                y
            }),
            None
        );
        assert_eq!(state.selected, selected);
        assert_eq!(state.mode, mode);
        state.layout(240, 30, true);
        state.mouse_input(Mouse {
            button: MouseButton::Motion,
            x,
            y,
        });
        assert_eq!(
            state.selected, selected,
            "stationary reports after wheel must not select"
        );
        assert_eq!(
            state.mouse_input(Mouse {
                button: MouseButton::Motion,
                x: x + 1,
                y
            }),
            None
        );
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        assert!(!state.closed);
        state.layout(240, 30, true);
        let (x, y, _, _) = state
            .mouse
            .borrow()
            .hits
            .iter()
            .find(|(_, _, _, h)| *h == Hit::Pane("p-other".into()))
            .unwrap()
            .clone();
        assert_eq!(
            state
                .mouse_input(Mouse {
                    button: MouseButton::Left,
                    x,
                    y
                })
                .as_deref(),
            Some("p-other")
        );
    }
}

#[test]
fn agent_theme_save_and_help_focus_restore_the_origin() {
    let base = if cfg!(windows) {
        std::env::temp_dir()
    } else {
        std::path::PathBuf::from("/tmp/opencode")
    };
    let directory = base.join(format!("crossroads-agent-theme-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut state = ui();
    state.config_dir = Some(directory.clone());
    state.key(Key::Char('a'));
    state.selected = Some("p-other".into());
    state.key(Key::Char('t'));
    state.key(Key::Down);
    assert_eq!(state.key(Key::Enter), None);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(state.table_view, TableView::Agent);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert_eq!(state.theme_index, 1);
    assert!(!state.closed);
    state.query = "Claude".into();
    state.key(Key::Char('?'));
    state.help.selected = state
        .help
        .entries
        .iter()
        .position(|e| e.action.is_some_and(|a| a.name() == "focus_pane"));
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn agent_default_footer_is_complete_and_stable_at_supported_sizes() {
    for width in [80, 110, 120, 145, 240] {
        let mut state = ui();
        state.key(Key::Char('a'));
        let first = state.layout(width, 15, true);
        assert!(!state.hint_more.get(), "{first:?}");
        assert!(first.iter().any(|line| line.contains("s: select")));
        state.hint_scroll.set(1);
        assert_eq!(state.layout(width, 15, true), first);
    }
}
