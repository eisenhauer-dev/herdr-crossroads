use crate::config;
use crate::input::{Arrow, Key};
use crate::keymap::{Action, Context, Keymap};
use crate::test_input::decode;
use crate::test_presentation::without_sgr;
use crate::test_support::ui;

#[test]
fn configured_navigation_synonyms_wrap_without_changing_modes_or_focusing() {
    use crate::selection::Mode;
    for (context, action, previous, expected_pane) in [
        ("select", "next_row", false, "p-b"),
        ("select", "previous_row", true, "p-other-untitled"),
        ("select", "next_tab", false, "p-b"),
        ("select", "previous_tab", true, "p-other"),
        ("select", "next_workspace", false, "p-b"),
        ("select", "previous_workspace", true, "p-other"),
        ("help", "next_category", false, "p-b"),
        ("help", "previous_category", true, "p-b"),
        ("themes", "next_theme", false, "p-b"),
        ("themes", "previous_theme", true, "p-b"),
    ] {
        for key in [Key::Char('N'), Key::Char('Z')] {
            let mut state = ui();
            if context == "help" {
                state.key(Key::Char('?'));
                state.help.selected = Some(if previous {
                    0
                } else {
                    state.help.entries.len() - 1
                });
            } else if context == "themes" {
                state.key(Key::Char('t'));
                state.theme_cursor = if previous {
                    0
                } else {
                    crate::theme::NAMES.len() - 1
                };
            } else if !previous {
                state.selected = state.rows.last().map(|row| row.id.clone());
            }
            let (map, errors) =
                Keymap::parse(&serde_json::json!({context:{action:["N", "Z"]}}).to_string());
            assert!(errors.is_empty(), "{errors:?}");
            state.keymap = map;
            assert!(state.key(key).is_none(), "{context}.{action}: {key:?}");
            assert!(!state.closed);
            assert_eq!(state.theme_index, 0);
            assert_eq!(state.selected.as_deref(), Some(expected_pane));
            match context {
                "help" => {
                    assert_eq!(state.mode, Mode::Help);
                    assert_eq!(
                        state.help.selected,
                        Some(if previous {
                            state.help.entries.len() - 1
                        } else {
                            0
                        })
                    );
                }
                "themes" => {
                    assert_eq!(state.mode, Mode::Themes);
                    assert_eq!(
                        state.theme_cursor,
                        if previous {
                            crate::theme::NAMES.len() - 1
                        } else {
                            0
                        }
                    );
                }
                _ => assert_eq!(state.mode, Mode::Select),
            }
        }
    }
}

#[test]
fn every_action_resolves_only_in_its_declared_modes_with_stable_names() {
    let map = Keymap::default();
    for (action, name, contexts) in [
        (Action::NextRow, "next_row", vec![Context::Select]),
        (Action::PreviousRow, "previous_row", vec![Context::Select]),
        (Action::PreviousTab, "previous_tab", vec![Context::Select]),
        (Action::NextTab, "next_tab", vec![Context::Select]),
        (
            Action::NextWorkspace,
            "next_workspace",
            vec![Context::Select],
        ),
        (
            Action::PreviousWorkspace,
            "previous_workspace",
            vec![Context::Select],
        ),
        (Action::FocusPane, "focus_pane", vec![Context::Select]),
        (
            Action::OpenSearch,
            "open_search",
            vec![Context::Select, Context::Help],
        ),
        (
            Action::ResetSearch,
            "reset_search",
            vec![Context::Select, Context::Help],
        ),
        (Action::OpenThemes, "open_themes", vec![Context::Select]),
        (
            Action::OpenHelp,
            "open_help",
            vec![Context::Select, Context::Themes],
        ),
        (Action::Close, "close", vec![Context::Select, Context::Help]),
        (Action::NextCategory, "next_category", vec![Context::Help]),
        (
            Action::PreviousCategory,
            "previous_category",
            vec![Context::Help],
        ),
        (Action::NextTheme, "next_theme", vec![Context::Themes]),
        (
            Action::PreviousTheme,
            "previous_theme",
            vec![Context::Themes],
        ),
        (
            Action::TogglePreview,
            "toggle_preview",
            vec![Context::Themes],
        ),
        (Action::Save, "save", vec![Context::Themes]),
        (Action::Cancel, "cancel", vec![Context::Themes]),
    ] {
        assert_eq!(action.name(), name);
        for context in [Context::Select, Context::Help, Context::Themes] {
            let (replacement, errors) =
                Keymap::parse(&serde_json::json!({context.name(): {name: ["Z"]}}).to_string());
            if contexts.contains(&context) {
                assert!(errors.is_empty(), "{context:?}.{name}: {errors:?}");
                assert_eq!(replacement.action(context, Key::Char('Z')), Some(action));
                let binding = map
                    .bindings
                    .iter()
                    .find(|b| b.context == context && b.action == action)
                    .unwrap();
                for &key in &binding.keys {
                    assert_eq!(map.action(context, key), Some(action));
                    assert_eq!(replacement.action(context, key), None);
                }
            } else {
                assert_eq!(
                    errors,
                    [format!("{}: unknown action {name}", context.name())]
                );
                assert_eq!(replacement.action(context, Key::Char('Z')), None);
            }
        }
    }
}

#[test]
fn collision_errors_and_groups_keep_canonical_and_display_ordering() {
    let (map, errors) = Keymap::parse(
        r#"{
        "select":{"next_row":["↓","Down"],"previous_row":["↓","Down"]},
        "help":{"next_category":["z","a"],"previous_category":["z","a"]}
    }"#,
    );
    assert_eq!(
        errors,
        [
            "help | a | next_category, previous_category",
            "help | z | next_category, previous_category",
            "select | Down | next_row, previous_row",
            "select | ↓ | next_row, previous_row",
        ]
    );
    assert_eq!(
        map.diagnostic_blocks(),
        vec![
            (
                "Keymap collisions - help mode".into(),
                vec!["next_category, previous_category: a/z".into()]
            ),
            (
                "Keymap collisions - select mode".into(),
                vec!["next_row, previous_row: ↓/↓".into()]
            ),
        ]
    );
}

#[test]
fn conservative_bindings_reject_all_ctrl_alt_in_every_context() {
    for (context, action) in [
        ("select", "next_row"),
        ("help", "next_category"),
        ("themes", "next_theme"),
    ] {
        for modifier in ["Ctrl", "Alt"] {
            for value in ('a'..='z')
                .chain('A'..='Z')
                .chain('0'..='9')
                .map(|c| c.to_string())
                .chain(
                    ["Up", "Down", "Left", "Right", "1", "!", "é", "Enter", "Tab"]
                        .map(str::to_owned),
                )
            {
                let key = format!("{modifier}+{value}");
                let (map, errors) =
                    Keymap::parse(&serde_json::json!({context: {action: [&key]}}).to_string());
                assert!(!errors.is_empty(), "accepted {context}: {key}");
                assert!(
                    map.diagnostic_blocks()
                        .iter()
                        .any(
                            |(heading, details)| heading.ends_with(&format!("- {context} mode"))
                                && details.iter().any(|detail| detail.contains(&key))
                        ),
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn conservative_allowed_characters_and_specials_roundtrip() {
    for text in [
        "j",
        "J",
        "0",
        "9",
        "/",
        ".",
        ",",
        "-",
        "_",
        "!",
        "\"",
        "?",
        "%",
        "&",
        "+",
        "|",
        "é",
        "🙂",
        "Up",
        "Down",
        "Left",
        "Right",
        "Tab",
        "Shift+Tab",
        "Esc",
        "Enter",
        "Backspace",
        "Space",
    ] {
        let key = Key::parse(text).unwrap();
        assert_eq!(Key::parse(&key.label()), Some(key), "{text}");
        let (map, errors) = Keymap::parse(&serde_json::json!({"themes": {"toggle_preview": [text], "next_theme": ["A"], "previous_theme": ["B"], "save": ["C"], "cancel": ["D"], "open_help": ["E"]}}).to_string());
        assert!(errors.is_empty(), "{text}: {errors:?}");
        assert_eq!(
            map.action(Context::Themes, key),
            Some(Action::TogglePreview)
        );
        if let Key::Char(c) = key {
            assert_eq!(decode(c.to_string().as_bytes()), [key], "raw {text}");
            let kitty = decode(format!("\x1b[{}u", c as u32).as_bytes());
            // Preserve the decoder's existing functional-code boundary; raw
            // Unicode input and configuration are not restricted to ASCII.
            if (c as u32) < 57344 {
                assert_eq!(kitty, [key], "Kitty {text}");
            } else {
                assert!(kitty.is_empty(), "existing Kitty boundary: {text}");
            }
        }
    }
    for (bytes, key) in [
        (b"\x1b[A".as_slice(), Key::Up),
        (b"\x1b[B", Key::Down),
        (b"\x1b[D", Key::Left),
        (b"\x1b[C", Key::Right),
        (b"\t", Key::Tab),
        (b"\x1b[Z", Key::BackTab),
        (b"\x1b[27u", Key::Esc),
        (b"\r", Key::Enter),
        (b"\x7f", Key::Backspace),
        (b" ", Key::Char(' ')),
        (b"\x1b[57352u", Key::Up),
        (b"\x1b[57353u", Key::Down),
        (b"\x1b[57350u", Key::Left),
        (b"\x1b[57351u", Key::Right),
        (b"\x1b[9u", Key::Tab),
        (b"\x1b[9;2u", Key::BackTab),
        (b"\x1b[13u", Key::Enter),
        (b"\x1b[127u", Key::Backspace),
    ] {
        assert_eq!(decode(bytes), [key], "{bytes:?}");
        assert_eq!(Key::parse(&key.label()), Some(key));
    }
    for text in ["Shift+1", "Shift+!", "Shift+j"] {
        assert_eq!(Key::parse(text), None);
    }
    assert_eq!(
        decode(b"\x1b[49:33;2u\x1b[106;2u"),
        [Key::Char('!'), Key::Char('J')]
    );
}

#[test]
fn single_slash_help_and_search_prompt_use_exact_unquoted_label() {
    for (keys, expected) in [
        (vec!["/"], "/"),
        (vec!["/", "s"], "'/'/s"),
        (vec!["s", "/"], "s/'/'"),
        (vec!["|"], "'|'"),
    ] {
        let mut state = ui();
        (state.keymap, state.keymap_errors) = Keymap::parse(
            &serde_json::json!({"select":{"open_search":keys},"help":{"open_search":keys}})
                .to_string(),
        );
        assert!(state.keymap_errors.is_empty());
        assert_eq!(
            state.keymap.display_labels(Context::Select, "open_search"),
            expected
        );
        assert!(state
            .frame_width(160)
            .starts_with(&format!("  Search: {expected} to search")));
        state.key(Key::Char('?'));
        let entry = state
            .help
            .entries
            .iter()
            .find(|entry| entry.meaning == "search")
            .unwrap();
        assert_eq!(entry.keys, expected);
        assert!(state
            .frame_width(160)
            .starts_with(&format!("  Search: {expected} to search")));
        state.help.query = expected.into();
        assert!(!state.help.visible().is_empty());
        assert!(state.terminal_frame(160, 30).contains("\x1b[4m"));
        if expected == "/" {
            state.help.query = "'".into();
            assert!(
                state.help.visible().is_empty(),
                "single slash must not invent searchable quotes"
            );
        }
    }
}

#[test]
fn loaded_error_pages_have_source_rows_and_pathless_useful_diagnostics() {
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-pathless-errors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("keymaps.jsonc");
    for (text, expected) in [
        (
            r#"{"select":{"next_row":["k"]}}"#,
            "next_row, previous_row: k",
        ),
        (
            r#"{"select":{"next_row":null}}"#,
            "next_row: expected nonempty key list",
        ),
        (
            r#"{"select":{"next_row":["Ctrl+i"]}}"#,
            "Ctrl/Alt bindings are not allowed",
        ),
        (
            r#"{"select":{"next_row":["F1",1,"q"]}}"#,
            "next_row: unknown key \"F1\"",
        ),
        (
            r#"{"select":{"next_row":[]},"select":{}}"#,
            "duplicate field select",
        ),
        ("{\n  invalid", "line 2"),
    ] {
        std::fs::write(&path, text).unwrap();
        let mut state = ui();
        (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
        let frame = without_source_rows(&state.frame_width(200), &path);
        assert!(frame.starts_with("ERROR:"), "{frame}");
        assert!(!frame.contains(&dir.display().to_string()), "{frame}");
        assert!(!frame.contains("keymaps.jsonc"), "{frame}");
        assert!(frame.contains(expected), "{frame}");
        assert!(
            frame.contains("↑/k: scroll up · ↓/j: scroll down"),
            "{frame}"
        );
    }
    std::fs::write(&path, [0xff]).unwrap();
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
    let frame = without_source_rows(&state.frame_width(200), &path);
    assert!(frame.starts_with("ERROR:"), "{frame}");
    assert!(frame.contains("UTF-8"), "{frame}");
    assert!(!frame.contains(&dir.display().to_string()), "{frame}");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
    let frame = without_source_rows(&state.frame_width(200), &path);
    assert!(frame.contains("directory"), "{frame}");
    assert!(!frame.contains(&dir.display().to_string()), "{frame}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn error_page_fixed_navigation_precedes_custom_actions_at_every_height() {
    for (width, height) in [(100, 20), (40, 7), (40, 3), (40, 2), (40, 1)] {
        let mut state = ui();
        (state.keymap, state.keymap_errors) =
            Keymap::parse(r#"{"select":{"focus_pane":["j","Down"],"open_search":["k","Up"]}}"#);
        assert!(!state.keymap_errors.is_empty());
        state.popup_size = Some((width, height));
        state.query = "kept".into();
        let selected = state.selected.clone();
        for key in [Key::Char('j'), Key::Down, Key::Down, Key::Char('j')] {
            let before = state.error_scroll.get();
            assert_eq!(state.key(key), None);
            assert_eq!(state.error_scroll.get(), before + 1);
        }
        for key in [Key::Ctrl('j'), Key::Alt('j'), Key::Ctrl('k'), Key::Alt('k')] {
            assert_eq!(state.key(key), None);
            assert_eq!(state.error_scroll.get(), 4);
        }
        for key in [
            Key::Char('k'),
            Key::Up,
            Key::Up,
            Key::Char('k'),
            Key::Char('k'),
        ] {
            let before = state.error_scroll.get();
            assert_eq!(state.key(key), None);
            assert_eq!(state.error_scroll.get(), before.saturating_sub(1));
        }
        assert_eq!(state.query, "kept");
        assert_eq!(state.selected, selected);
        assert_eq!(state.mode, crate::selection::Mode::Select);
        assert!(!state.closed);
        for key in [Key::Enter, Key::Esc, Key::Char('q')] {
            assert_eq!(state.key(key), None);
            assert!(state.closed);
            state.closed = false;
        }
    }
}

fn without_source_rows(frame: &str, source: &std::path::Path) -> String {
    let row = format!(
        "{}  {}",
        source.file_name().unwrap().to_string_lossy(),
        source.display()
    );
    let source = source.display().to_string();
    let lines: Vec<_> = frame.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("ERROR:") {
            assert_eq!(
                lines[at + 1],
                row,
                "source must immediately follow every file heading: {frame}"
            );
        }
        if line.contains(&source) {
            assert_eq!(*line, row, "source is not part of a diagnostic: {frame}");
            assert!(
                at > 0 && lines[at - 1].starts_with("ERROR:"),
                "source repeated away from heading: {frame}"
            );
        }
    }
    lines
        .into_iter()
        .filter(|line| *line != row)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn source_row_preserves_filename_dims_only_path_and_clips_by_unicode_cells() {
    use ratatui::buffer::CellWidth;
    let dir = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-source-style-{}-路径-e\u{301}-🙂",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("keymaps.jsonc");
    std::fs::write(&source, r#"{"select":{"next_row":[]}}"#).unwrap();
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
    let path = source.display().to_string();
    let full = format!("keymaps.jsonc  {path}");
    let filename_width = usize::from("keymaps.jsonc".cell_width());
    let complete_width = usize::from(full.cell_width()) + 1;
    let unicode_start = path.find("路径").unwrap();
    let unicode_cells = usize::from(path[..unicode_start].cell_width());
    let mut widths = vec![
        240,
        complete_width,
        complete_width - 1,
        complete_width - 2,
        complete_width - 3,
        80,
        67,
        40,
        18,
        16,
        15,
        14,
        13,
        12,
        5,
        2,
        1,
    ];
    widths.extend((0..10).map(|extra| filename_width + 2 + unicode_cells + 1 + extra));
    for theme_index in [1, 2, 3] {
        state.theme_index = theme_index;
        let fg = |color| match color {
            ratatui::style::Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
            ratatui::style::Color::Gray => "\x1b[37m".into(),
            ratatui::style::Color::Reset => "\x1b[39m".into(),
            color => panic!("unexpected source palette: {color:?}"),
        };
        for &width in &widths {
            state.error_scroll.set(0);
            let lines = state.layout(width, 120, true);
            let at = if width < 80 {
                crate::presentation::wrapped(
                    &format!("need 80x15; actual {width}x120."),
                    width.saturating_sub(3),
                )
                .len()
                    + crate::presentation::wrapped(
                        "Enlarge popup and reopen.",
                        width.saturating_sub(3),
                    )
                    .len()
                    + 3
            } else {
                1
            };
            let row = &lines[at];
            let text = without_sgr(row);
            assert!(usize::from(text.cell_width()) < width, "{width}: {row:?}");
            assert!(!text.contains('\n') && !text.contains('\r'));
            if width >= complete_width {
                assert_eq!(text, full);
            } else if width >= filename_width + 5 {
                let clipped_path = text
                    .strip_prefix("keymaps.jsonc  ")
                    .unwrap_or_else(|| panic!("filename must precede clipped path: {row:?}"));
                let prefix = clipped_path
                    .strip_suffix('…')
                    .unwrap_or_else(|| panic!("long path must end with ellipsis: {row:?}"));
                assert!(path.starts_with(prefix), "{prefix}");
                assert!(!prefix.is_empty(), "path must not be an isolated ellipsis");
                assert!(
                    !path[prefix.len()..].starts_with('\u{301}'),
                    "must not split combining grapheme: {prefix}"
                );
            } else if width > filename_width {
                assert_eq!(text, "keymaps.jsonc");
            } else if width >= 2 {
                assert!(text.ends_with('…'), "filename safely clipped: {text}");
            } else {
                assert!(text.is_empty());
            }
            if text.contains("  ") {
                assert!(
                    row.starts_with(&format!("keymaps.jsonc  {}\x1b[2m", fg(state.theme().dim))),
                    "only path gets dim color/style: {row:?}"
                );
                assert!(
                    row.ends_with(&format!("\x1b[22m{}", fg(state.theme().text))),
                    "path must reset to ordinary foreground: {row:?}"
                );
            } else {
                assert!(
                    !row.contains('\x1b'),
                    "filename must remain ordinary text: {row:?}"
                );
            }
            assert_eq!(
                without_sgr(&lines[at + 1]),
                crate::presentation::wrapped(
                    "next_row: expected nonempty key list",
                    width.saturating_sub(3)
                )[0],
                "source must use exactly one row: {lines:?}"
            );
            let plain = state.frame_width(width);
            assert_eq!(
                plain.lines().nth(1),
                Some(text.as_str()),
                "plaintext source also clips without wrapping: {plain}"
            );
            assert!(!plain.contains('\x1b'));
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn r2_source_path_requires_a_visible_grapheme_before_clipping_ellipsis() {
    use ratatui::buffer::CellWidth;
    for first in ["a", "路", "e\u{301}", "👩\u{200d}💻"] {
        let source = std::path::PathBuf::from(format!("{first}long-directory/x"));
        let mut state = ui();
        (state.keymap, state.keymap_errors) = Keymap::parse(r#"{"select":{"next_row":[]}}"#);
        state.keymap.diagnostics.source = Some(source.clone());
        let first_cells = usize::from(first.cell_width());
        // One reserved last column, filename, and two separator spaces.
        for path_budget in 0..=first_cells + 1 {
            let width = 1 + 1 + 2 + path_budget;
            let expected = if path_budget <= first_cells {
                "x".to_owned()
            } else {
                format!("x  {first}…")
            };
            let plain = state.frame_width(width);
            assert_eq!(
                plain.lines().nth(1),
                Some(expected.as_str()),
                "{source:?} budget {path_budget}: {plain:?}"
            );
            assert!(usize::from(expected.cell_width()) < width);
            let lines = state.layout(width, 120, true);
            let row = lines
                .iter()
                .find(|row| without_sgr(row) == expected)
                .unwrap_or_else(|| panic!("{source:?} budget {path_budget}: {lines:?}"));
            if path_budget <= first_cells {
                assert_eq!(row, "x", "no separator/style without a real path prefix");
            } else {
                assert!(row.contains("\x1b[2m") && row.contains("\x1b[22m"));
            }
        }
    }
}

#[test]
fn r3_custom_characters_require_visible_standalone_text() {
    for text in [
        "\0",
        "\u{7f}",
        "\u{9f}",
        "\u{ad}",
        "\u{600}",
        "\u{61c}",
        "\u{200b}",
        "\u{200c}",
        "\u{200d}",
        "\u{200e}",
        "\u{202e}",
        "\u{2060}",
        "\u{feff}",
        "\u{301}",
        "\u{34f}",
        "\u{93e}",
        "\u{20dd}",
        "\u{fe0f}",
        "\u{e0100}",
        "\u{a0}",
        "\u{2003}",
        "\u{2028}",
        "\u{2029}",
        "\u{3000}",
        "\u{378}",
        "\u{379}",
        "\u{d7ff}",
        "\u{e000}",
        "\u{f8ff}",
        "\u{f0000}",
        "\u{ffffd}",
        "\u{100000}",
        "\u{10fffd}",
        "\u{fffe}",
        "\u{10ffff}",
        "e\u{301}",
    ] {
        assert_eq!(Key::parse(text), None, "accepted {text:?}");
        let (map, errors) =
            Keymap::parse(&serde_json::json!({"themes": {"toggle_preview": [text]}}).to_string());
        assert_eq!(errors.len(), 1, "{text:?}: {errors:?}");
        assert_eq!(map.action(Context::Themes, Key::Char(' ')), None);
        assert_eq!(
            map.diagnostic_blocks()[0].0,
            "Unknown keymap keys - themes mode"
        );
    }
    for text in [
        "é",
        "中",
        "Ω",
        "Ж",
        "ش",
        "↓",
        "🙂",
        "ﾞ",
        "'",
        "\"",
        "\\",
        " ",
        "ǅ",
        "ʰ",
        "٣",
        "Ⅳ",
        "½",
        "_",
        "—",
        "(",
        ")",
        "«",
        "»",
        "!",
        "$",
        "^",
        "\u{d7a3}",
        "\u{f900}",
        "\u{10000}",
    ] {
        let key = Key::Char(text.chars().next().unwrap());
        assert_eq!(Key::parse(text), Some(key), "{text:?}");
        let (map, errors) =
            Keymap::parse(&serde_json::json!({"themes": {"toggle_preview": [text]}}).to_string());
        assert!(errors.is_empty(), "{text:?}: {errors:?}");
        assert_eq!(
            map.action(Context::Themes, key),
            Some(Action::TogglePreview)
        );
    }
    assert_eq!(Key::parse("Space"), Some(Key::Char(' ')));
    // Surrogates cannot be represented as Rust chars or a valid single-scalar string.
    assert_eq!(char::from_u32(0xd800), None);
    assert_eq!(char::from_u32(0xdfff), None);
}

#[test]
fn r3_collision_diagnostics_distinguish_glyphs_from_special_keys() {
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["↓"],"previous_row":["Down"],"open_help":["F1"]}}"#,
    );
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        map.action(Context::Select, Key::Char('↓')),
        Some(Action::NextRow)
    );
    assert_eq!(
        map.action(Context::Select, Key::Down),
        Some(Action::PreviousRow)
    );
    assert_eq!(
        map.diagnostic_blocks(),
        vec![(
            "Unknown keymap keys - select mode".into(),
            vec!["open_help: unknown key \"F1\"".into()],
        )]
    );
    for key in ["↓", "Down", "Space", " "] {
        let (map, errors) = Keymap::parse(
            &serde_json::json!({"select": {"next_row": [key], "previous_row": [key]}}).to_string(),
        );
        assert_eq!(errors.len(), 1, "{key}: {errors:?}");
        assert_eq!(
            map.diagnostic_blocks(),
            vec![(
                "Keymap collisions - select mode".into(),
                vec![format!(
                    "next_row, previous_row: {}",
                    Key::parse(key).unwrap().display_label()
                )],
            )]
        );
    }
}

#[test]
fn r2_arrow_glyph_configuration_is_literal_not_the_special_arrow_key() {
    assert_eq!(Key::parse("↓"), Some(Key::Char('↓')));
    assert_eq!(Key::parse("Down"), Some(Key::Down));
    let (map, errors) = Keymap::parse(r#"{"select":{"next_row":["↓"]}}"#);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        map.action(Context::Select, Key::Char('↓')),
        Some(Action::NextRow)
    );
    assert_eq!(map.action(Context::Select, Key::Down), None);
    assert_eq!(decode("↓".as_bytes()), [Key::Char('↓')]);
    assert_eq!(decode(b"\x1b[B"), [Key::Down]);
}

#[test]
fn loaded_error_blocks_show_selected_source_once_immediately_under_each_heading() {
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-block-sources-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for extension in ["jsonc", "json"] {
        for (name, content) in [
            (
                "keymaps",
                r#"{"select":{"next_row":["J","J","J"],"broken":null},"help":{"bad":["F1"]}}"#,
            ),
            ("status-colors", r#"{"working":false,"typo":1}"#),
            ("theme", r#"{"theme":"bogus","extra":1}"#),
        ] {
            std::fs::write(dir.join(format!("{name}.{extension}")), content).unwrap();
        }
    }
    for (extension, syntax, unreadable) in [
        ("jsonc", false, false),
        ("json", false, false),
        ("jsonc", true, false),
        ("jsonc", false, true),
        ("json", false, true),
    ] {
        for name in ["keymaps", "status-colors", "theme"] {
            let jsonc = dir.join(format!("{name}.jsonc"));
            if extension == "json" && jsonc.exists() {
                std::fs::remove_file(&jsonc).unwrap();
            }
            let path = dir.join(format!("{name}.{extension}"));
            if syntax {
                std::fs::write(&path, "{").unwrap();
            }
            if unreadable {
                if path.is_file() {
                    std::fs::remove_file(&path).unwrap();
                }
                std::fs::create_dir(&path).unwrap();
            }
        }
        let mut state = ui();
        (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
        (state.status_colors, state.status_errors) =
            crate::status_colors::StatusColors::load_diagnostics(Some(&dir));
        (_, state.theme_diagnostics) = crate::theme::load_diagnostics(Some(&dir));
        let frame = state.frame_width(400);
        let lines: Vec<_> = frame.lines().collect();
        let mut sources = std::collections::BTreeSet::new();
        for (at, line) in lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with("ERROR:"))
        {
            let (filename, source) = lines[at + 1].split_once("  ").unwrap();
            assert!(
                source.starts_with(&dir.display().to_string()) && source.ends_with(extension),
                "source missing immediately after {line}: {frame}"
            );
            assert_eq!(
                filename,
                std::path::Path::new(source)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            );
            sources.insert(source.to_owned());
            let end = (at + 1..lines.len())
                .find(|&at| lines[at].starts_with('─'))
                .unwrap();
            assert_eq!(
                lines[at + 1..end]
                    .iter()
                    .filter(|line| line.contains(&dir.display().to_string()))
                    .count(),
                1,
                "source repeated in block: {frame}"
            );
            if syntax {
                assert!(line.ends_with("syntax"), "{frame}");
            }
            if unreadable {
                assert!(line.ends_with("IO"), "{frame}");
            }
        }
        assert_eq!(sources.len(), 3, "{frame}");
        assert!(
            !frame.contains(if extension == "json" {
                ".jsonc"
            } else {
                ".json\n"
            }),
            "wrong selected source: {frame}"
        );
        if unreadable {
            for name in ["keymaps", "status-colors", "theme"] {
                std::fs::remove_dir(dir.join(format!("{name}.{extension}"))).unwrap();
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn long_block_source_clips_once_without_hiding_diagnostics_or_tiny_priority() {
    use ratatui::buffer::CellWidth;
    let dir = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-long-source-{}-{}",
        std::process::id(),
        "long-path-component-".repeat(6)
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("keymaps.jsonc");
    std::fs::write(&source, r#"{"select":{"next_row":[]}}"#).unwrap();
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
    for (width, height) in [
        (240, 36),
        (180, 20),
        (67, 12),
        (40, 3),
        (10, 3),
        (40, 2),
        (40, 1),
        (80, 15),
    ] {
        state.error_scroll.set(0);
        let mut rows = Vec::new();
        for at in 0..500 {
            state.error_scroll.set(at);
            let lines = state.layout(width, height, true);
            if height > 2 {
                assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
                assert!(without_sgr(&lines[height - 1]).starts_with("↑/k:"));
            }
            let body = if height > 2 {
                &lines[..height - 2]
            } else {
                &lines[..]
            };
            assert!(body
                .iter()
                .all(|line| usize::from(without_sgr(line).cell_width()) < width));
            if state.error_scroll.get() != at {
                rows.extend(body.iter().skip(1).map(|line| without_sgr(line)));
                break;
            }
            rows.push(without_sgr(&body[0]));
        }
        let text = rows.join("");
        assert!(
            text.replace("ERROR: ", "")
                .replace(' ', "")
                .contains("next_row:expectednonemptykeylist"),
            "{width}x{height}: {rows:?}"
        );
        if height > 2 {
            assert_eq!(
                rows.iter().filter(|row| row.starts_with("keymaps")).count(),
                1,
                "one physical source row per block: {rows:?}"
            );
            let source_at = rows
                .iter()
                .position(|line| line.starts_with("keymaps"))
                .unwrap();
            assert!(
                rows[source_at + 1].starts_with("next_row")
                    || rows[source_at + 1].starts_with("next_"),
                "diagnostics immediately follow the single source row: {rows:?}"
            );
            if width >= 240 {
                assert_eq!(
                    rows[source_at],
                    format!("keymaps.jsonc  {}", source.display())
                );
            } else {
                assert!(
                    rows[source_at].ends_with('…'),
                    "long source must clip with ellipsis: {rows:?}"
                );
            }
            assert!(
                !rows.iter().any(|line| line.starts_with("/tmp/")),
                "no wrapped source continuation: {rows:?}"
            );
            assert!(
                rows[source_at - 1].starts_with("ERROR:"),
                "source must directly follow block heading: {rows:?}"
            );
            assert!(
                !rows[..source_at]
                    .iter()
                    .any(|line| line.contains("opencode")),
                "size block must have no source: {rows:?}"
            );
        } else {
            assert!(
                !text.contains("opencode") && !text.contains("keymaps.jsonc"),
                "tiny rows prioritize diagnostics, not sources: {rows:?}"
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn absent_and_directly_parsed_configuration_never_invents_source_paths() {
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-no-source-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for directory in [None, Some(dir.as_path())] {
        let (map, errors) = Keymap::load(directory);
        assert!(errors.is_empty() && map.diagnostics.source.is_none());
        let (_, status) = crate::status_colors::StatusColors::load_diagnostics(directory);
        assert!(status.is_empty() && status.source.is_none());
        let (_, theme) = crate::theme::load_diagnostics(directory);
        assert!(theme.is_empty() && theme.source.is_none());
    }
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::parse(r#"{"select":{"next_row":[]}}"#);
    assert!(state.keymap.diagnostics.source.is_none());
    let frame = state.frame_width(400);
    assert!(frame
        .contains("ERROR: Keymap structure - select mode\nnext_row: expected nonempty key list"));
    assert!(!frame.contains("keymaps.json") && !frame.contains("/tmp/opencode"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn known_mode_error_blocks_put_mode_only_in_heading_without_losing_errors() {
    for (mode, next, previous) in [
        ("select", "next_row", "previous_row"),
        ("help", "next_category", "previous_category"),
        ("themes", "next_theme", "previous_theme"),
    ] {
        let close = if mode == "themes" { "cancel" } else { "close" };
        let config = format!(
            r#"{{"{mode}":{{"{next}":["A","B","C"],"{previous}":[],"bad_action":["F1"],"{close}":null,"{close}":["Ctrl+i"],"nested":[{{"field":1,"field":2}}]}},"{mode}":false,"typo":{{"extra":["X","Y","Z"]}}}}"#
        );
        let mut state = ui();
        (state.keymap, state.keymap_errors) = Keymap::parse(&config);
        let blocks = state.keymap.diagnostic_blocks();
        for (kind, diagnostic) in [
            ("Maximum 2 keybinds per action", next.to_owned()),
            ("Unknown keymap actions", "bad_action".into()),
            (
                "Keymap structure",
                format!("{previous}: expected nonempty key list"),
            ),
            (
                "Unknown keymap keys",
                "bad_action: unknown key \"F1\"".into(),
            ),
            (
                "Duplicate keymap fields",
                format!("{close}: duplicate field"),
            ),
            ("Unsupported keymap keys", format!("{close}: \"Ctrl+i\"")),
        ] {
            let heading = format!("{kind} - {mode} mode");
            let (_, details) = blocks
                .iter()
                .find(|(name, _)| name == &heading)
                .unwrap_or_else(|| panic!("missing {heading}: {blocks:?}"));
            assert!(
                details.iter().any(|detail| detail.contains(&diagnostic)),
                "{heading}: {details:?}"
            );
        }
        let frame = state.frame_width(1200);
        for prefix in [format!("{mode}."), format!("{mode}:"), format!("{mode} | ")] {
            assert!(!frame.contains(&prefix), "redundant mode {prefix}: {frame}");
        }
        assert!(
            frame.contains("nested[0]: duplicate field field"),
            "{frame}"
        );
        assert!(frame.contains("expected object of actions"), "{frame}");
        assert!(
            frame.contains(&format!("duplicate field {mode}")),
            "root duplicate must remain general: {frame}"
        );
        assert!(
            frame.contains("unknown context typo") && frame.contains("extra"),
            "{frame}"
        );
        assert!(!frame.contains("- typo mode"), "{frame}");
        assert!(
            !state.keymap_errors.is_empty(),
            "empty lists and unknown actions remain errors"
        );
        let collision_config =
            format!(r#"{{"{mode}":{{"{next}":["A","B"],"{previous}":["A","B"]}}}}"#);
        let (map, errors) = Keymap::parse(&collision_config);
        assert!(!errors.is_empty());
        let blocks = map.diagnostic_blocks();
        assert_eq!(
            blocks,
            vec![(
                format!("Keymap collisions - {mode} mode"),
                vec![format!("{next}, {previous}: A/B")]
            )]
        );
    }
}

#[test]
fn duplicate_scopes_use_ast_segments_not_dotted_field_name_guesses() {
    for (text, identity) in [
        (
            r#"[{"select":{"x":1,"x":2}}]"#,
            "$[0].select: duplicate field x",
        ),
        (
            r#"{"select.x":{"bad":[],"bad":null}}"#,
            "$.select.x: duplicate field bad",
        ),
        (r#"{"[0]":{"x":1,"x":2}}"#, "$.[0]: duplicate field x"),
        (r#"{"":{"":{"x":1,"x":2}}}"#, "$..: duplicate field x"),
    ] {
        let parsed = config::parse(text).unwrap();
        assert!(
            parsed.errors.iter().any(|error| error == identity),
            "{:?}",
            parsed.errors
        );
        let (map, errors) = Keymap::parse(text);
        assert!(!errors.is_empty());
        let blocks = map.diagnostic_blocks();
        assert!(
            blocks
                .iter()
                .any(|(heading, details)| heading == "Duplicate keymap fields"
                    && details
                        .iter()
                        .any(|detail| detail.contains("duplicate field"))),
            "{blocks:?}"
        );
        assert!(
            !blocks
                .iter()
                .any(|(heading, _)| heading.ends_with("- select mode")),
            "must not invent scope: {blocks:?}"
        );
    }
}

#[test]
fn mixed_small_popup_block_is_first_updates_on_resize_and_keeps_config_diagnostics() {
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::parse(r#"{"select":{"broken":null}}"#);
    state
        .status_errors
        .push("Status colors invalid values", "working must be #RRGGBB");
    state
        .theme_diagnostics
        .push("Theme invalid values", "unknown theme bogus");
    for (width, height) in [
        (67, 12),
        (79, 15),
        (80, 14),
        (80, 15),
        (120, 24),
        (67, 12),
        (40, 3),
        (40, 2),
        (40, 1),
        (80, 15),
    ] {
        state.popup_size = Some((width, height));
        state.error_scroll.set(0);
        let lines = state.layout(width, height, true);
        let small = width < 80 || height < 15;
        if small && height > 2 {
            assert!(
                without_sgr(&lines[0]).starts_with("ERROR: Popup too small"),
                "{width}x{height}: {lines:?}"
            );
        }
        let mut visible = String::new();
        for at in 0..300 {
            state.error_scroll.set(at);
            let lines = state.layout(width, height, true);
            let body = if height > 2 {
                assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
                assert!(without_sgr(&lines[height - 1]).starts_with("↑/k:"));
                &lines[..height - 2]
            } else {
                assert!(lines
                    .iter()
                    .all(|line| without_sgr(line).starts_with("ERROR:")));
                &lines[..]
            };
            if state.error_scroll.get() != at {
                for line in body.iter().skip(1) {
                    visible.push_str(&without_sgr(line));
                }
                break;
            }
            visible.push_str(&without_sgr(&body[0]));
        }
        let normalized = visible.replace(' ', "");
        for detail in ["broken", "working must be #RRGGBB", "unknown theme bogus"] {
            assert!(
                normalized.contains(&detail.replace(' ', "")),
                "{width}x{height}: {visible}"
            );
        }
        assert_eq!(normalized.contains("need80x15"), small, "{visible}");
        if small {
            assert!(
                normalized.contains(&format!("actual{width}x{height}")),
                "{visible}"
            );
            assert!(normalized.contains("Enlargepopupandreopen."), "{visible}");
        }
        state.error_scroll.set(usize::MAX);
        state.layout(width, height, true);
        assert!(state.error_scroll.get() < 300);
        assert!(
            !state.frame_width(67).contains("Popup too small"),
            "plaintext must not gain a size gate"
        );
    }
}

#[test]
fn oversized_mixed_errors_keep_all_diagnostics_in_a_distinct_section() {
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["F1",1,"q"],"broken":null},"typo":{"extra":["A","B","C"]},"help":{"close":[],"close":["Esc"]}}"#,
    );
    let mut state = ui();
    state.keymap = map;
    state.keymap_errors = errors;
    let frame = state.frame_width(160);
    for diagnostic in [
        "unknown key \"F1\"",
        "unknown key 1",
        "next_row, close",
        "unknown context typo",
        "ERROR: Unknown keymap actions - select mode\nbroken",
        "broken: expected nonempty",
        "close: expected nonempty",
        "duplicate field",
    ] {
        assert!(frame.contains(diagnostic), "{frame}");
    }
    assert!(!frame.contains("Other diagnostics"), "{frame}");
    assert!(
        frame.contains("ERROR: Unknown keymap keys - select mode"),
        "{frame}"
    );
    assert!(
        frame.contains("ERROR: Maximum 2 keybinds per action - select mode\nnext_row"),
        "{frame}"
    );
    assert!(frame.contains("extra (unknown mode typo)"), "{frame}");
    assert!(!frame.contains("Context |"), "{frame}");
}

#[test]
fn oversized_loaded_actions_have_one_pathless_list_and_keep_other_diagnostics() {
    use ratatui::buffer::CellWidth;
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-overflow-list-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("keymaps.jsonc");
    for mixed in [false, true] {
        let extra = if mixed {
            r#", "bad":[], "next_tab":["F1","q"], "broken":null, "next_workspace":["Ctrl+i"]"#
        } else {
            ""
        };
        std::fs::write(&path, format!(r#"{{"select":{{"next_row":["J","J","J"],"previous_row":["K","K","K"]{extra}}},"help":{{"next_category":["A","B","C"]}},"themes":{{"next_theme":["X","Y","Z"]}}}}"#)).unwrap();
        let (map, errors) = Keymap::load(Some(&dir));
        let mut state = ui();
        state.keymap = map;
        state.keymap_errors = errors;
        let frame = without_source_rows(&state.frame_width(160), &path);
        assert_eq!(
            frame.matches("Maximum 2 keybinds per action - ").count(),
            3,
            "{frame}"
        );
        assert!(
            frame.starts_with("ERROR: Maximum 2 keybinds per action - select mode\n"),
            "{frame}"
        );
        assert!(!frame.contains("Context |"), "{frame}");
        assert!(
            frame.contains(
                "ERROR: Maximum 2 keybinds per action - select mode\nnext_row\nprevious_row"
            ),
            "{frame}"
        );
        for row in [
            "next_row",
            "previous_row",
            "ERROR: Maximum 2 keybinds per action - help mode\nnext_category",
            "ERROR: Maximum 2 keybinds per action - themes mode\nnext_theme",
        ] {
            assert!(frame.contains(row), "{frame}");
        }
        let list = frame.split("ERROR: Keymap").next().unwrap();
        assert!(!list.contains(&path.display().to_string()), "{frame}");
        assert!(!list.contains("maximum 2 defined"), "{frame}");
        if mixed {
            assert!(!frame.contains("Other diagnostics"), "{frame}");
            assert!(
                frame.contains("ERROR: Unknown keymap actions - select mode"),
                "{frame}"
            );
            for diagnostic in [
                "\nbad\n",
                "bad: expected nonempty",
                "unknown key",
                "broken: expected nonempty",
                "next_tab, close",
                "Ctrl/Alt bindings are not allowed",
            ] {
                assert!(frame.contains(diagnostic), "{frame}");
            }
            assert!(!frame.contains(&path.display().to_string()), "{frame}");
        } else {
            assert!(!frame.contains("Other diagnostics"), "{frame}");
        }
        for (width, height) in [(80, 15), (35, 7), (10, 3), (40, 2), (40, 1)] {
            state.popup_size = Some((width, height));
            state.error_scroll.set(0);
            if height <= 2 {
                let first = state.layout(width, height, true).join("\n");
                assert!(first.contains("need 80x15"), "{width}x{height}: {first}");
                assert!(!first.contains("Context"), "{first}");
            }
            let mut visible = String::new();
            for start in 0..250 {
                let lines = state.layout(width, height, true);
                assert!(lines.len() <= height);
                let body = if height > 2 {
                    assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
                    &lines[..height - 2]
                } else {
                    &lines[..]
                };
                assert!(body
                    .iter()
                    .all(|line| usize::from(without_sgr(line).cell_width()) < width));
                // Collect each fragment once despite overlapping viewports.
                let at_end = state.error_scroll.get() != start;
                for line in
                    body.iter()
                        .skip(usize::from(at_end))
                        .take(if at_end { usize::MAX } else { 1 })
                {
                    let text = without_sgr(line);
                    visible.push_str(text.strip_prefix("ERROR: ").unwrap_or(&text));
                }
                if at_end {
                    break;
                }
                assert_eq!(state.key(Key::Down), None);
            }
            for action in ["next_row", "previous_row", "next_category", "next_theme"] {
                assert!(visible.contains(action), "{width}x{height}: {visible}");
            }
            if mixed {
                assert_eq!(visible.contains("keymaps"), height > 2, "{visible}");
                assert!(visible.contains("broken:"), "{visible}");
            }
            assert!(!visible.contains("minimum content"));
            for close in [Key::Enter, Key::Esc, Key::Char('q')] {
                assert_eq!(state.key(close), None);
                assert!(state.closed);
                state.closed = false;
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a10c_defined_key_limit_precedes_dedup_and_keeps_diagnostics() {
    for keys in [r#"["J","K","L"]"#, r#"["J","J","J"]"#] {
        let (map, errors) = Keymap::parse(&format!(r#"{{"select":{{"next_row":{keys}}}}}"#));
        assert_eq!(errors, [crate::keymap::KEY_LIMIT_HINT]);
        assert_eq!(map.oversized_actions.len(), 1);
        let entry = &map.oversized_actions[0];
        assert_eq!((&*entry.context, &*entry.action), ("select", "next_row"));
        assert_eq!(map.diagnostic_blocks().len(), 1);
        assert_eq!(map.diagnostic_blocks()[0].1, ["next_row"]);
    }
    let (_, errors) = Keymap::parse(r#"{"select":{"next_row":["F1",1,"q"]}}"#);
    assert_eq!(
        errors.len(),
        4,
        "limit, both bad keys, and conflict: {errors:?}"
    );
    for keys in [r#"["J"]"#, r#"["J","J"]"#, r#"["J","K"]"#] {
        assert!(
            Keymap::parse(&format!(r#"{{"select":{{"next_row":{keys}}}}}"#))
                .1
                .is_empty()
        );
    }
}

#[test]
fn a10c_defaults_quit_and_search_q_are_context_specific() {
    use crate::selection::Mode;
    let map = Keymap::default();
    assert_eq!(map.labels(Context::Help, "close"), "Esc/q");
    assert_eq!(map.labels(Context::Themes, "cancel"), "Esc/q");
    for help in [false, true] {
        for end in [Key::Enter, Key::Esc] {
            let mut state = ui();
            if help {
                state.key(Key::Char('?'));
            }
            state.key(Key::Char('/'));
            state.key(Key::Char('q'));
            state.key(end);
            assert!(!state.closed);
            assert_eq!(
                if help {
                    &state.help.query
                } else {
                    &state.query
                },
                "q"
            );
            assert_eq!(state.mode, if help { Mode::Help } else { Mode::Select });
        }
    }
    let mut state = ui();
    state.key(Key::Char('?'));
    state.key(Key::Char('?'));
    assert_eq!(state.mode, Mode::Help);
    state.key(Key::Char('q'));
    assert_eq!(state.mode, Mode::Select);
    state.key(Key::Char('t'));
    state.key(Key::Char('q'));
    assert_eq!(state.mode, Mode::Select);
    assert!(!state.closed);
}

#[test]
fn footer_label_selection_applies_to_every_navigation_and_non_navigation_action() {
    for binding in Keymap::default().bindings {
        let context = binding.context;
        let action = binding.action.name();
        let navigation = matches!(
            action,
            "next_row"
                | "previous_row"
                | "next_tab"
                | "previous_tab"
                | "next_workspace"
                | "previous_workspace"
                | "next_category"
                | "previous_category"
                | "next_theme"
                | "previous_theme"
        );
        for (keys, full, first) in [
            (vec!["X", "Right"], "X/→", "X"),
            (vec!["Right", "X"], "→/X", "→"),
            (vec!["X", "Y"], "X/Y", "X"),
            (vec!["Left", "Right"], "←/→", "←"),
            (vec!["X", "X"], "X", "X"),
        ] {
            let mut actions = serde_json::Map::new();
            for (at, binding) in Keymap::default()
                .bindings
                .iter()
                .filter(|b| b.context == context)
                .enumerate()
            {
                actions.insert(
                    binding.action.name().into(),
                    serde_json::json!([char::from_u32(0x100 + at as u32).unwrap().to_string()]),
                );
            }
            actions.insert(action.into(), serde_json::json!(keys));
            let (map, errors) =
                Keymap::parse(&serde_json::json!({context.name(): actions}).to_string());
            assert!(errors.is_empty(), "{errors:?}");
            assert_eq!(map.display_labels(context, action), full);
            let expected = if navigation && keys[1].contains("Right") && keys[1] != keys[0] {
                full
            } else {
                first
            };
            assert_eq!(
                map.footer_labels(context, action),
                expected,
                "{}.{}",
                context.name(),
                action
            );
        }
    }
}

#[test]
fn jsonc_rejects_non_json_whitespace_outside_strings_and_comments() {
    for space in ['\u{000b}', '\u{000c}', '\u{00a0}', '\u{2003}'] {
        assert!(
            config::parse(&format!("{{{space}\"select\":{{}}}}")).is_err(),
            "JSON forbids U+{:04X} whitespace",
            space as u32
        );
    }
    assert!(config::parse("{/*\u{00a0}*/\"x\":\"\u{00a0}\"}").is_ok());
}

#[test]
fn jsonc_only_allows_json_plus_comments_and_trailing_commas() {
    assert!(config::parse("//header\n{\"x\":[1,true,null,],/*tail*/}").is_ok());
    for bad in [
        "",
        "//only",
        "{x:1}",
        "{'x':1}",
        "{\"x\":1 \"y\":2}",
        "[1 2]",
        "[01]",
        "[+1]",
        "[0x1]",
        "[.5]",
        "[5.]",
        "[NaN]",
        "[Infinity]",
        "[\"\\x41\"]",
        "[\"\\v\"]",
        "[\"raw\nline\"]",
        "{}{}",
        "/*",
    ] {
        assert!(config::parse(bad).is_err(), "accepted {bad:?}");
    }
    let errors = config::parse(r#"{"x":{},"x":{},"nested":{"a":1,"\u0061":2}}"#)
        .unwrap()
        .errors;
    assert_eq!(errors.len(), 2);
}

#[test]
fn replacements_deduplicate_and_validate_all_effective_owners() {
    let (map, errors) = Keymap::parse(
        r#"{"select":{"next_row":["J","!"],"previous_row":["K"]},"themes":{"next_theme":["J","J"]}}"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(map.labels(Context::Select, "next_row"), "J/!");
    assert_eq!(map.action(Context::Select, Key::Char('j')), None);
    assert_eq!(map.action(Context::Select, Key::Down), None);
    assert_eq!(
        map.action(Context::Select, Key::Char('!')),
        Some(Action::NextRow)
    );
    assert_eq!(
        map.action(Context::Select, Key::Enter),
        Some(Action::FocusPane)
    );
    let (_, errors) = Keymap::parse(r#"{"select":{"next_row":["k","q"]}}"#);
    assert_eq!(errors.len(), 2);
    assert!(errors
        .iter()
        .any(|e| e.contains("k") && e.contains("previous_row")));
    assert!(errors
        .iter()
        .any(|e| e.contains("q") && e.contains("close")));
    let (_, errors) = Keymap::parse(r#"{"select":{"next_row":["Space"],"previous_row":[" "]}}"#);
    assert_eq!(
        errors.len(),
        1,
        "Space and literal space are the same identity"
    );
}

#[test]
fn keymap_reports_structure_unknowns_and_all_list_errors() {
    for text in [
        "null",
        "[]",
        r#"{"select":[]}"#,
        r#"{"select":{"next_row":[]}}"#,
        r#"{"select":{"next_row":"J"}}"#,
        r#"{"select":{"next_row":["F1",1,"Ctrl+Enter"]}}"#,
        r#"{"search":{"close":["q"]}}"#,
    ] {
        assert!(!Keymap::parse(text).1.is_empty(), "{text}");
    }
    let errors =
        Keymap::parse(r#"{"select":{"next_row":["F1",1,"Ctrl+Enter"],"other":[]},"search":{}}"#).1;
    assert_eq!(errors.len(), 7, "{errors:?}");
}

#[test]
fn unknown_context_keeps_structure_and_key_errors_without_action_lookup_noise() {
    let errors = Keymap::parse(r#"{"typo":{"good":["q"],"empty":[],"bad":["F1"],"wrong":"j"},"notobject":[],"select":{"notaction":["F1"]}}"#).1;
    assert_eq!(
        errors,
        [
            "unknown context typo",
            "typo.empty: expected nonempty key list",
            "typo.bad: unknown key \"F1\"",
            "typo.wrong: expected nonempty key list",
            "unknown context notobject",
            "notobject: expected object of actions",
            "select: unknown action notaction",
            "select.notaction: unknown key \"F1\"",
        ]
    );
}

#[test]
fn duplicate_fields_do_not_hide_other_recoverable_diagnostics() {
    let errors = Keymap::parse(
        r#"{"select":{"next_row":["F1"],"next_row":["J"]},"select":{"previous_row":[]}}"#,
    )
    .1;
    assert!(
        errors
            .iter()
            .any(|e| e.contains("unknown key") && e.contains("F1")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("previous_row") && e.contains("nonempty")),
        "{errors:?}"
    );
    assert_eq!(
        errors
            .iter()
            .filter(|e| e.contains("duplicate field"))
            .count(),
        2
    );
}

#[test]
fn unreadable_and_dangling_jsonc_never_fall_back() {
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-config-errors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("keymaps.json"), "{}").unwrap();
    std::fs::create_dir(dir.join("keymaps.jsonc")).unwrap();
    let errors = Keymap::load(Some(&dir)).1;
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("directory"), "{errors:?}");
    assert!(!errors[0].contains("keymaps.jsonc"), "{errors:?}");
    std::fs::remove_dir(dir.join("keymaps.jsonc")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.join("absent"), dir.join("keymaps.jsonc")).unwrap();
        let errors = Keymap::load(Some(&dir)).1;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("No such file"), "{errors:?}");
        assert!(!errors[0].contains("keymaps.jsonc"), "{errors:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn key_syntax_is_case_sensitive_and_limited() {
    for good in ["+", "/", "é", "A", "Space", "Shift+Tab"] {
        assert!(Key::parse(good).is_some(), "{good}");
    }
    for bad in [
        "",
        "Ctrl+j",
        "Alt+Left",
        "enter",
        "CTRL+j",
        "Ctrl+J",
        "Ctrl+Alt+j",
        "Ctrl+Enter",
        "Alt+Tab",
        "Shift+j",
        "F1",
        "jj",
        "\n",
    ] {
        assert!(Key::parse(bad).is_none(), "{bad}");
    }
    let (map, errors) = Keymap::parse(r#"{"select":{"next_row":["Ctrl+i","Ctrl+m"]}}"#);
    assert_eq!(errors.len(), 2);
    assert_eq!(map.diagnostic_blocks()[0].1.len(), 2);
}

#[test]
fn fixed_search_ignores_all_modifiers_despite_mappings() {
    let (map, errors) =
        Keymap::parse(r#"{"select":{"close":["J","X"]},"help":{"close":["H","X"]}}"#);
    assert!(errors.is_empty());
    let mut state = ui();
    state.keymap = map;
    for help in [false, true] {
        if help {
            state.key(Key::Char('?'));
        }
        state.key(Key::Char('/'));
        state.key(Key::Char('x'));
        for key in [
            Key::Ctrl('j'),
            Key::Ctrl('h'),
            Key::Ctrl('i'),
            Key::Ctrl('m'),
            Key::Alt('x'),
            Key::CtrlArrow(Arrow::Up),
            Key::Tab,
        ] {
            assert!(state.key(key).is_none());
        }
        assert!(!state.closed);
        if help {
            assert!(state.help.searching);
            assert_eq!(state.help.query, "x");
        } else {
            assert_eq!(state.mode, crate::selection::Mode::Search);
            assert_eq!(state.query, "x");
        }
        state.key(Key::Backspace);
        state.key(Key::Char('y'));
        state.key(Key::Enter);
        if help {
            assert!(!state.help.searching);
            assert_eq!(state.help.query, "y");
        } else {
            assert_eq!(state.query, "y");
        }
    }
}

#[test]
fn error_page_takes_priority_at_small_sizes_and_scrolls_without_focus() {
    let mut state = ui();
    let actions: serde_json::Map<_, _> = (0..25)
        .map(|i| (format!("diagnosis-{i}"), serde_json::json!(["Ctrl+j"])))
        .collect();
    (state.keymap, state.keymap_errors) =
        Keymap::parse(&serde_json::json!({"select": actions}).to_string());
    for (width, height) in [(80, 15), (35, 7), (10, 3)] {
        state.popup_size = Some((width, height));
        assert!(
            state
                .layout(width, height, true)
                .iter()
                .take(height - 2)
                .all(|line| crate::presentation::columns(&without_sgr(line)) < width),
            "error details must not wrap the terminal"
        );
        let first = state.terminal_frame(width, height);
        assert!(first.contains("ERROR:"));
        if width >= 35 {
            assert!(first.to_lowercase().contains("keymap"));
        }
        assert!(!first.contains("minimum content"));
        for _ in 0..100 {
            state.key(Key::Down);
            state.terminal_frame(width, height);
        }
        assert!(state.error_scroll.get() > 0);
        state.key(Key::Up);
        assert!(!state.closed);
        assert_eq!(state.key(Key::Enter), None);
        assert!(state.closed);
        state.closed = false;
        state.error_scroll.set(0);
    }
}

#[test]
fn dynamic_help_exposes_both_replacements_footer_only_first_non_arrow() {
    let keys = vec!["A".to_string(), "Z".to_string()];
    let (map, errors) = Keymap::parse(&serde_json::json!({"select":{"next_row":keys}}).to_string());
    assert!(errors.is_empty());
    let mut state = ui();
    state.keymap = map;
    state.key(Key::Char('?'));
    let all = state.frame_width(80);
    for key in &keys {
        assert!(state.help.entries[0].keys.contains(key));
    }
    assert!(all.contains("next pane"));
    assert!(state
        .help
        .entries
        .iter()
        .any(|entry| entry.keys == "t" && entry.meaning == "choose theme"));
    let mut pages = String::new();
    for page in 0..30 {
        state.hint_scroll.set(page);
        pages.push_str(&state.hints(Context::Select, 80, 2, true).join("\n"));
    }
    assert!(pages.contains("A: ↓pane"));
    assert!(
        !pages.contains("Z"),
        "second non-arrow belongs only in full help"
    );
    assert!(!pages.contains("Down"), "replaced Down is absent");
    assert!(
        pages.contains("Shift+Tab")
            && pages.contains("focus")
            && pages.contains("exit")
            && pages.contains("keys")
    );
    assert!(
        !pages.contains(": theme") && !pages.contains(": search") && !pages.contains(": reset")
    );
}

#[test]
fn custom_help_retains_the_active_theme_selection_highlight() {
    let keys = ["A", "Z"];
    let (map, errors) = Keymap::parse(&serde_json::json!({"select":{"next_row":keys}}).to_string());
    assert!(errors.is_empty());
    let mut state = ui();
    state.keymap = map;
    state.key(Key::Char('?'));
    assert!(
        state.terminal_frame(80, 15).contains("\x1b[48;5;236m"),
        "wrapped help must retain Mocha selection styling"
    );
    state.theme_index = 6;
    assert!(
        state.terminal_frame(80, 15).contains("\x1b[48;2;70;63;93m"),
        "wrapped help must retain the active theme selection color"
    );
    state.help.query = "Z".into();
    assert!(
        state.terminal_frame(80, 15).contains("\x1b[4m"),
        "wrapped help must retain search match highlighting"
    );
}

#[test]
fn search_prompts_show_both_bindings_in_select_and_help() {
    let keys = ["A", "Z"];
    let (map, errors) = Keymap::parse(
        &serde_json::json!({"select":{"open_search":keys},"help":{"open_search":keys}}).to_string(),
    );
    assert!(errors.is_empty());
    let mut state = ui();
    state.keymap = map;
    for help in [false, true] {
        if help {
            state.key(Key::Char('?'));
        }
        let mut prompts = String::new();
        for page in 0..30 {
            state.hint_scroll.set(page);
            prompts.push_str(&state.layout(80, 15, true)[0]);
        }
        assert!(
            prompts.contains("Z"),
            "tail of open_search prompt must be accessible: {prompts}"
        );
    }
}

#[test]
fn documented_jsonc_examples_use_the_production_parser_and_schema() {
    let keymap = include_str!("../../docs/keymaps.md")
        .split("```jsonc\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let (map, errors) = Keymap::parse(keymap);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(map.labels(Context::Select, "next_row"), "J/Down");
    let status = include_str!("../../docs/status-colors.md")
        .split("```jsonc\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let parsed = config::parse(status).unwrap();
    assert!(parsed.errors.is_empty());
    assert_eq!(parsed.value["working"], "#123ABC");
}

#[test]
fn common_error_blocks_scroll_after_resize_and_never_resolve_custom_input() {
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::parse(
        r#"{"select":{"next_row":["A","B"],"previous_row":["A","B"],"focus_pane":["j","Down"],"close":["X"]},"typo":null}"#,
    );
    state
        .status_errors
        .push("Status colors invalid values", "working-color-diagnostic");
    state
        .status_errors
        .push("Status colors unknown keys", "unknown-status-diagnostic");
    state
        .theme_diagnostics
        .push("Theme invalid values", "theme-value-diagnostic");
    let selected = state.selected.clone();
    state.query = "preserved-filter".into();
    for (width, height) in [(40, 7), (100, 20), (10, 3), (40, 1), (35, 2), (80, 15)] {
        state.popup_size = Some((width, height));
        state.layout(width, height, true); // Clamp the prior viewport on resize.
        for _ in 0..200 {
            state.key(Key::Up);
        }
        let first = state.layout(width, height, true);
        for ignored in [
            Key::Char('y'),
            Key::Char('X'),
            Key::Ctrl('j'),
            Key::Alt('k'),
            Key::Char('?'),
        ] {
            assert_eq!(state.key(ignored), None);
            assert_eq!(state.error_scroll.get(), 0);
            assert!(!state.closed);
            assert_eq!(state.layout(width, height, true), first);
        }
        let mut visible = String::new();
        for at in 0..400 {
            let lines = state.layout(width, height, true);
            assert!(lines.len() <= height);
            let body = if height > 2 {
                assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
                &lines[..height - 2]
            } else {
                &lines[..]
            };
            assert!(body
                .iter()
                .all(|line| crate::presentation::columns(&without_sgr(line)) < width));
            if state.error_scroll.get() != at {
                // The final viewport can contain more than the first fragment.
                for line in body.iter().skip(1) {
                    visible.push_str(&without_sgr(line).replace("ERROR: ", ""));
                }
                break;
            }
            visible.push_str(&without_sgr(&body[0]).replace("ERROR: ", ""));
            assert_eq!(state.key(Key::Down), None);
        }
        for diagnostic in [
            "next_row, previous_row: A/B",
            "unknown context typo",
            "working-color-diagnostic",
            "unknown-status-diagnostic",
            "theme-value-diagnostic",
        ] {
            assert!(
                visible
                    .replace(' ', "")
                    .contains(&diagnostic.replace(' ', "")),
                "{width}x{height}: {visible}"
            );
        }
        assert_eq!(visible.contains('─'), height > 2, "{visible}");
        assert!(!visible.contains("minimum content"));
        assert_eq!(state.selected, selected);
        assert_eq!(state.query, "preserved-filter");
        for close in [Key::Enter, Key::Esc, Key::Char('q')] {
            assert_eq!(state.key(close), None);
            assert!(state.closed);
            state.closed = false;
        }
    }
    let mut status_only = ui();
    status_only
        .status_errors
        .push("Status colors structure", "expected JSON object");
    assert!(status_only.configuration_failed());
    assert_eq!(status_only.key(Key::Enter), None);
    assert!(status_only.closed);
}

#[test]
fn unreadable_jsonc_in_each_source_yields_only_io_without_legacy_fallback() {
    let dir = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-common-io-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for (name, legacy) in [
        ("keymaps", "{}"),
        ("theme", r#"{"theme":"dracula"}"#),
        ("status-colors", "{}"),
    ] {
        std::fs::create_dir(dir.join(format!("{name}.jsonc"))).unwrap();
        std::fs::write(dir.join(format!("{name}.json")), legacy).unwrap();
    }
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::load(Some(&dir));
    (state.status_colors, state.status_errors) =
        crate::status_colors::StatusColors::load_diagnostics(Some(&dir));
    (state.theme_index, state.theme_diagnostics) = crate::theme::load_diagnostics(Some(&dir));
    let frame = state.frame_width(160);
    assert!(frame.starts_with("ERROR:"), "{frame}");
    for source in ["Keymap IO", "Theme IO", "Status colors IO"] {
        assert!(frame.contains(source), "{frame}");
    }
    assert_eq!(frame.matches("directory").count(), 3, "{frame}");
    assert!(
        !frame.contains("structure")
            && !frame.contains("missing theme")
            && !frame.contains("must be"),
        "{frame}"
    );
    let lines: Vec<_> = frame.lines().collect();
    for (heading, name) in [
        ("Keymap IO", "keymaps"),
        ("Theme IO", "theme"),
        ("Status colors IO", "status-colors"),
    ] {
        let at = lines
            .iter()
            .position(|line| *line == format!("ERROR: {heading}"))
            .unwrap();
        assert_eq!(
            lines[at + 1],
            format!(
                "{name}.jsonc  {}",
                dir.join(format!("{name}.jsonc")).display()
            )
        );
    }
    assert_eq!(
        frame.matches(&dir.display().to_string()).count(),
        3,
        "{frame}"
    );
    assert_eq!(state.theme_index, 0);
    for height in [1, 2] {
        let mut visible = String::new();
        for start in 0..20 {
            state.error_scroll.set(start);
            let rows = state.layout(40, height, true);
            assert!(rows.iter().all(|row| !row.contains('─')), "{rows:?}");
            if state.error_scroll.get() != start {
                for row in rows.iter().skip(1) {
                    visible.push_str(&without_sgr(row));
                }
                break;
            }
            visible.push_str(&without_sgr(&rows[0]));
        }
        assert_eq!(visible.matches("directory").count(), 3, "{visible}");
        assert!(!visible.contains(".json"), "{visible}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn themes_help_default_remap_and_collisions_follow_existing_validation() {
    use crate::keymap::{Action, Context, Keymap};
    assert_eq!(
        Keymap::default().action(Context::Themes, crate::input::Key::Char('?')),
        Some(Action::OpenHelp)
    );
    let (map, errors) = Keymap::parse(r#"{"themes":{"open_help":["H","Right"]}}"#);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(map.display_labels(Context::Themes, "open_help"), "H/→");
    let (_, errors) = Keymap::parse(r#"{"themes":{"save":["?"]}}"#);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("themes | ?") && e.contains("save") && e.contains("open_help")),
        "{errors:?}"
    );
    let (map, _) = Keymap::parse(r#"{"themes":{"open_help":["H"],"open_help":["?"]}}"#);
    assert!(map
        .diagnostic_blocks()
        .iter()
        .any(|(heading, details)| heading.contains("Duplicate")
            && details.iter().any(|d| d.contains("open_help"))));
}
