use crate::presentation::StatusColors;
use crate::theme::{auto_choice_at, load, save, saved_choice, Theme, NAMES};
use ratatui::style::Color;
use std::path::PathBuf;

#[test]
fn r2_theme_file_notices_sanitize_decoded_controls_before_styling_and_clipping() {
    use ratatui::buffer::CellWidth;
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-r2-notice-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for content in [
        r#"{"theme":"\u001b[2J\n\r\t\u0000bad"}"#,
        r#"{"theme":"dracula","\u001b[2J\n\u007fbad":true}"#,
        r#"{"theme":"unknown"}"#,
    ] {
        std::fs::write(dir.join("theme.jsonc"), content).unwrap();
        let mut state = crate::test_support::ui();
        (state.theme_index, state.theme_error) = load(Some(&dir));
        state.theme_diagnostics = crate::theme::load_diagnostics(Some(&dir)).1;
        assert_eq!(
            state.theme_diagnostics.source,
            Some(dir.join("theme.jsonc"))
        );
        assert!(!state.configuration_failed());
        assert_eq!(
            state.theme().background,
            Theme::named("catppuccin").background
        );
        for width in [80, 120, 240] {
            let plain = state.layout(width, 15, false);
            let notice = plain.iter().find(|line| line.contains("ERROR ·")).unwrap();
            assert!(!notice.chars().any(char::is_control), "{notice:?}");
            assert!(usize::from(notice.cell_width()) < width, "{notice:?}");
            assert!(!notice.contains("theme.json") && !notice.contains(&dir.display().to_string()));
            if width == 240 {
                assert!(notice.ends_with("; using Mocha"), "{notice:?}");
                assert_eq!(
                    notice,
                    &format!(
                        "  ERROR · {}",
                        crate::presentation::clean(state.theme_error.as_ref().unwrap())
                    )
                );
            }
            let frame = state.terminal_frame(width, 15);
            assert!(!frame.contains("\x1b[2J"), "{frame:?}");
            assert_eq!(frame.split("\r\n").count(), 15, "{frame:?}");
            assert!(frame.contains("\x1b[1m\x1b[38;2;243;139;168mERROR"));
            assert!(frame.contains("Claude"), "normal UI must remain: {frame:?}");
        }
    }
    for error in [false, true] {
        let mut state = crate::test_support::ui();
        let message = "message\x1b[2J\n\t\u{7f}; not saved";
        state.theme_error = error.then(|| message.into());
        state.auto_toast = (!error).then(|| message.into());
        let plain = state.layout(120, 15, false);
        let kind = if error { "ERROR" } else { "INFO" };
        assert!(plain.contains(&format!(
            "  {kind} · {}",
            crate::presentation::clean(message)
        )));
        let frame = state.terminal_frame(120, 15);
        assert!(!frame.contains("\x1b[2J"));
        assert_eq!(frame.split("\r\n").count(), 15);
        assert!(frame.contains(if error {
            "\x1b[1m\x1b[38;2;243;139;168mERROR"
        } else {
            "\x1b[38;2;108;112;134mINFO"
        }));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn r2_theme_and_status_duplicate_messages_use_structured_locations_without_root_markers() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-r2-duplicates-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, content, expected, unknown_heading) in [
        (
            "theme",
            r#"{"theme":"nord","th\u0065me":"dracula","extra":{"nested":[{"field":1,"field":2}]},"other":{"x":1,"x":2}}"#,
            vec![
                "duplicate field theme",
                "extra.nested[0]: duplicate field field",
                "other: duplicate field x",
            ],
            "Theme unknown fields",
        ),
        (
            "status-colors",
            r##"{"working":"#123456","wor\u006bing":"bad","extra":{"nested":[{"field":1,"field":2}]},"other":{"x":1,"x":2}}"##,
            vec![
                "duplicate field working",
                "extra.nested[0]: duplicate field field",
                "other: duplicate field x",
            ],
            "Status colors unknown keys",
        ),
    ] {
        let path = dir.join(format!("{name}.jsonc"));
        std::fs::write(&path, content).unwrap();
        let diagnostics = if name == "theme" {
            crate::theme::load_diagnostics(Some(&dir)).1
        } else {
            StatusColors::load_diagnostics(Some(&dir)).1
        };
        assert_eq!(diagnostics.source.as_ref(), Some(&path));
        let duplicates = &diagnostics
            .blocks
            .iter()
            .find(|(heading, _)| heading.ends_with("duplicate fields"))
            .unwrap()
            .1;
        for message in expected {
            assert!(duplicates.contains(&message.to_owned()), "{duplicates:?}");
        }
        assert_eq!(duplicates.len(), 3, "no duplicate may be lost");
        assert!(diagnostics
            .blocks
            .iter()
            .any(|(heading, messages)| heading == unknown_heading && messages.len() == 2));
        if name == "status-colors" {
            assert!(diagnostics
                .messages()
                .contains(&"working must be #RRGGBB".to_owned()));
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a10c_jsonc_theme_precedes_legacy_and_accepts_comments_and_trailing_comma() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-jsonc-red-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("theme.json"), "{\"theme\":\"nord\"}").unwrap();
    std::fs::write(
        dir.join("theme.jsonc"),
        "{/* choice */\"theme\":\"dracula\",}// end",
    )
    .unwrap();
    let actual = load(Some(&dir));
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        actual,
        (6, None),
        "JSONC must take priority over legacy JSON"
    );
}

#[test]
fn a10c_explicit_save_keeps_legacy_and_auto_removes_both_without_start_migration() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-auto-both-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("theme.json"), "{\"theme\":\"nord\"}").unwrap();
    assert_eq!(load(Some(&dir)), (7, None));
    assert!(!dir.join("theme.jsonc").exists());
    save(Some(&dir), 6).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.jsonc")).unwrap(),
        "{\"theme\":\"dracula\"}\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.json")).unwrap(),
        "{\"theme\":\"nord\"}"
    );
    save(Some(&dir), 0).unwrap();
    assert!(!dir.join("theme.jsonc").exists() && !dir.join("theme.json").exists());
    std::fs::write(dir.join("theme.jsonc"), "{\"theme\":\"dracula\"}").unwrap();
    std::fs::create_dir(dir.join("theme.json")).unwrap();
    let error = save(Some(&dir), 0).unwrap_err().to_string();
    assert!(error.contains("directory"), "{error}");
    assert!(
        !error.contains("theme.json") && !error.contains(&dir.display().to_string()),
        "{error}"
    );
    assert!(!dir.join("theme.jsonc").exists());
    let mut state = crate::test_support::ui();
    state.config_dir = Some(dir.clone());
    state.theme_index = 6;
    state.key(crate::input::Key::Char('t'));
    state.theme_cursor = 0;
    state.key(crate::input::Key::Enter);
    assert_eq!(state.theme_index, 6);
    assert_eq!(state.mode, crate::selection::Mode::Themes);
    assert!(state.theme_error.as_ref().unwrap().contains("not saved"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn theme_file_round_trip_auto_delete_and_invalid_input() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-theme-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(load(Some(&dir)), (0, None));
    save(Some(&dir), 5).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.jsonc")).unwrap(),
        "{\"theme\":\"tokyo-night-day\"}\n"
    );
    assert_eq!(load(Some(&dir)), (5, None));
    for content in [
        "{",
        "{\"theme\":\"unknown\"}",
        "{\"theme\":\"dracula\",\"extra\":true}",
    ] {
        std::fs::write(dir.join("theme.jsonc"), content).unwrap();
        assert!(load(Some(&dir)).1.is_some());
        assert_eq!(
            std::fs::read_to_string(dir.join("theme.jsonc")).unwrap(),
            content
        );
    }
    save(Some(&dir), 0).unwrap();
    assert!(!dir.join("theme.jsonc").exists());
    std::fs::create_dir(dir.join("theme.jsonc")).unwrap();
    assert!(load(Some(&dir)).1.is_some());
    assert!(save(Some(&dir), 0).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn terminal_palette_uses_terminal_colors_and_statuses_follow_the_theme() {
    let palette = Theme::named("terminal");
    assert_eq!(palette.text, Color::Reset);
    assert_eq!(palette.working, Color::Yellow);
    assert_eq!(palette.blocked, Color::LightRed);
    assert_eq!(NAMES.len(), 19);
    assert_eq!(Theme::named("dracula").accent, Color::Rgb(189, 147, 249));
}

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
fn saved_herdr_choice_requires_single_plain_manual_theme() {
    assert_eq!(
        saved_choice("[ui]\nfoo = 1\n[theme]\nname = \"dracula\"\n").0,
        "dracula"
    );
    for source in [
        "[theme]\nname = \"dracula\"\nauto_switch = true",
        "[theme]\nname = \"dracula\"\n[theme.custom]\naccent = \"#fff\"",
        "[theme]\nname = \"dracula\"\n[theme]\nname = \"nord\"",
        "[theme]\nname = \"dracula\"\nauto_switch = false\nauto_switch = false",
        "[theme]\nname = \"catppuccin-mocha\"",
        "[theme]\nname = \"dracula\"\n[ui]\ninvalid toml",
        "[theme]\nname = \"",
    ] {
        assert_eq!(saved_choice(source).0, "catppuccin", "{source}");
    }
}

#[test]
fn saved_herdr_choice_reports_auto_picker_label_as_unknown() {
    for source in [
        "[theme]\nname = \"Auto (Herdr)\"",
        "[theme]\nname = \"Auto (Herdr)\"\nauto_switch = false",
    ] {
        assert_eq!(
            saved_choice(source),
            ("catppuccin", Some("Auto (Herdr)".into())),
            "{source}"
        );
    }
}

#[test]
fn saved_herdr_choice_rejects_malformed_table_headers() {
    for source in [
        "[theme]\nname = \"dracula\"\n[broken",
        "[broken\n[theme]\nname = \"dracula\"",
        "[theme]\nname = \"dracula\"\n[broken] trailing",
        "[theme]\nname = \"dracula\"\n[[broken]",
        "[theme]\nname = \"dracula\"\n[]",
    ] {
        assert_eq!(saved_choice(source).0, "catppuccin", "{source}");
    }
}

#[test]
fn unsupported_manual_name_is_reported_only_when_literal_and_unambiguous() {
    assert_eq!(
        saved_choice("[ui]\nfoo = 1\n[theme]\nname = \"catppuccin-mocha\"\nauto_switch = false\n"),
        ("catppuccin", Some("catppuccin-mocha".into()))
    );
    assert_eq!(
        saved_choice("[theme]\nname = \"my new.theme\""),
        ("catppuccin", Some("my new.theme".into()))
    );
    for source in [
        "",
        "[theme]\n",
        "[theme]\nname = \"dracula\"",
        "[theme]\nname = \"catppuccin-mocha\"\nauto_switch = true",
        "[theme]\nname = \"catppuccin-mocha\"\nname = \"nord\"",
        "[theme]\nname = \"catppuccin-mocha\"\n[theme.custom]\nfoo = 1",
        "[theme]\nname = \"catppuccin-mocha\"\n[broken",
        "[theme]\nname = \"catppuccin-mocha\"\ninvalid toml",
        "[theme]\nname = \"catppuccin-mocha\"\n[ui]\nfoo = 1\nfoo = 2",
        "[theme]\nname = \"catppuccin-mocha\"\n[[ui]]\nfoo = 1",
        "[theme]\nname = \"catppuccin-mocha\" trailing",
        "[theme]\nname = \"catppuccin-\\\"mocha\"",
    ] {
        assert_eq!(saved_choice(source).1, None, "{source}");
    }
}

#[test]
fn shared_layout_fixtures_use_mocha_without_an_auto_notice() {
    use crate::test_support::{render, ui, SNAPSHOT};

    let state = ui();
    assert_eq!(state.theme_index, 0);
    assert_eq!(state.auto_name, "catppuccin");
    assert_eq!(state.auto_unsupported, None);
    assert_eq!(state.theme_notice(), None);
    assert_eq!(state.theme().accent, Theme::named("catppuccin").accent);
    assert_eq!(
        render(SNAPSHOT, Some("p-self"), 160).unwrap(),
        state.frame_width(160)
    );
}

#[test]
fn auto_choice_reads_supported_manual_themes_from_explicit_paths() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-supported-auto-choice-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    for name in ["dracula", "terminal"] {
        let content = format!("[theme]\nname = \"{name}\"\n");
        std::fs::write(&path, &content).unwrap();
        assert_eq!(auto_choice_at(Some(&path), false), (name, None));
        assert_eq!(auto_choice_at(Some(&path), true), ("catppuccin", None));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn auto_choice_silently_uses_mocha_for_absent_unreadable_and_ssh_config() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-auto-choice-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(auto_choice_at(None, false), ("catppuccin", None));
    assert_eq!(
        auto_choice_at(Some(&dir.join("absent")), false),
        ("catppuccin", None)
    );
    assert_eq!(auto_choice_at(Some(&dir), false), ("catppuccin", None));
    let path = dir.join("config.toml");
    std::fs::write(&path, "[theme]\nname = \"catppuccin-mocha\"\n").unwrap();
    assert_eq!(auto_choice_at(Some(&path), true), ("catppuccin", None));
    assert_eq!(
        auto_choice_at(Some(&path), false),
        ("catppuccin", Some("catppuccin-mocha".into()))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_saved_plugin_theme_remains_an_error_without_changing_its_file() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-unknown-json-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let content = "{\"theme\":\"catppuccin-mocha\"}\n";
    std::fs::write(dir.join("theme.json"), content).unwrap();
    assert_eq!(
        load(Some(&dir)),
        (
            0,
            Some("Theme configuration: unknown theme catppuccin-mocha; using Mocha".into())
        )
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("theme.json")).unwrap(),
        content
    );
    std::fs::remove_dir_all(dir).unwrap();
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

#[test]
fn notice_uses_active_light_or_terminal_palette_and_clips_long_messages() {
    use crate::selection::Mode;
    use crate::test_support::ui;

    for (index, warning, info) in [
        (2, "\x1b[38;2;210;15;57m", "\x1b[38;2;156;160;176m"),
        (3, "\x1b[91m", "\x1b[37m"),
    ] {
        let mut state = ui();
        state.theme_index = index;
        state.theme_cursor = index;
        state.mode = Mode::Themes;
        for error in [true, false] {
            state.theme_error = error.then(|| format!("theme.json {}", "x".repeat(250)));
            state.auto_toast = (!error).then(|| format!("Herdr theme {}", "x".repeat(250)));
            for table in [false, true] {
                state.theme_table_preview = table;
                let frame = state.terminal_frame(80, 15);
                let lines: Vec<_> = frame.split("\r\n").collect();
                assert_eq!(lines.len(), 15);
                assert!(
                    lines[12].contains(if error { warning } else { info }),
                    "{frame:?}"
                );
                assert!(lines[12].contains('…'), "{frame:?}");
                let hints = state.hints(crate::keymap::Context::Themes, 80, 2, true);
                assert!(lines[14].contains(&hints[1]), "{frame:?}");
                let text = lines[12]
                    .split_once(if error { warning } else { info })
                    .unwrap()
                    .1
                    .split("\x1b[")
                    .next()
                    .unwrap();
                assert!(crate::presentation::columns(text) + 2 < 80, "{frame:?}");
            }
        }
    }
}

#[test]
fn replacing_theme_does_not_truncate_the_previous_choice() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-theme-replace-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("theme.jsonc");
    std::fs::write(&path, "{\"theme\":\"dracula\"}\n").unwrap();
    let previous = dir.join("previous.json");
    std::fs::hard_link(&path, &previous).unwrap();

    save(Some(&dir), 5).unwrap();
    assert_eq!(load(Some(&dir)), (5, None));
    assert_eq!(
        std::fs::read_to_string(&previous).unwrap(),
        "{\"theme\":\"dracula\"}\n"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_replacement_leaves_target_and_no_temporary_file() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-theme-failure-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let target = dir.join("theme.jsonc");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("existing"), "keep").unwrap();

    assert!(save(Some(&dir), 5).is_err());
    assert_eq!(
        std::fs::read_to_string(target.join("existing")).unwrap(),
        "keep"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn status_file_overrides_only_explicit_roles_across_theme_switches() {
    let dir = PathBuf::from(format!(
        "/tmp/opencode/crossroads-status-theme-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("status-colors.json"), "{\"blocked\":\"#010203\"}").unwrap();
    let status = StatusColors::load(Some(&dir)).unwrap();
    let dracula = Theme::named("dracula");
    let nord = Theme::named("nord");
    assert_eq!(status.themed("blocked", dracula), Color::Rgb(1, 2, 3));
    assert_eq!(status.themed("blocked", nord), Color::Rgb(1, 2, 3));
    assert_eq!(status.themed("working", dracula), dracula.working);
    assert_eq!(status.themed("working", nord), nord.working);
    std::fs::remove_dir_all(dir).unwrap();
}
