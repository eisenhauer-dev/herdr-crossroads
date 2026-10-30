use crate::input::{Arrow, Key};
use crate::keymap::{Context, Keymap};
use crate::test_input::decode;
use crate::test_support::ui;

#[test]
fn recognized_modifiers_never_resolve_actions_search_or_focus() {
    use crate::selection::Mode;
    let mut modified = Vec::new();
    for c in 'a'..='z' {
        modified.extend([Key::Ctrl(c), Key::Alt(c)]);
    }
    for arrow in [Arrow::Up, Arrow::Down, Arrow::Left, Arrow::Right] {
        modified.extend([Key::CtrlArrow(arrow), Key::AltArrow(arrow)]);
    }
    modified.extend(decode(
        b"\x01\n\x08\x1bf\x1b[102;5u\x1b[109;5u\x1b[105;5u\x1b[1;5A\x1b[1;3B\x1b[57351;5u",
    ));
    for mode in [Mode::Select, Mode::Help, Mode::Themes, Mode::Search] {
        for help_search in [false, true] {
            let mut state = ui();
            state.key(Key::Char('?')); // Initialize effective help labels.
            state.mode = match mode {
                Mode::Select => Mode::Select,
                Mode::Help => Mode::Help,
                Mode::Themes => Mode::Themes,
                Mode::Search => Mode::Search,
            };
            state.help.searching = help_search;
            state.query = "kept".into();
            state.help.query = "kept".into();
            let selected = state.selected.clone();
            let help_selected = state.help.selected;
            let cursor = state.theme_cursor;
            for &key in &modified {
                for context in [Context::Select, Context::Help, Context::Themes] {
                    assert_eq!(state.keymap.action(context, key), None);
                }
                assert_eq!(state.key(key), None, "focus from {key:?}");
                assert_eq!(state.mode, mode);
                assert_eq!(state.selected, selected);
                assert_eq!(state.help.selected, help_selected);
                assert_eq!(state.theme_cursor, cursor);
                assert!(!state.theme_table_preview && !state.closed);
                assert_eq!(state.help.searching, help_search);
                assert_eq!(state.query, "kept");
                assert_eq!(state.help.query, "kept");
            }
        }
    }
}

#[test]
fn every_existing_action_dispatches_its_replacement_without_changing_pane_state() {
    use crate::selection::Mode;
    let (map, errors) = Keymap::parse(
        r#"{
        "select":{"next_row":["J"],"previous_row":["K"],"next_tab":["L"],"previous_tab":["H"],"next_workspace":["N"],"previous_workspace":["P"],"open_search":["S"],"reset_search":["R"],"open_help":["?"],"open_themes":["T"],"focus_pane":["F"],"close":["Q"]},
        "help":{"next_category":["J"],"previous_category":["K"],"open_search":["S"],"reset_search":["R"],"close":["Q"]},
        "themes":{"next_theme":["J"],"previous_theme":["K"],"toggle_preview":["P"],"save":["S"],"cancel":["Q"]}
    }"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    let mut state = ui();
    state.keymap = map;
    let ids = state
        .rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    for (key, id) in [
        (Key::Char('J'), "p-a"),
        (Key::Char('K'), "p-b"),
        (Key::Char('L'), "p-second-tab"),
        (Key::Char('H'), "p-b"),
        (Key::Char('N'), "p-other"),
        (Key::Char('P'), "p-b"),
    ] {
        assert!(state.key(key).is_none());
        assert_eq!(state.selected.as_deref(), Some(id));
    }
    assert!(
        state.key(Key::Enter).is_none(),
        "replaced focus default is inactive"
    );
    assert!(!state.closed);
    state.key(Key::Char('S'));
    assert_eq!(state.mode, Mode::Search);
    state.key(Key::Char('x'));
    state.key(Key::Enter);
    state.key(Key::Char('R'));
    assert!(state.query.is_empty());
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    state.key(Key::Down);
    assert_eq!(state.help.selected, Some(0));
    state.key(Key::Char('J'));
    assert_eq!(state.help.selected, Some(1));
    state.key(Key::Char('K'));
    assert_eq!(state.help.selected, Some(0));
    state.key(Key::Char('S'));
    assert!(state.help.searching);
    state.key(Key::Char('x'));
    state.key(Key::Enter);
    state.key(Key::Char('R'));
    assert!(state.help.query.is_empty());
    state.key(Key::Char('Q'));
    assert_eq!(state.mode, Mode::Select);
    assert!(!state.closed);
    state.key(Key::Char('T'));
    assert_eq!(state.mode, Mode::Themes);
    state.key(Key::Char('J'));
    assert_eq!(state.theme_cursor, 1);
    state.key(Key::Char('K'));
    assert_eq!(state.theme_cursor, 0);
    state.key(Key::Char('P'));
    assert!(state.theme_table_preview);
    state.key(Key::Char('Q'));
    assert_eq!(state.mode, Mode::Select);
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-remapped-save-{}", std::process::id()));
    state.config_dir = Some(dir.clone());
    state.key(Key::Char('T'));
    state.key(Key::Char('J'));
    state.key(Key::Char('S'));
    assert_eq!(state.theme_index, 1);
    assert_eq!(state.mode, Mode::Select);
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.jsonc")).unwrap(),
        "{\"theme\":\"catppuccin\"}\n"
    );
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        state
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(state.key(Key::Char('F')).as_deref(), Some("p-b"));
    assert!(state.closed);
    state.closed = false;
    assert_eq!(state.key(Key::Char('Q')), None);
    assert!(state.closed);
}
#[test]
fn help_enter_restores_origin_and_executes_the_marked_action() {
    use crate::{input::Key, selection::Mode, test_support::ui};
    let mut state = ui();
    state.key(Key::Char('?'));
    let before = state.selected.clone();
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Select);
    assert_ne!(state.selected, before);
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    state.key(Key::Char('?'));
    state.key(Key::Enter);
    assert_eq!(state.mode, Mode::Themes);
    assert!(state.theme_table_preview);
    assert_eq!(state.theme_cursor, 1);
}
