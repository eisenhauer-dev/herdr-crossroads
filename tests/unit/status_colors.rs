use crate::status_colors::StatusColors;
use crate::theme::Theme;
use ratatui::style::Color;
use std::path::PathBuf;

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
