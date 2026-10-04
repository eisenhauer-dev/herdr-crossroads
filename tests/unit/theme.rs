#[path = "theme/discovery.rs"]
mod discovery;
use crate::theme::Theme;

#[path = "theme/palette.rs"]
mod palette;
#[path = "theme/persistence.rs"]
mod persistence;

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
