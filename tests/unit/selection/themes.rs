use crate::input::Key;
use crate::selection::{Mode, Ui};
use crate::test_support::ui;

#[test]
fn unsupported_auto_notice_is_transient_on_start_and_each_picker_auto_transition() {
    let mut state = ui();
    state.set_auto_choice("catppuccin", Some("catppuccin-mocha".into()));
    let selected = state.selected.clone();
    state.query = "Claude".into();
    let message = "Herdr theme catppuccin-mocha not yet supported; using Mocha";
    for _ in 0..2 {
        let frame = state.terminal_frame(80, 15);
        assert!(frame.contains(message), "{frame}");
        assert!(frame.contains("\x1b[48;2;24;24;37m"), "{frame}");
    }
    state.key(Key::Char('?'));
    assert!(!state.terminal_frame(80, 15).contains(message));
    state.key(Key::Esc);
    state.key(Key::Char('t'));
    assert!(state.terminal_frame(80, 15).contains(message));
    state.key(Key::Down);
    assert!(!state.terminal_frame(80, 15).contains(message));
    state.key(Key::Up);
    assert!(state.terminal_frame(80, 15).contains(message));
    state.key(Key::Char(' '));
    assert!(!state.terminal_frame(80, 15).contains(message));
    state.key(Key::Char(' '));
    assert!(!state.terminal_frame(80, 15).contains(message));
    assert_eq!(state.selected, selected);
    assert_eq!(state.query, "Claude");
    assert!(state.theme_error.is_none());
    assert!(!state.theme_invalid);
}

#[test]
fn theme_picker_replaces_panes_without_changing_navigation() {
    let mut state = ui();
    let selected = state.selected.clone();
    state.key(Key::Char('t'));
    let frame = state.terminal_frame(80, 15);
    assert!(frame.contains("Auto (Herdr)"), "{frame}");
    assert!(state.frame_width(80).contains("vesper"));
    assert!(!frame.contains("Agent/Pane Name"), "{frame}");
    state.key(Key::Down);
    state.key(Key::Esc);
    assert_eq!(state.selected, selected);
    assert_eq!(state.key(Key::Enter), selected);
}

#[test]
fn theme_picker_preview_cancel_and_search_t_preserve_pane_state() {
    let mut state = ui();
    state.key(Key::Char('/'));
    state.key(Key::Char('t'));
    assert_eq!(state.query, "t");
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Esc);
    let selected = state.selected.clone();
    let visible = state.visible();
    state.key(Key::Char('t'));
    state.key(Key::Down);
    state.key(Key::Down);
    assert_eq!(
        state.theme().accent,
        ratatui::style::Color::Rgb(30, 102, 245)
    );
    assert_eq!(state.visible(), visible);
    assert_eq!(state.query, "t");
    assert_eq!(state.selected, selected);
    state.key(Key::Esc);
    assert_eq!(state.theme_index, 0);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(state.selected, selected);
}

#[test]
fn theme_picker_failed_save_stays_open_and_reports_error_until_next_action() {
    let mut state = ui();
    state.key(Key::Char('t'));
    state.key(Key::Down);
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    assert_eq!(state.theme_index, 0);
    assert!(state.terminal_frame(80, 15).contains("not saved"));
    state.key(Key::Up);
    assert!(!state.terminal_frame(80, 15).contains("not saved"));
    state.key(Key::Esc);
    assert!(!state.closed);
}

#[test]
fn invalid_saved_theme_uses_mocha_even_if_auto_can_resolve_another_choice() {
    let mut state = ui();
    state.set_auto_choice("catppuccin", Some("catppuccin-mocha".into()));
    state.theme_invalid = true;
    state.theme_error = Some("theme.json invalid; using Mocha".into());
    assert_eq!(
        state.theme().working,
        ratatui::style::Color::Rgb(249, 226, 175)
    );
    assert!(state.terminal_frame(80, 15).contains("theme.json invalid"));
    assert!(!state.terminal_frame(80, 15).contains("not yet supported"));
    state.key(Key::Char('t'));
    assert!(state.theme_error.is_none());
    assert!(state.theme_invalid);
    assert!(!state.terminal_frame(80, 15).contains("not yet supported"));
    state.key(Key::Esc);
    assert!(!state.closed);
}

#[test]
fn picker_scrolls_to_last_theme_at_minimum_size_without_losing_keys() {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    state.key(Key::Char('t'));
    for _ in 0..30 {
        state.key(Key::Char('j'));
    }
    assert_eq!(state.theme_cursor, 18);
    let frame = state.terminal_frame(80, 15);
    assert!(frame.contains("vesper"), "{frame}");
    assert!(frame.contains("Auto (Herdr)"), "{frame}");
    assert!(frame.contains("Enter: save"), "{frame}");
    assert_eq!(frame.split("\r\n").count(), 15);
    state.key(Key::Up);
    assert_eq!(state.theme_cursor, 17);
    state.key(Key::Char('k'));
    assert_eq!(state.theme_cursor, 16);
}

#[test]
fn theme_preview_recolors_selected_row_search_match_and_status_in_both_layouts() {
    let mut state = ui();
    state.query = "Claude".into();
    state.key(Key::Char('t'));
    for _ in 0..6 {
        state.key(Key::Down);
    }
    for width in [80, 150] {
        let frame = state.terminal_frame(width, 20);
        assert!(frame.contains("dracula"), "{frame}");
        assert!(!frame.contains("Agent/Pane Name"));
    }
    state.key(Key::Esc);
    state.theme_index = 6;
    for width in [80, 150] {
        let frame = state.terminal_frame(width, 20);
        assert!(frame.contains("\x1b[48;2;70;63;93m"), "{frame}");
        assert!(frame.contains("\x1b[38;2;189;147;249m"), "{frame}");
        assert!(frame.contains("\x1b[38;2;241;250;140mworking"), "{frame}");
    }
}

#[test]
fn theme_space_switches_between_list_and_real_filtered_table_without_moving_pane() {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    state.query = "Claude".into();
    let selected = state.selected.clone();
    let visible = state.visible();
    state.key(Key::Char('t'));
    let list = state.terminal_frame(80, 15);
    assert!(
        list.contains("Auto (Herdr)") && !list.contains("Agent/Pane Name"),
        "{list}"
    );
    assert_complete_theme_footer(&state, &list, 80, 15, "table");
    state.key(Key::Char(' '));
    for key in [Key::Down, Key::Char('j'), Key::Up, Key::Char('k')] {
        state.key(key);
        assert_eq!(state.selected, selected);
        assert_eq!(state.query, "Claude");
        assert_eq!(state.visible(), visible);
        assert_eq!(state.mode, Mode::Themes);
    }
    for _ in 0..6 {
        state.key(Key::Down);
    }
    for (width, height) in [(80, 15), (150, 20)] {
        let frame = state.terminal_frame(width, height);
        assert_eq!(frame.split("\r\n").count(), height, "{frame}");
        assert!(
            frame.contains("preview: dracula") && frame.contains("Search: Claude"),
            "{frame}"
        );
        assert!(
            frame.contains("Agent/Pane Name") && !frame.contains("codex"),
            "{frame}"
        );
        assert!(frame.contains("\x1b[48;2;70;63;93m"), "{frame}");
        assert!(
            frame.contains("\x1b[38;2;189;147;249m") && frame.contains("\x1b[4m"),
            "{frame}"
        );
        assert!(frame.contains("\x1b[38;2;241;250;140mworking"), "{frame}");
        assert_complete_theme_footer(&state, &frame, width, height, "list");
    }
    state.key(Key::Char(' '));
    assert!(!state.terminal_frame(80, 15).contains("Agent/Pane Name"));
    assert_eq!(state.theme_cursor, 6);
    state.key(Key::Esc);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(state.selected, selected);
    assert_eq!(state.query, "Claude");
    assert_eq!(state.visible(), visible);
    assert_eq!(state.theme_index, 0);
}

#[test]
fn theme_table_preview_handles_empty_filter_bounds_and_failed_save() {
    let mut state = ui();
    state.popup_size = Some((80, 15));
    state.query = "no-such-pane".into();
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Up);
    assert_eq!(state.theme_cursor, 0);
    let empty = state.terminal_frame(80, 15);
    assert!(empty.contains("No matching panes"), "{empty}");
    assert_complete_theme_footer(&state, &empty, 80, 15, "list");
    for _ in 0..30 {
        state.key(Key::Char('j'));
    }
    assert_eq!(state.theme_cursor, 18);
    state.key(Key::Down);
    assert_eq!(state.theme_cursor, 18);
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    let failed = state.terminal_frame(80, 15);
    assert!(failed.contains("not saved"), "{failed}");
    assert_complete_theme_footer(&state, &failed, 80, 15, "list");
    state.key(Key::Char(' '));
    state.key(Key::Enter);
    let failed_list = state.terminal_frame(80, 15);
    assert_complete_theme_footer(&state, &failed_list, 80, 15, "table");
}

fn assert_complete_theme_footer(
    state: &Ui,
    frame: &str,
    width: usize,
    height: usize,
    toggle: &str,
) {
    let budget = state.hint_rows(crate::keymap::Context::Themes, width, height);
    assert!(!state.hint_more.get(), "{frame}");
    let lines: Vec<_> = frame.split("\r\n").collect();
    assert_eq!(lines.len(), height, "{frame}");
    // Footer text has no internal styling; remove only each row's ANSI paint envelope.
    let footer = lines[height - budget..]
        .iter()
        .map(|line| {
            line.split_once("  ")
                .map_or("", |(_, text)| text)
                .split('\x1b')
                .next()
                .unwrap()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("/ ", "/");
    assert_eq!(
        footer,
        format!("j/↓: ↓theme · k/↑: ↑theme · Space: {toggle} · Enter: save · Esc: cancel"),
        "{frame}"
    );
}

#[test]
fn saving_from_table_preview_keeps_pane_focus_and_reopens_in_list() {
    let dir = std::path::PathBuf::from(format!(
        "/tmp/opencode/crossroads-theme-preview-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut state = ui();
    state.config_dir = Some(dir.clone());
    state.query = "codex".into();
    let selected = state.selected.clone();
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Down);
    assert!(state.key(Key::Enter).is_none());
    assert_eq!(state.mode, Mode::Select);
    assert!(!state.closed);
    assert_eq!(state.theme_index, 1);
    assert_eq!(state.selected, selected);
    assert_eq!(state.query, "codex");
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.jsonc")).unwrap(),
        "{\"theme\":\"catppuccin\"}\n"
    );
    state.key(Key::Char('t'));
    assert!(!state.terminal_frame(80, 15).contains("Agent/Pane Name"));
    assert_eq!(state.theme_cursor, 1);
    std::fs::remove_dir_all(dir).unwrap();
}
