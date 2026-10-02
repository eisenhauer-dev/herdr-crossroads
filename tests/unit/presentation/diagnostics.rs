use super::without_sgr;
use crate::input::Key;
use crate::keymap::Keymap;
use crate::test_support::ui;

#[test]
fn error_blocks_have_individual_highlighted_headings_and_bottom_full_width_footer() {
    let mut state = ui();
    (state.keymap, state.keymap_errors) =
        Keymap::parse(r#"{"select":{"next_row":["J","J","J"],"next_tab":["q"],"broken":null}}"#);
    state
        .status_errors
        .push("Status colors invalid values", "working must be #RRGGBB");
    state
        .theme_diagnostics
        .push("Theme invalid values", "unknown theme bogus");
    let plain = state.frame_width(240);
    assert!(!plain.contains("Other diagnostics"), "{plain}");
    assert!(!plain.contains("Keymap input diagnostics"), "{plain}");
    assert!(
        !plain.contains("ERROR: Configuration\n")
            && !plain.contains("ERROR: Keymap configuration\n"),
        "{plain}"
    );
    for heading in [
        "Maximum 2 keybinds per action - select mode",
        "Unknown keymap actions - select mode",
        "Keymap structure - select mode",
        "Keymap collisions - select mode",
        "Status colors invalid values",
        "Theme invalid values",
    ] {
        assert!(plain.contains(&format!("ERROR: {heading}\n")), "{plain}");
    }
    assert!(!plain.contains("\n\n"), "plaintext must not pad: {plain}");
    state.error_scroll.set(usize::MAX);
    assert_eq!(
        state.frame_width(240),
        plain,
        "plaintext includes all blocks regardless of terminal scroll"
    );
    for theme_index in [1, 2, 3] {
        state.theme_index = theme_index;
        for (width, height) in [(240, 36), (162, 36), (80, 15), (40, 7), (10, 3)] {
            state.error_scroll.set(0);
            let first = state.layout(width, height, true);
            assert_eq!(first.len(), height);
            assert_eq!(without_sgr(&first[height - 2]), "─".repeat(width));
            let footer = first[height - 1].clone();
            assert!(without_sgr(&footer).starts_with(if width >= 40 {
                "↑/k: scroll up"
            } else {
                "↑/k:"
            }));
            let color = match state.theme().blocked {
                ratatui::style::Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
                ratatui::style::Color::Reset => "\x1b[39m".into(),
                ratatui::style::Color::LightRed => "\x1b[91m".into(),
                color => panic!("unexpected test palette color: {color:?}"),
            };
            let expected_style = format!("\x1b[1m{color}");
            let mut seen = String::new();
            for at in 0..200 {
                state.error_scroll.set(at);
                let lines = state.layout(width, height, true);
                assert_eq!(lines[height - 1], footer);
                assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
                for line in &lines[..height - 2] {
                    let text = without_sgr(line);
                    assert!(crate::presentation::columns(&text) <= width);
                    if text.starts_with("ERROR:") {
                        assert!(line.starts_with(&expected_style), "{line:?}");
                        assert!(line.contains("\x1b[22m"));
                    } else {
                        assert!(!line.contains("\x1b[1m"), "{line:?}");
                    }
                    seen.push_str(&text);
                }
                if state.error_scroll.get() != at {
                    break;
                }
            }
            assert!(seen.contains("broken"), "{width}x{height}: {seen}");
            assert!(
                seen.replace(' ', "").contains("unknownthemebogus"),
                "{seen}"
            );
        }
    }
}

#[test]
fn error_footer_stays_at_absolute_bottom_across_scroll_and_resize() {
    let mut state = ui();
    let actions: serde_json::Map<_, _> = (0..60)
        .map(|at| (format!("diagnostic-{at}"), serde_json::json!(["J"])))
        .collect();
    (state.keymap, state.keymap_errors) =
        Keymap::parse(&serde_json::json!({"select": actions}).to_string());
    for (width, height) in [(240, 36), (80, 15), (40, 2), (40, 1), (40, 3), (162, 36)] {
        let lines = state.layout(width, height, true);
        if height <= 2 {
            assert!(
                lines
                    .iter()
                    .all(|line| without_sgr(line).starts_with("ERROR:")),
                "{lines:?}"
            );
            assert!(without_sgr(&lines[0]).contains("need 80x15; actual"));
            continue;
        }
        assert_eq!(lines.len(), height);
        assert_eq!(without_sgr(&lines[height - 2]), "─".repeat(width));
        let footer = lines[height - 1].clone();
        state.key(Key::Down);
        let scrolled = state.layout(width, height, true);
        assert_ne!(scrolled[..height - 2], lines[..height - 2]);
        assert_eq!(scrolled[height - 1], footer);
        assert_eq!(without_sgr(&scrolled[height - 2]), "─".repeat(width));
        state.key(Key::Up);
        assert_eq!(state.layout(width, height, true), lines);
    }
}

#[test]
fn tiny_error_marker_reserves_width_without_clipping_diagnostic_fragments() {
    let diagnostic = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    for width in [10, 20, 40] {
        for height in [1, 2] {
            let mut state = ui();
            (state.keymap, state.keymap_errors) =
                Keymap::parse(&serde_json::json!({"select": {diagnostic: ["J"]}}).to_string());
            let mut visible = String::new();
            for start in 0..200 {
                state.error_scroll.set(start);
                let lines = state.layout(width, height, true);
                assert!(lines.iter().all(|line| !line.contains('─')), "{lines:?}");
                if state.error_scroll.get() != start {
                    for line in lines.iter().skip(1) {
                        visible.push_str(without_sgr(line).strip_prefix("ERROR: ").unwrap());
                    }
                    break;
                }
                let text = without_sgr(&lines[0]);
                assert!(crate::presentation::columns(&text) < width, "{text}");
                visible.push_str(text.strip_prefix("ERROR: ").unwrap());
            }
            assert_eq!(
                visible.replace(' ', ""),
                format!("need80x15;actual{width}x{height}.Enlargepopupandreopen.{diagnostic}"),
                "{width}x{height}"
            );
            assert!(visible.ends_with(diagnostic), "{visible}");
        }
    }
}

#[test]
fn painted_error_footer_fills_last_cell_without_erasing_it() {
    use ratatui::{buffer::CellWidth, style::Color};
    let mut state = ui();
    (state.keymap, state.keymap_errors) = Keymap::parse(r#"{"select":{"broken":null}}"#);
    for theme_index in 0..crate::theme::NAMES.len() {
        state.theme_index = theme_index;
        for (width, height) in [(10, 3), (40, 7), (80, 15), (162, 36), (240, 36)] {
            state.error_scroll.set(0);
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height);
            for (at, line) in lines.iter().enumerate() {
                let cells = usize::from(without_sgr(&line.replace("\x1b[K", "")).cell_width());
                assert!(cells <= width, "{width}x{height}: {line:?}");
                if at == height - 2 {
                    assert_eq!(cells, width);
                    assert!(
                        !line.contains("\x1b[K"),
                        "full-width footer erased: {line:?}"
                    );
                } else if state.theme().background != Color::Reset {
                    assert!(line.ends_with("\x1b[K"), "short row must clear: {line:?}");
                }
            }
        }
    }
}

#[test]
fn keymap_error_marker_matches_theme_error_style_without_hiding_details() {
    use crate::keymap::Keymap;

    for theme_index in [1, 2, 3] {
        for config in [
            r#"{"select":{"next_row":["J","J","J"],"previous_row":["K","K","K"]}}"#,
            r#"{"select":{"next_row":["J","J","J"],"broken":null}}"#,
            r#"{"select":{"broken":null}}"#,
        ] {
            let mut state = super::test_support::ui();
            state.theme_index = theme_index;
            state.theme_error = Some("unknown theme".into());
            let notice_frame = state.terminal_frame(100, 15);
            let notice = notice_frame
                .split("\r\n")
                .find(|line| line.contains("ERROR"))
                .unwrap();
            let start = notice.find("\x1b[1m").unwrap();
            let error_style = &notice[start..notice.find("ERROR").unwrap()];
            let end = notice.find("\x1b[22m").unwrap();
            let reset_end = notice[end + 5..].find('m').unwrap() + end + 6;
            let text_reset = &notice[end..reset_end];
            state.theme_error = None;
            (state.keymap, state.keymap_errors) = Keymap::parse(config);

            let error_frame = state.terminal_frame(100, 15);
            assert!(
                error_frame.contains(error_style),
                "must match theme ERROR style: {error_frame:?}"
            );

            let plain = state.frame_width(100);
            assert!(plain.contains("ERROR"), "{plain}");
            assert!(!plain.contains('\x1b'), "{plain:?}");
            if !state.keymap.oversized_actions.is_empty() {
                assert_eq!(
                    plain
                        .matches("Maximum 2 keybinds per action - select mode")
                        .count(),
                    1
                );
                assert!(!plain.contains("Context |"));
                assert!(plain.contains("next_row"));
                assert!(plain.starts_with("ERROR: Maximum 2 keybinds per action - select mode\n"));
                if config.contains("previous_row") {
                    assert!(plain.contains("next_row\nprevious_row"));
                }
            }
            for height in [1, 2, 3, 15] {
                let frame = state.terminal_frame(100, height);
                let lines: Vec<_> = frame.split("\r\n").collect();
                let header = lines[0];
                assert!(header.contains("ERROR"), "{height}: {frame:?}");
                assert!(
                    header.contains(&format!("{error_style}ERROR:")),
                    "{height}: {frame:?}"
                );
                assert!(header.contains(text_reset), "{height}: {frame:?}");
                assert!(
                    !header.contains("\x1b[0m"),
                    "background must survive: {frame:?}"
                );
                let diagnostic_frame = if height < 15 {
                    state.error_scroll.set(if height <= 2 { 2 } else { 5 });
                    let scrolled = state.terminal_frame(100, height);
                    if height == 3 {
                        assert_eq!(scrolled.split("\r\n").nth(1), Some(lines[1]));
                        assert_eq!(scrolled.split("\r\n").nth(2), Some(lines[2]));
                        assert!(!scrolled.split("\r\n").next().unwrap().contains("\x1b[1m"));
                    }
                    state.error_scroll.set(0);
                    scrolled
                } else {
                    frame.clone()
                };
                assert!(
                    diagnostic_frame.contains(if state.keymap.oversized_actions.is_empty() {
                        "broken"
                    } else {
                        "next_row"
                    }),
                    "diagnostic lost at {height}: {diagnostic_frame:?}"
                );
                if height >= 3 {
                    let detail = lines[1];
                    assert!(
                        !detail.contains("\x1b[1m"),
                        "details must remain normal text: {frame:?}"
                    );
                }
                if theme_index != 3 {
                    let background = notice.split("\x1b[38;").next().unwrap();
                    assert!(
                        lines.iter().all(|line| line.starts_with(background)),
                        "{frame:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn one_and_two_row_error_pages_still_expose_diagnostics() {
    let mut state = ui();
    (state.keymap, state.keymap_errors) =
        Keymap::parse(r#"{"select":{"unique-diagnosis-123":["J"]}}"#);
    for height in [1, 2] {
        state.error_scroll.set(0);
        let lines = state.layout(40, height, true);
        assert!(lines[0].contains("need 80x15"), "{lines:?}");
        state.error_scroll.set(2);
        let lines = state.layout(40, height, true);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("unique-diagnosis-123")),
            "{lines:?}"
        );
        assert!(lines.len() <= height);
    }
}
