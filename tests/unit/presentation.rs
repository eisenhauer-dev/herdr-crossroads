use super::test_support::*;
use crate::input::Key;
use crate::presentation::{clipped, marked};
use crate::selection::{Mode, Ui as ProductionUi};
use crate::status_colors::StatusColors;
use crate::test_support;
use crate::theme::NAMES;
use ratatui::{
    buffer::CellWidth,
    style::{Color, Modifier},
};

#[test]
fn footers_omit_opening_actions_but_preserve_full_keys_and_search_controls() {
    use crate::keymap::{Context, Keymap};
    for custom in [false, true] {
        let mut state = ui();
        let (search, reset, themes, labels) = if custom {
            let (map, errors) = Keymap::parse(
                r#"{"select":{"open_search":["S"],"reset_search":["R"],"open_themes":["T"]},"help":{"open_search":["S"],"reset_search":["R"]}}"#,
            );
            assert!(errors.is_empty(), "{errors:?}");
            state.keymap = map;
            (
                Key::Char('S'),
                Key::Char('R'),
                Key::Char('T'),
                ["S", "R", "T"],
            )
        } else {
            (
                Key::Char('f'),
                Key::Char('r'),
                Key::Char('t'),
                ["f", "r", "t"],
            )
        };
        for context in [Context::Select, Context::Help] {
            for width in [80, 120, 240] {
                let footer = state.hints(context, width, 3, false).join(" ");
                for action in ["search", "reset", "theme"] {
                    assert!(!footer.contains(action), "{context:?}: {footer}");
                }
                for label in &labels[..if context == Context::Help { 2 } else { 3 }] {
                    assert!(
                        !footer
                            .split_whitespace()
                            .any(|token| token == format!("{label}:")),
                        "{footer}"
                    );
                }
            }
        }
        state.key(search);
        assert_eq!(state.mode, Mode::Search);
        assert!(state
            .frame_width(240)
            .contains("Enter/Esc: select  Backspace: erase"));
        state.key(Key::Char('x'));
        state.key(Key::Enter);
        state.key(reset);
        assert!(state.query.is_empty());
        state.key(themes);
        assert_eq!(state.mode, Mode::Themes);
        state.key(Key::Esc);
        state.key(Key::Char('?'));
        for (label, meaning) in labels
            .into_iter()
            .zip(["search", "reset search", "choose theme"])
        {
            assert!(state
                .help
                .entries
                .iter()
                .any(|entry| entry.keys == label && entry.meaning == meaning));
            assert!(state
                .layout(240, 30, false)
                .iter()
                .any(|line| line.contains(label) && line.ends_with(meaning)));
        }
        for (keys, meaning) in [
            ("h/←", "previous tab"),
            ("l/→", "next tab"),
            ("Tab", "next workspace"),
            ("Shift+Tab", "previous workspace"),
        ] {
            assert!(state
                .help
                .entries
                .iter()
                .any(|entry| entry.keys == keys && entry.meaning == meaning));
        }
        state.key(search);
        assert!(state.help.searching);
        assert!(state
            .frame_width(240)
            .contains("Enter/Esc: return to keys · Backspace: erase"));
        state.key(Key::Char('x'));
        state.key(Key::Enter);
        state.key(reset);
        assert!(state.help.query.is_empty());
    }
}

#[test]
fn all_status_defaults_and_partial_override_are_read_once() {
    let colors = StatusColors::default();
    for (status, expected) in [
        ("working", Color::Rgb(249, 226, 175)),
        ("blocked", Color::Rgb(243, 139, 168)),
        ("done", Color::Rgb(148, 226, 213)),
        ("idle", Color::Rgb(166, 227, 161)),
        ("unknown", Color::Rgb(108, 112, 134)),
        ("unrecognized", Color::Rgb(108, 112, 134)),
    ] {
        assert_eq!(colors.for_status(status), expected);
    }
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-palette-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("status-colors.json");
    assert_eq!(
        StatusColors::load(Some(&dir)).unwrap().working,
        colors.working
    );
    std::fs::write(&path, r##"{"working":"#123abc","unknown":"#010203"}"##).unwrap();
    let loaded = StatusColors::load(Some(&dir)).unwrap();
    assert_eq!(loaded.working, Color::Rgb(18, 58, 188));
    assert_eq!(loaded.unknown, Color::Rgb(1, 2, 3));
    assert_eq!(loaded.idle, colors.idle);
    let mut state = ui();
    state.status_colors = loaded;
    std::fs::write(&path, r##"{"working":"#FFFFFF"}"##).unwrap();
    for width in [60, 150] {
        let rendered = state.terminal_frame(width, 15);
        assert!(
            rendered.contains("\x1b[38;2;18;58;188mworking\x1b[39m"),
            "{rendered}"
        );
        assert!(!rendered.contains("\x1b[38;2;255;255;255m"), "{rendered}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn search_hint_appears_in_navigation_with_empty_query_not_input() {
    let mut state = ui();
    for width in [76, 120] {
        assert_eq!(
            state.terminal_frame(width, 8).split("\r\n").next(),
            Some(format!("  Search: f to search{}Select", " ".repeat(width - 29)).as_str())
        );
    }
    state.key(Key::Char('f'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: {}Search", " ".repeat(102)).as_str())
    );
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: codex{}Search", " ".repeat(97)).as_str())
    );
    state.key(Key::Enter);
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: codex{}Select", " ".repeat(97)).as_str())
    );
    state.key(Key::Char('r'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: f to search{}Select", " ".repeat(91)).as_str())
    );
    state.key(Key::Char('?'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: f to search{}Help", " ".repeat(93)).as_str())
    );
    state.key(Key::Char('f'));
    assert_eq!(
        state.terminal_frame(120, 8).split("\r\n").next(),
        Some(format!("  Search: {}Help", " ".repeat(104)).as_str())
    );
}

#[test]
fn hint_actions_and_literal_slash_are_unambiguous_without_identity_changes() {
    use crate::input::Key;
    use crate::keymap::{Context, Keymap};
    let mut state = super::test_support::ui();
    let select = state.hints(Context::Select, 240, 3, false).join(" ");
    assert!(select.contains("j/↓: ↓pane · k/↑: ↑pane"), "{select}");
    assert!(select.contains("h/←: ↑tab · l/→: ↓tab"), "{select}");
    assert!(select.contains("Tab: ↓ws · Shift+Tab: ↑ws"), "{select}");
    for (context, action) in [(Context::Help, "move"), (Context::Themes, "theme")] {
        let text = state.hints(context, 240, 3, false).join(" ");
        assert!(
            text.contains(&format!("j/↓: ↓{action} · k/↑: ↑{action}")),
            "{text}"
        );
    }
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["/","J"],"open_search":["s"]},"themes":{"toggle_preview":["/","p"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(map.labels(Context::Select, "next_row"), "//J");
    assert_eq!(
        map.action(Context::Select, Key::Char('/')),
        Some(crate::keymap::Action::NextRow)
    );
    state.keymap = map;
    assert!(state
        .hints(Context::Select, 240, 3, false)
        .join(" ")
        .contains("'/': ↓pane · k/↑: ↑pane"));
    assert!(state
        .hints(Context::Themes, 240, 3, false)
        .join(" ")
        .contains("'/': table"));
    state.key(Key::Char('?'));
    let help = state.frame_width(240);
    assert!(
        help.contains("'/'/J")
            && help.contains("next workspace")
            && help.contains("previous workspace"),
        "{help}"
    );
}

#[test]
fn all_contexts_keep_defaults_stable_and_oversized_custom_lists_are_errors() {
    use crate::keymap::{Context, Keymap};
    use crate::selection::Mode;
    for (context, mode, preview) in [
        (Context::Select, Mode::Select, false),
        (Context::Help, Mode::Help, false),
        (Context::Themes, Mode::Themes, false),
        (Context::Themes, Mode::Themes, true),
    ] {
        let mut state = super::test_support::ui();
        state.mode = mode;
        state.theme_table_preview = preview;
        for width in [80, 240] {
            let first = state.layout(width, 15, true);
            assert!(!state.hint_more.get(), "{context:?}: {first:?}");
            let footer = state.hints(context, width, 3, false).join(" ");
            assert!(
                footer.contains(" · ") && !footer.contains(" | "),
                "{footer}"
            );
            state.hint_scroll.set(1);
            assert_eq!(state.layout(width, 15, true), first);
        }
    }
    let aliases: Vec<_> = ["/".to_string(), "|".to_string()]
        .into_iter()
        .chain(('A'..='Z').map(|c| c.to_string()))
        .collect();
    let previous_aliases: Vec<_> = ('0'..='9').map(|c| c.to_string()).collect();
    let (map, errors) = Keymap::parse(&serde_json::json!({"select":{"next_row":aliases,"previous_row":previous_aliases,"open_search":["s"]},"help":{"next_category":aliases,"previous_category":previous_aliases,"open_search":["s"]},"themes":{"next_theme":aliases,"previous_theme":previous_aliases}}).to_string());
    assert_eq!(errors, [crate::keymap::KEY_LIMIT_HINT]);
    assert_eq!(map.oversized_actions.len(), 6);
    assert_eq!(map.diagnostic_blocks().len(), 3);
    assert!(map
        .diagnostic_blocks()
        .iter()
        .all(
            |(heading, details)| heading.starts_with("Maximum 2 keybinds per action - ")
                && details.len() == 2
        ));
    for (context, action) in [
        ("select", "next_row"),
        ("select", "previous_row"),
        ("help", "next_category"),
        ("help", "previous_category"),
        ("themes", "next_theme"),
        ("themes", "previous_theme"),
    ] {
        assert!(
            map.oversized_actions
                .iter()
                .any(|entry| entry.context == context && entry.action == action),
            "{context}.{action} missing"
        );
    }
    let mut state = super::test_support::ui();
    state.keymap = map;
    state.keymap_errors = errors;
    for (context, mode, preview) in [
        (Context::Select, Mode::Select, false),
        (Context::Help, Mode::Help, false),
        (Context::Themes, Mode::Themes, false),
        (Context::Themes, Mode::Themes, true),
    ] {
        state.mode = mode;
        state.theme_table_preview = preview;
        let frame = state.layout(80, 15, true).join("\n");
        assert!(
            frame.contains("Maximum 2 keybinds per action - ")
                && !frame.contains("Context |")
                && frame.contains("ERROR: Maximum 2 keybinds per action - themes mode")
                && frame.contains("previous_theme"),
            "{context:?}: {frame}"
        );
        assert!(!frame.contains("Agent/Pane Name"));
    }
}

#[test]
fn all_displayed_arrows_are_symbols_but_identity_is_canonical() {
    use crate::keymap::{Context, Keymap};
    for (syntax, display) in [("Up", "↑"), ("Down", "↓"), ("Left", "←"), ("Right", "→")] {
        let (map, errors) = Keymap::parse(&format!(
            r#"{{"select":{{"open_help":["{syntax}"],"next_row":["J"],"previous_row":["K"],"previous_tab":["H"],"next_tab":["L"]}}}}"#
        ));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(map.labels(Context::Select, "open_help"), syntax);
        assert_eq!(map.display_labels(Context::Select, "open_help"), display);
    }
    let mut state = super::test_support::ui();
    (state.keymap, state.keymap_errors) = Keymap::parse(r#"{"select":{"bad_config":null}}"#);
    assert!(state
        .frame_width(80)
        .contains("↑/k: scroll up · ↓/j: scroll down"));
}

#[path = "presentation/diagnostics.rs"]
mod diagnostics;
#[path = "presentation/footer.rs"]
mod footer;
#[path = "presentation/help.rs"]
mod help;
#[path = "presentation/table.rs"]
mod table;
#[path = "presentation/themes.rs"]
mod themes;

#[test]
fn mocha_paints_panel_and_text_in_table_and_picker_while_terminal_inherits_defaults() {
    use crate::selection::{Mode, Ui};
    use crate::test_support::ui;

    let mut state: Ui = ui();
    state.theme_index = 1;
    for (mode, table) in [
        (Mode::Select, false),
        (Mode::Themes, false),
        (Mode::Themes, true),
    ] {
        state.mode = mode;
        state.theme_cursor = 1;
        state.theme_table_preview = table;
        let mocha = state.terminal_frame(80, 15);
        let lines: Vec<_> = mocha.split("\r\n").collect();
        assert_eq!(lines.len(), 15);
        assert!(
            lines.iter().all(|line| {
                line.starts_with("\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m")
                    && line.ends_with("\x1b[48;2;24;24;37m\x1b[K")
            }),
            "{mocha:?}"
        );
        assert!(
            mocha.contains(if state.mode == Mode::Themes && !table {
                "\x1b[48;2;49;50;68m"
            } else {
                "\x1b[48;5;236m"
            }),
            "{mocha:?}"
        );

        state.theme_index = 3;
        state.theme_cursor = 3;
        let terminal = state.terminal_frame(80, 15);
        assert!(!terminal.contains("\x1b[48;2;24;24;37m"), "{terminal:?}");
        assert!(!terminal.contains("\x1b[38;2;205;214;244m"), "{terminal:?}");
        assert!(!terminal.contains("\x1b[K"), "{terminal:?}");
        assert!(terminal.contains("\x1b[49;7m"), "{terminal:?}");
        state.theme_index = 1;
    }
}

#[test]
fn rendered_theme_frames_cover_all_rows_without_leaking_selected_background() {
    use crate::selection::Ui;
    use crate::test_support::ui;

    let mut state: Ui = ui();
    for (name, background) in [
        ("catppuccin-latte", "\x1b[48;2;239;241;245m"),
        ("dracula", "\x1b[48;2;40;42;54m"),
    ] {
        state.theme_index = NAMES.iter().position(|entry| *entry == name).unwrap();
        let frame = state.terminal_frame(80, 15);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 15);
        assert!(
            lines
                .iter()
                .all(|line| line.starts_with(background) && line.ends_with("\x1b[K")),
            "{name}: {frame:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("\x1b[0m") && line.contains("\x1b[48;2;")),
            "{frame:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!("\x1b[0m{background}\x1b[K"))),
            "{frame:?}"
        );

        state.mode = crate::selection::Mode::Themes;
        state.theme_cursor = state.theme_index;
        for table in [false, true] {
            state.theme_table_preview = table;
            let frame = state.terminal_frame(80, 15);
            assert!(
                frame
                    .split("\r\n")
                    .all(|line| line.starts_with(background) && line.ends_with("\x1b[K")),
                "{frame:?}"
            );
        }
        state.mode = crate::selection::Mode::Select;
    }

    state.theme_index = NAMES.iter().position(|entry| *entry == "terminal").unwrap();
    state.theme_cursor = state.theme_index;
    state.mode = crate::selection::Mode::Themes;
    state.theme_table_preview = false;
    let frame = state.terminal_frame(80, 15);
    assert!(
        !frame.contains("\x1b[48;2;") && !frame.contains("\x1b[38;2;"),
        "{frame:?}"
    );
    assert!(
        frame.contains("\x1b[49;7m") && frame.contains("\x1b[0m"),
        "{frame:?}"
    );

    state.theme_index = 1;
    state.theme_cursor = 1;
    let mocha = state.terminal_frame(80, 15);
    assert!(
        mocha.split("\r\n").all(|line| line
            .starts_with("\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m")
            && line.ends_with("\x1b[48;2;24;24;37m\x1b[K"))
            && mocha.contains("\x1b[48;2;49;50;68m"),
        "{mocha:?}"
    );
}

#[test]
fn help_and_undersized_notice_paint_full_height_except_terminal_theme() {
    use crate::selection::{Mode, Ui};
    use crate::test_support::ui;

    let mut state: Ui = ui();
    state.popup_size = Some((80, 15));
    for (name, background) in [
        ("catppuccin", "\x1b[48;2;24;24;37m"),
        ("dracula", "\x1b[48;2;40;42;54m"),
    ] {
        state.theme_index = NAMES.iter().position(|entry| *entry == name).unwrap();
        state.mode = Mode::Help;
        let help = state.terminal_frame(80, 15);
        assert_eq!(help.split("\r\n").count(), 15, "{name}: {help:?}");
        assert!(
            help.split("\r\n")
                .all(|line| line.starts_with(background) && line.ends_with("\x1b[K")),
            "{name}: {help:?}"
        );

        state.popup_size = Some((79, 15));
        let notice = state.terminal_frame(79, 15);
        assert_eq!(notice.split("\r\n").count(), 15, "{name}: {notice:?}");
        assert!(
            notice
                .split("\r\n")
                .all(|line| line.starts_with(background) && line.ends_with("\x1b[K")),
            "{name}: {notice:?}"
        );
        assert!(notice
            .split("\r\n")
            .nth(3)
            .unwrap()
            .contains("Press Enter/Esc/q to close."));
    }

    state.theme_index = NAMES.iter().position(|entry| *entry == "terminal").unwrap();
    let notice = state.terminal_frame(79, 15);
    assert_eq!(notice.split("\r\n").count(), 4);
    assert!(!notice.contains("\x1b[K"));
}

#[test]
fn notices_are_separate_styled_rows_above_unchanged_keys_at_minimum_and_wide_sizes() {
    use crate::selection::Mode;
    use crate::test_support::ui;

    for height in [15, 24] {
        for (width, mode, table) in [
            (80, Mode::Select, false),
            (120, Mode::Select, false),
            (80, Mode::Themes, false),
            (120, Mode::Themes, false),
            (80, Mode::Themes, true),
            (120, Mode::Themes, true),
        ] {
            let mut state = ui();
            state.theme_index = 1;
            state.theme_cursor = 1;
            state.mode = mode;
            state.theme_table_preview = table;
            state.theme_error = None;
            state.auto_toast = None;
            let unchanged_frame = state.terminal_frame(width, height);
            let unchanged_lines: Vec<_> = unchanged_frame.split("\r\n").collect();
            let help_rows = state.hint_rows(
                if state.mode == Mode::Select {
                    crate::keymap::Context::Select
                } else {
                    crate::keymap::Context::Themes
                },
                width,
                height,
            );
            for error in [true, false] {
                state.theme_error = error.then(|| "theme.json invalid; using Mocha".into());
                state.auto_toast = (!error).then(|| "Herdr theme unknown; using Mocha".into());
                let frame = state.terminal_frame(width, height);
                let lines: Vec<_> = frame.split("\r\n").collect();
                assert_eq!(lines.len(), height, "{frame:?}");
                let notice = &lines[height - help_rows - 1];
                assert!(
                    notice.contains(if error { "ERROR" } else { "INFO" }),
                    "{frame:?}"
                );
                assert!(
                    notice.contains(if error {
                        "theme.json invalid"
                    } else {
                        "Herdr theme unknown"
                    }),
                    "{frame:?}"
                );
                assert!(
                    notice.contains(if error {
                        "\x1b[38;2;243;139;168m"
                    } else {
                        "\x1b[38;2;108;112;134m"
                    }),
                    "{frame:?}"
                );
                assert!(notice.contains("\x1b[1m") == error, "{frame:?}");
                assert_eq!(
                    &lines[height - help_rows..],
                    &unchanged_lines[height - help_rows..],
                    "notice must precede the complete unchanged key help: {frame:?}"
                );
                let context = if state.mode == Mode::Select {
                    crate::keymap::Context::Select
                } else {
                    crate::keymap::Context::Themes
                };
                let hints = state.hints(context, width, help_rows, true);
                for (line, hint) in lines[height - help_rows..].iter().zip(hints) {
                    assert!(line.contains(&hint), "{frame:?}");
                }
            }
        }
    }
}

// These layout assertions predate theme painting. Compare their original ANSI
// content while the theme frame test checks the actual painted output.
struct Ui(ProductionUi);

impl Ui {
    fn new(rows: Vec<crate::topology::Row>) -> Self {
        Self(ProductionUi::with_theme_choice(rows, "catppuccin", None))
    }

    fn terminal_frame(&self, width: usize, height: usize) -> String {
        let frame = self.0.terminal_frame(width, height);
        frame
            .replace("\x1b[48;2;24;24;37m\x1b[38;2;205;214;244m", "")
            .replace("\x1b[48;2;24;24;37m\x1b[K", "")
            .replace("\x1b[38;2;205;214;244m", "\x1b[39m")
    }
}

impl std::ops::Deref for Ui {
    type Target = ProductionUi;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Ui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn ui() -> Ui {
    Ui(super::test_support::ui())
}

pub(super) fn without_sgr(line: &str) -> String {
    line.split('\x1b')
        .enumerate()
        .map(|(index, part)| {
            if index == 0 {
                part
            } else {
                part.split_once('m').unwrap().1
            }
        })
        .collect()
}

#[test]
fn marked_distinguishes_generated_from_source_ellipsis() {
    let generated = marked("abcde", 4, &[2], true);
    assert_eq!(
        generated
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "ab…"
    );
    assert!(generated
        .spans
        .iter()
        .all(|span| !span.style.add_modifier.contains(Modifier::UNDERLINED)));

    let source = marked("ab…", 4, &[2], true);
    assert!(source
        .spans
        .iter()
        .any(|span| span.content == "…" && span.style.add_modifier.contains(Modifier::UNDERLINED)));
}

#[test]
fn marked_keeps_combining_and_joined_graphemes_in_single_spans() {
    for glyph in ["e\u{301}", "👩\u{200d}💻"] {
        for positions in [&[][..], &[0][..], &[1][..]] {
            let line = marked(glyph, 20, positions, true);
            assert_eq!(line.spans.len(), 1, "{glyph:?}: {line:?}");
            assert_eq!(line.spans[0].content, glyph);
            assert_eq!(
                line.spans[0]
                    .style
                    .add_modifier
                    .contains(Modifier::UNDERLINED),
                !positions.is_empty()
            );
        }
        assert_eq!(clipped(&format!("{glyph}abc"), 4), "…");
    }
}

#[test]
fn undersized_notice_starts_error_and_matches_theme_error_style_at_tiny_heights() {
    for theme_index in [1, 2, 3] {
        let mut state = super::test_support::ui();
        state.theme_index = theme_index;
        state.theme_error = Some("unknown theme".into());
        let notice_frame = state.terminal_frame(100, 15);
        let notice = notice_frame
            .split("\r\n")
            .find(|line| line.contains("ERROR"))
            .unwrap();
        let start = notice.find("\x1b[1m").unwrap();
        let style = &notice[start..notice.find("ERROR").unwrap()];
        let end = notice.find("\x1b[22m").unwrap();
        let reset_end = notice[end + 5..].find('m').unwrap() + end + 6;
        let reset = &notice[end..reset_end];
        state.theme_error = None;
        for (width, height) in [(79, 15), (80, 14), (40, 2), (40, 1)] {
            state.popup_size = Some((width, height));
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert!(lines[0].contains(&format!("{style}ERROR:")), "{frame:?}");
            assert!(lines[0].contains(reset), "{frame:?}");
            assert!(!lines[0].contains("\x1b[0m"), "{frame:?}");
            assert!(frame.contains(&format!("{width}x{height}")), "{frame:?}");
            assert!(frame.contains("80x15"), "{frame:?}");
            if height > 1 {
                assert!(frame.contains("reopen"), "{frame:?}");
                assert!(!lines[1].contains("\x1b[1m"), "{frame:?}");
            }
            assert!(lines.len() <= height);
            let plain = state.frame_width(width);
            assert!(plain.starts_with("ERROR:"), "{plain}");
            assert!(!plain.contains('\x1b'), "{plain:?}");
            assert!(plain.contains(&format!("{width}x{height}")), "{plain}");
            assert!(plain.contains("80x15"), "{plain}");
        }
    }
}

#[test]
fn undersized_popup_displays_only_notice_and_ignores_keys_without_losing_state() {
    let plain_line = |line: &str| -> String {
        line.split('\x1b')
            .enumerate()
            .map(|(at, part)| {
                if at == 0 {
                    part
                } else {
                    part.split_once('m').unwrap().1
                }
            })
            .collect()
    };
    let mut state = ui();
    state.theme_index = 3; // Terminal theme retains its default background.
    state.key(Key::Down);
    state.key(Key::Char('f'));
    for c in "codex".chars() {
        state.key(Key::Char(c));
    }
    state.key(Key::Enter);
    let selected = state.selected.clone();
    let query = state.query.clone();
    let ids: Vec<_> = state.rows.iter().map(|row| row.id.clone()).collect();
    for (width, height) in [(67, 12), (79, 15), (80, 14)] {
        state.popup_size = Some((width, height));
        let piped = state.frame_width(width);
        assert!(piped.starts_with("ERROR:") && !piped.contains("Search: codex"));
        let frame = state.terminal_frame(width, height);
        assert_eq!(
            frame.split("\r\n").map(plain_line).collect::<Vec<_>>(),
            [
                "ERROR: Crossroads needs minimum content size 80x15.".to_owned(),
                format!("  Your content size is: {width}x{height}."),
                "  Set larger size in herdr plugin manifest and reopen.".to_owned(),
                "  Press Enter/Esc/q to close.".to_owned(),
            ],
            "{frame}"
        );
        assert!(
            !frame.contains("Search:") && !frame.contains("Claude") && !frame.contains("focus"),
            "{frame}"
        );
        for key in [
            Key::Char('f'),
            Key::Down,
            Key::Up,
            Key::Char('j'),
            Key::Char('k'),
            Key::Char('?'),
        ] {
            assert!(state.key(key).is_none());
            assert!(!state.closed);
        }
        assert_eq!(state.query, query);
        assert_eq!(state.selected, selected);
        assert_eq!(
            state
                .rows
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
    }
    state.popup_size = Some((80, 15));
    assert!(state.terminal_frame(80, 15).contains("Search: codex"));
    assert_eq!(state.query, query);
    assert_eq!(state.selected, selected);
    for key in [Key::Enter, Key::Esc, Key::Char('q')] {
        state.closed = false;
        state.popup_size = Some((79, 15));
        assert!(state.key(key).is_none(), "must not request focus");
        assert!(state.closed);
    }
    for width in [1, 2, 20] {
        state.popup_size = Some((width, 12));
        let frame = state.terminal_frame(width, 12);
        assert_eq!(frame.split("\r\n").count(), 4, "{frame}");
        assert!(frame
            .split("\r\n")
            .all(|line| usize::from(plain_line(line).cell_width()) < width));
    }
}

#[test]
fn undersized_popup_enter_closes_without_requesting_focus() {
    let mut state = ui();
    state.popup_size = Some((79, 15));
    assert!(state.key(Key::Enter).is_none());
    assert!(state.closed);
}

#[test]
fn titleless_layout_preserves_selection_filters_and_short_viewport() {
    for width in [60, 150] {
        let mut state = ui();
        for mode in [Mode::Select, Mode::Search, Mode::Help] {
            let search = mode == Mode::Search;
            let help = mode == Mode::Help;
            state.mode = mode;
            let frame = state.terminal_frame(width, 15);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), 15, "{frame}");
            assert!(
                lines[0].starts_with(if search || help {
                    "  Search: "
                } else {
                    "  Search: f to search"
                }),
                "{frame}"
            );
            assert!(!frame.contains("Herdr Crossroads"), "{frame}");
            if !help {
                assert!(lines[1].starts_with("  ─"), "{frame}");
                assert!(
                    lines[2].contains(if width == 60 {
                        "Agent/Pane"
                    } else {
                        "Agent/Pane Name"
                    }),
                    "{frame}"
                );
                assert!(
                    lines[4].contains(if width == 60 { "C \x1b[2m|" } else { "Claude" }),
                    "{frame}"
                );
            }
        }
        state.mode = Mode::Select;
        state.query = "nomatch".into();
        let empty = state.terminal_frame(width, 15);
        assert!(
            empty.contains("No matching panes") && !empty.contains("Herdr Crossroads"),
            "{empty}"
        );
        state.query.clear();
        state.selected = Some("p-other".into());
        let short = state.terminal_frame(width, 5);
        let lines: Vec<_> = short.split("\r\n").collect();
        assert_eq!(lines.len(), 5, "{short}");
        assert!(
            lines[0].starts_with("  Search:") && lines[1].starts_with("  ─"),
            "{short}"
        );
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains(if width == 60 { "idle" } else { "codex" })),
            "{short}"
        );
        assert!(!short.contains('┄'), "{short}");
    }
}

#[test]
fn terminal_footer_stays_on_bottom_row_for_short_content() {
    for width in [76, 120] {
        let mut state = ui();
        for height in [5, 16] {
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height, "{frame}");
            assert!(
                lines[height - 1]
                    == state
                        .hints(
                            crate::keymap::Context::Select,
                            width,
                            state.hint_rows(crate::keymap::Context::Select, width, height),
                            true
                        )
                        .last()
                        .unwrap(),
                "{frame}"
            );
            assert!(
                lines.iter().all(|line| without_status_ansi(line)
                    .replace("\x1b[48;5;236m", "")
                    .replace("\x1b[0m", "")
                    .replace("\x1b[2m", "")
                    .replace("\x1b[22m", "")
                    .chars()
                    .count()
                    < width),
                "{frame}"
            );
        }
        state.query = "not found".into();
        let empty = state.terminal_frame(width, 16);
        assert!(empty.contains("No matching panes"), "{empty}");
        assert_eq!(empty.split("\r\n").count(), 16, "{empty}");
        state.mode = Mode::Help;
        let help = state.terminal_frame(width, 16);
        assert_eq!(help.split("\r\n").count(), 16, "{help}");
        assert!(
            help.ends_with(
                state
                    .hints(
                        crate::keymap::Context::Help,
                        width,
                        state.hint_rows(crate::keymap::Context::Help, width, 16),
                        true
                    )
                    .join("\r\n")
                    .as_str()
            ),
            "{help}"
        );
    }
}
#[test]
fn mode_badge_reserves_query_width_and_reports_theme_table_preview_and_help_search() {
    for (width, height) in [(80, 15), (142, 30), (240, 40)] {
        for (mode, badge, preview) in [
            (Mode::Select, "Select", false),
            (Mode::Search, "Search", false),
            (Mode::Help, "Help", false),
            (Mode::Themes, "Themes", true),
        ] {
            let mut state = super::test_support::ui();
            state.mode = mode;
            state.theme_table_preview = preview;
            state.help.searching = true;
            for query in ["", "界👩\u{200d}💻", &"界".repeat(300)] {
                state.query = query.into();
                state.help.query = query.into();
                for ansi in [false, true] {
                    let lines = state.layout(width, height, ansi);
                    let row = &lines[usize::from(preview)];
                    assert!(
                        row.starts_with("  Search:") && row.ends_with(badge),
                        "{row:?}"
                    );
                    assert_eq!(row.cell_width() as usize, width - 2, "{row:?}");
                    if query.len() > width {
                        assert!(row.contains('…'), "{row:?}");
                    }
                }
            }
        }
    }
}
#[test]
fn theme_table_search_row_labels_current_mode_themes() {
    let mut state = super::test_support::ui();
    state.key(Key::Char('t'));
    state.key(Key::Char(' '));
    assert!(state.layout(80, 15, false)[1].ends_with("Themes"));
}
