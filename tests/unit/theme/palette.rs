use crate::theme::{Theme, NAMES};
use ratatui::style::Color;

#[test]
fn terminal_palette_uses_terminal_colors_and_statuses_follow_the_theme() {
    let palette = Theme::named("terminal");
    assert_eq!(palette.text, Color::Reset);
    assert_eq!(palette.working, Color::Yellow);
    assert_eq!(palette.blocked, Color::LightRed);
    assert_eq!(NAMES.len(), 19);
    assert_eq!(Theme::named("dracula").accent, Color::Rgb(189, 147, 249));
}
