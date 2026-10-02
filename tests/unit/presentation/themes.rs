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
