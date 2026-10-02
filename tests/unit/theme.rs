use crate::status_colors::StatusColors;
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
