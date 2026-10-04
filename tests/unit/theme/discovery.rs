use crate::theme::{auto_choice_at, saved_choice};
use std::path::PathBuf;

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
