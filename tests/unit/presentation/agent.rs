use crate::input::{Key, Mouse, MouseButton};
use crate::selection::{Hit, TableView};
use crate::test_support::ui;
use ratatui::buffer::CellWidth;

#[test]
fn agent_cards_show_location_and_only_selected_optional_details() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    for row in &mut state.rows {
        row.agent_metadata.repository = Some(("identity".into(), "crossroads".into()));
        row.agent_metadata.branch = Some("develop".into());
        row.agent_metadata.title = Some(format!("Task {}", row.id));
    }
    let frame = state.frame_width(80);
    assert!(!frame.contains("Agent/Pane Name"), "{frame}");
    assert!(frame.contains("Same › Same › Same"), "{frame}");
    assert!(frame.contains("Repository: crossroads"), "{frame}");
    assert!(frame.contains("Branch: develop"), "{frame}");
    assert!(frame.contains("Titel: Task p-b"), "{frame}");
    assert!(!frame.contains("Titel: Task p-other"), "{frame}");
    assert_eq!(frame.matches("Repository:").count(), 1);
    state.rows[0].agent_metadata.title = Some("Same › Same › Same".into());
    assert!(!state.frame_width(80).contains("Titel:"));
    state.key(Key::Char('s'));
    assert!(state.frame_width(80).contains("Agent/Pane Name"));
}

#[test]
fn agent_cards_clip_whole_graphemes_and_keep_keyboard_selection_visible() {
    let mut rows = Vec::new();
    for i in 0..20 {
        let mut row = ui().rows.remove(0);
        row.id = format!("card-{i}");
        row.agent = format!("Agent {i}");
        row.agent_metadata.title = Some("e\u{301}👩\u{200d}💻界".repeat(100));
        row.agent_metadata.branch = Some("develop".into());
        rows.push(row);
    }
    let mut state = crate::selection::Ui::with_theme_choice(rows, "catppuccin", None);
    state.set_table_view(TableView::Agent);
    state.popup_size = Some((80, 15));
    for i in 0..20 {
        let frame = state.terminal_frame(80, 15);
        let plain: Vec<_> = frame
            .split("\r\n")
            .map(|line| super::without_sgr(&line.replace("\x1b[K", "")))
            .collect();
        assert_eq!(plain.len(), 15);
        assert!(plain
            .iter()
            .all(|line| line.replace("\x1b[K", "").cell_width() < 80));
        assert!(frame.contains(&format!("Agent {i}")), "{frame}");
        assert!(frame.contains("Titel:"), "{frame}");
        state.key(Key::Down);
    }
}

#[test]
fn agent_hover_expansion_invalidates_hits_and_stationary_motion_does_not_reselect() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    state.popup_size = Some((80, 15));
    state.rows[0].agent_metadata.title = Some("Expanded task".into());
    state.layout(80, 15, true);
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, hit)| *hit == Hit::Pane("p-other".into()))
        .unwrap()
        .clone();
    let motion = Mouse {
        button: MouseButton::Motion,
        x,
        y,
    };
    state.mouse_input(motion);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    assert_eq!(
        state.mouse_input(Mouse {
            button: MouseButton::Left,
            x,
            y
        }),
        None
    );
    assert!(
        !state.closed,
        "old card geometry must not activate after expansion"
    );
    state.layout(80, 15, true);
    state.mouse_input(motion);
    assert_eq!(state.selected.as_deref(), Some("p-other"));
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, hit)| *hit == Hit::Pane("p-other".into()))
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
fn agent_card_metadata_changes_reject_old_click_geometry() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    state.popup_size = Some((80, 15));
    state.layout(80, 15, true);
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, hit)| *hit == Hit::Pane("p-other".into()))
        .unwrap()
        .clone();
    state.rows[0].agent_metadata.title = Some("New expanded line".into());
    assert_eq!(
        state.mouse_input(Mouse {
            button: MouseButton::Left,
            x,
            y
        }),
        None
    );
    assert!(!state.closed);
}

#[test]
fn agent_wheel_records_stationary_pointer_even_without_previous_hover() {
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    state.popup_size = Some((80, 15));
    state.layout(80, 15, true);
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, hit)| *hit == Hit::Pane("p-other".into()))
        .unwrap()
        .clone();
    let selected = state.selected.clone();
    state.mouse_input(Mouse {
        button: MouseButton::WheelDown,
        x,
        y,
    });
    state.layout(80, 15, true);
    state.mouse_input(Mouse {
        button: MouseButton::Motion,
        x,
        y,
    });
    assert_eq!(state.selected, selected);
}

#[test]
fn agent_cards_reject_stale_selection_filter_status_and_resize_hits() {
    for change in 0..5 {
        let mut state = ui();
        state.set_table_view(TableView::Agent);
        state.popup_size = Some((80, 15));
        state.layout(80, 15, true);
        let (x, y, _, _) = state
            .mouse
            .borrow()
            .hits
            .iter()
            .find(|(_, _, _, hit)| *hit == Hit::Pane("p-other".into()))
            .unwrap()
            .clone();
        match change {
            0 => {
                state.key(Key::Down);
            }
            1 => {
                state.key(Key::Char('w'));
            }
            2 => {
                state.rows[0].status = "blocked".into();
                state.reconcile_agent_order();
            }
            3 => state.popup_size = Some((120, 30)),
            _ => state.query = "Claude".into(),
        }
        assert_eq!(
            state.mouse_input(Mouse {
                button: MouseButton::Left,
                x,
                y
            }),
            None
        );
        assert!(!state.closed);
    }
}

#[test]
fn agent_card_status_colors_and_full_width_selection_follow_every_theme() {
    for index in 0..crate::theme::NAMES.len() {
        for status in ["working", "blocked", "done", "idle", "unknown"] {
            let mut state = ui();
            state.set_table_view(TableView::Agent);
            state.theme_index = index;
            state.rows[0].status = status.into();
            let frame = state.terminal_frame(80, 15);
            assert!(
                frame.contains(&format!(
                    "{}{status}",
                    crate::presentation::sgr_fg(state.status_colors.themed(status, state.theme()))
                )),
                "{frame}"
            );
            let selected: Vec<_> = frame
                .split("\r\n")
                .filter(|line| line.contains("\x1b[0m"))
                .collect();
            assert_eq!(selected.len(), 2, "{frame}");
            for line in selected {
                assert_eq!(
                    super::without_sgr(&line.replace("\x1b[K", "")).cell_width(),
                    78,
                    "{line}"
                );
            }
        }
    }
    let mut state = ui();
    state.set_table_view(TableView::Agent);
    let frame = state.frame_width(80);
    for label in ["Repository:", "Branch:", "Titel:"] {
        assert!(!frame.contains(label), "absent metadata: {frame}");
    }
    state.rows[0].agent_metadata.title = Some("task\x1b[31m\nnext".into());
    let frame = state.terminal_frame(80, 15);
    assert!(frame.contains("Titel: task [31m next"));
    assert!(!frame.contains("\x1b[31m"));
}

#[test]
fn agent_variable_card_viewport_wheel_and_preview_preserve_selection_and_origin() {
    let rows = (0..12)
        .map(|i| {
            let mut row = ui().rows.remove(0);
            row.id = format!("card-{i}");
            row.agent = format!("Agent {i}");
            row.agent_metadata.title = Some(format!("Task {i}"));
            row.agent_metadata.repository = Some(("key".into(), "Repository".into()));
            row.agent_metadata.branch = Some("develop".into());
            row
        })
        .collect();
    let mut state = crate::selection::Ui::with_theme_choice(rows, "catppuccin", None);
    state.set_table_view(TableView::Agent);
    state.popup_size = Some((80, 15));
    state.layout(80, 15, true);
    state.rows[0].agent_metadata = Default::default();
    state.layout(80, 15, true);
    assert!(state
        .mouse
        .borrow()
        .hits
        .iter()
        .any(|(_, _, _, hit)| *hit == Hit::Pane("card-2".into())));
    let (x, y, _, _) = state
        .mouse
        .borrow()
        .hits
        .iter()
        .find(|(_, _, _, hit)| *hit == Hit::Pane("card-2".into()))
        .unwrap()
        .clone();
    state.mouse_input(Mouse {
        button: MouseButton::Motion,
        x,
        y,
    });
    let frame = state.layout(80, 15, true).join("\n");
    assert!(
        frame.contains("Titel: Task 2"),
        "hover must keep expanded card visible: {frame}"
    );
    let selected = state.selected.clone();
    let start = state.viewport_start.get();
    for _ in 0..40 {
        state.mouse_input(Mouse {
            button: MouseButton::WheelDown,
            x,
            y,
        });
        state.layout(80, 15, true);
        state.mouse_input(Mouse {
            button: MouseButton::Motion,
            x,
            y,
        });
        assert_eq!(state.selected, selected);
    }
    assert!(state.viewport_start.get() > start);
    assert_eq!(state.viewport_start.get(), state.mouse.borrow().scroll_max);
    state.key(Key::Down);
    let frame = state.layout(80, 15, true).join("\n");
    assert!(frame.contains("Titel: Task 3"), "{frame}");
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    let frame = state.layout(80, 15, true).join("\n");
    assert!(frame.contains("MODE: THEMES") && frame.contains("Titel: Task 3"));
    assert!(!frame.contains("Agent/Pane Name"));
    state.key(Key::Esc);
    state.key(Key::Char('s'));
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    assert!(state
        .layout(80, 15, true)
        .join("\n")
        .contains("Agent/Pane Name"));
}
