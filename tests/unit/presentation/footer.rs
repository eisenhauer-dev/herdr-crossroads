use super::ui;

#[test]
fn select_footer_uses_dot_groups_without_right_padding() {
    use ratatui::buffer::CellWidth;
    for width in [60, 76, 109, 110, 111, 119, 120, 121, 137, 138, 139, 150] {
        for height in [5, 15] {
            let state = ui();
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            let last = lines[height - 1];
            let budget = state.hint_rows(crate::keymap::Context::Select, width, height);
            let expected = state.hints(crate::keymap::Context::Select, width, budget, true);
            assert_eq!(
                &lines[height - budget..],
                expected.iter().map(String::as_str).collect::<Vec<_>>(),
                "{frame}"
            );
            assert!(!last.ends_with(' '), "{frame}");
            assert!((last.cell_width() as usize) < width, "{frame}");
            assert_eq!(
                expected
                    .iter()
                    .map(|line| line.matches('|').count())
                    .sum::<usize>(),
                lines[height - budget..]
                    .iter()
                    .map(|line| line.matches('|').count())
                    .sum::<usize>(),
                "{frame}"
            );
        }
    }
}

#[test]
fn compact_and_roomy_footers_show_navigation_with_final_help_group() {
    use ratatui::buffer::CellWidth;
    for width in [60, 76, 109, 110, 111, 119, 120, 121, 137, 138, 139, 150] {
        let state = ui();
        let frame = state.terminal_frame(width, 15);
        let footer = frame.split("\r\n").last().unwrap();
        let complete = state
            .hints(crate::keymap::Context::Select, width, 2, false)
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("/ ", "/");
        let hint_lines = state.hints(crate::keymap::Context::Select, width, 2, false);
        assert_eq!(
            complete.matches('·').count() + hint_lines.len() - 1,
            8,
            "{complete}"
        );
        for group in [
            "j/↓: ↓pane",
            "k/↑: ↑pane",
            "h/←: ↑tab",
            "l/→: ↓tab",
            "Tab: ↓ws",
            "Shift+Tab: ↑ws",
            "Enter: focus",
            "Esc: exit",
            "?: keys",
        ] {
            assert!(complete.contains(group), "{group}: {complete}");
        }
        assert_eq!(complete.matches('|').count(), 0, "{frame}");
        assert_eq!(complete.matches('↓').count(), 4, "{frame}");
        assert_eq!(complete.matches('↑').count(), 4, "{frame}");
        assert_eq!(complete.matches('→').count(), 1, "{complete}");
        assert_eq!(complete.matches('←').count(), 1, "{complete}");
        assert_eq!(
            footer,
            state
                .hints(
                    crate::keymap::Context::Select,
                    width,
                    state.hint_rows(crate::keymap::Context::Select, width, 15),
                    true
                )
                .last()
                .unwrap(),
            "{frame}"
        );
        assert!((footer.cell_width() as usize) < width, "{frame}");
        assert_eq!(frame.split("\r\n").count(), 15, "{frame}");
    }
}

#[test]
fn footer_tab_and_workspace_arrows_are_vertical_with_effective_keys() {
    use crate::keymap::{Context, Keymap};
    for custom in [false, true] {
        let mut state = ui();
        let expected = if custom {
            let (map, errors) = Keymap::parse(
                r#"{"select":{"previous_tab":["H"],"next_tab":["L"],"next_workspace":["N"],"previous_workspace":["P"]}}"#,
            );
            assert!(errors.is_empty(), "{errors:?}");
            state.keymap = map;
            ["H: ↑tab", "L: ↓tab", "N: ↓ws", "P: ↑ws"]
        } else {
            ["h/←: ↑tab", "l/→: ↓tab", "Tab: ↓ws", "Shift+Tab: ↑ws"]
        };
        let footer = state.hints(Context::Select, 240, 3, false).join(" ");
        for group in expected {
            assert!(footer.contains(group), "{group}: {footer}");
        }
    }
}

#[test]
fn custom_opening_actions_are_absent_from_select_and_help_footers() {
    use crate::keymap::{Context, Keymap};
    let mut state = ui();
    let (map, errors) = Keymap::parse(
        r#"{"select":{"open_search":["S"],"reset_search":["R"],"open_themes":["T"]},"help":{"open_search":["S"],"reset_search":["R"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    let select = state.hints(Context::Select, 240, 3, false).join(" ");
    let help = state.hints(Context::Help, 240, 3, false).join(" ");
    assert_eq!(
        (
            select.contains("S:"),
            select.contains("R:"),
            select.contains("T:"),
            help.contains("S:"),
            help.contains("R:")
        ),
        (false, false, false, false, false),
        "select={select}; help={help}"
    );
}

#[test]
fn default_hint_frames_are_complete_and_stable_at_compact_and_wide_boundaries() {
    for width in [80, 109, 110, 120, 145, 146, 180, 240] {
        for notice in [false, true] {
            let mut state = super::test_support::ui();
            state.popup_size = Some((width, 15));
            if notice {
                state.theme_error = Some("theme.jsonc invalid; using Mocha".into());
            }
            let first = state.layout(width, 15, true);
            assert!(
                !state.hint_more.get(),
                "defaults must not page at {width}: {first:?}"
            );
            assert_eq!(first.len(), 15);
            let footer = first
                .iter()
                .rev()
                .take_while(|line| !line.contains('─') && !line.contains("ERROR"))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let footer = footer
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace("/ ", "/");
            for binding in [
                "j/↓",
                "k/↑",
                "h/←",
                "l/→",
                "Tab",
                "Shift+Tab",
                "Enter: focus",
                "Esc: exit",
                "?: keys",
            ] {
                assert!(
                    footer.contains(binding),
                    "missing {binding} at {width}: {footer}"
                );
            }
            assert_eq!(footer.replace("  ", " ").replace(" · ", " "), "j/↓: ↓pane k/↑: ↑pane h/←: ↑tab l/→: ↓tab Tab: ↓ws Shift+Tab: ↑ws Enter: focus Esc: exit ?: keys");
            assert!(
                first
                    .iter()
                    .any(|line| line.contains("Claude") && line.contains("working")),
                "{first:?}"
            );
            assert_eq!(
                first.iter().filter(|line| line.contains("ERROR")).count(),
                usize::from(notice)
            );
            state.hint_scroll.set(1);
            assert_eq!(
                state.layout(width, 15, true),
                first,
                "default hints changed at {width}"
            );
        }
    }
}

#[test]
fn footer_keys_precede_actions_for_custom_bindings_in_every_context() {
    use crate::keymap::{Context, Keymap};
    let mut state = super::test_support::ui();
    let (map, errors) = Keymap::parse(
        r#"{
        "select":{"next_row":["J","Down"],"previous_row":["K","Up"],
                  "previous_tab":["H","Left"],"next_tab":["L","Right"],
                  "next_workspace":["N"],"previous_workspace":["P"]},
        "help":{"next_category":["J","Down"],"previous_category":["K","Up"]},
        "themes":{"next_theme":["J","Down"],"previous_theme":["K","Up"]}
    }"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    for (context, expected) in [
        (Context::Select, "J/↓: ↓pane K/↑: ↑pane H/←: ↑tab L/→: ↓tab N: ↓ws P: ↑ws Enter: focus Esc: exit ?: keys"),
        (Context::Help, "J/↓: ↓move K/↑: ↑move Esc: close keys"),
        (Context::Themes, "J/↓: ↓theme K/↑: ↑theme Space: table Enter: save Esc: cancel"),
    ] {
        let actual = state.hints(context, 80, 3, false).join(" ")
            .split_whitespace().collect::<Vec<_>>().join(" ").replace("/ ", "/");
        assert_eq!(actual.replace(" · ", " "), expected, "{context:?}");
    }
    state.theme_table_preview = true;
    assert!(state
        .hints(Context::Themes, 240, 3, false)
        .join(" ")
        .contains("Space: list"));
}

#[test]
fn separator_characters_are_quoted_only_for_display() {
    use crate::input::Key;
    use crate::keymap::{Context, Keymap};
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["/","|"],"open_search":["s"]},"help":{"next_category":["/","|"],"open_search":["s"]},"themes":{"next_theme":["/","|"]}}"#,
    );
    assert!(errors.is_empty());
    assert_eq!(map.labels(Context::Select, "next_row"), "//|");
    assert_eq!(
        map.action(Context::Select, Key::Char('|')),
        Some(crate::keymap::Action::NextRow)
    );
    let mut state = super::test_support::ui();
    state.keymap = map;
    for width in [80, 240] {
        for preview in [false, true] {
            state.theme_table_preview = preview;
            for (context, action, direction, separators) in [
                (Context::Select, "next_row", "pane", 8),
                (Context::Help, "next_category", "move", 2),
                (Context::Themes, "next_theme", "theme", 4),
            ] {
                let hints = state
                    .hints(context, width, 3, false)
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace("/ ", "/");
                assert!(
                    hints.contains(&format!("'/': ↓{direction} · k/↑: ↑{direction}")),
                    "{hints}"
                );
                assert!(!hints.contains(" | "), "{hints}");
                let lines = state.hints(context, width, 3, false);
                assert_eq!(
                    hints.matches('·').count() + lines.len() - 1,
                    separators,
                    "{hints}"
                );
                assert_eq!(
                    state
                        .keymap
                        .action(context, Key::Char('|'))
                        .map(crate::keymap::Action::name),
                    Some(action)
                );
            }
        }
    }
}

#[test]
fn footer_units_are_atomic_and_only_navigation_shows_second_arrows() {
    use crate::keymap::{Context, Keymap};
    let mut state = super::test_support::ui();
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["J","Left"],"previous_row":["Up","K"],"previous_tab":["H"],"next_tab":["L"],"next_workspace":["Tab","Right"],"focus_pane":["Enter","F"],"close":["Esc","X"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    state.keymap = map;
    let wide = state.hints(Context::Select, 240, 3, false).join(" ");
    assert!(wide.contains("J/←: ↓pane"), "{wide}");
    assert!(
        wide.contains("↑: ↑pane") && wide.contains("Tab/→: ↓ws"),
        "{wide}"
    );
    assert!(
        wide.contains("Enter: focus") && wide.contains("Esc: exit"),
        "{wide}"
    );
    assert!(
        !wide.contains("F:") && !wide.contains("X:") && !wide.contains("/K"),
        "{wide}"
    );
    for width in 80..=240 {
        let lines = state.hints(Context::Select, width, 3, false);
        for unit in wide.trim().split(" · ") {
            assert!(
                lines.iter().any(|line| line.contains(unit)),
                "split unit {unit} at {width}: {lines:?}"
            );
        }
        assert!(lines
            .iter()
            .all(|line| !line.trim().starts_with('·') && !line.trim().ends_with('·')));
    }
}

#[test]
fn footer_never_splits_default_action_units() {
    use crate::keymap::Context;
    let state = super::test_support::ui();
    let units = state.hints(Context::Select, 240, 3, false).join(" ");
    for width in 80..=240 {
        let lines = state.hints(Context::Select, width, 3, false);
        for unit in units.trim().split(" · ") {
            assert!(
                lines.iter().any(|line| line.contains(unit)),
                "split {unit} at {width}: {lines:?}"
            );
        }
    }
}
