#![cfg(unix)]
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn case(files: &[(&str, &str)], input: &[u8]) -> std::process::Output {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-a10c-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let fake = dir.join("herdr");
    std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$*\" = 'api snapshot' ]; then cat '{}'; else echo '{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}'; fi\n", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/topology.json"))).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    for (name, content) in files {
        std::fs::write(dir.join(name), content).unwrap();
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .env("HERDR_BIN_PATH", fake)
        .env("HERDR_PLUGIN_CONFIG_DIR", &dir)
        .env("HERDR_PANE_ID", "p-self")
        .env_remove("HERDR_SOCKET_PATH")
        .env("COLUMNS", "160")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    output
}

fn diagnostics_without_source_rows(output: &std::process::Output) -> String {
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = text.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("ERROR:") {
            assert!(
                lines[at + 1]
                    .split_once("  ")
                    .is_some_and(|(filename, path)| path
                        .starts_with("/tmp/opencode/crossroads-a10c-")
                        && std::path::Path::new(path)
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            == filename),
                "source missing immediately under heading: {text}"
            );
        }
        if line.contains("/tmp/opencode/") {
            assert!(
                at > 0 && lines[at - 1].starts_with("ERROR:"),
                "source repeated in diagnostics: {text}"
            );
        }
    }
    lines
        .into_iter()
        .filter(|line| !line.contains("/tmp/opencode/"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn oversized_lists_open_startup_error_page_before_dedup() {
    for (config, heading, row) in [
        (r#"{"select":{"close":["q","q","q"]}}"#, "select", "close"),
        (
            r#"{"themes":{"cancel":["Esc","q","X"]}}"#,
            "themes",
            "cancel",
        ),
    ] {
        let output = case(&[("keymaps.jsonc", config), ("keymaps.json", "{}")], b"q");
        let text = diagnostics_without_source_rows(&output);
        assert!(
            output.status.success()
                && text.contains("ERROR")
                && !text.contains('\x1b')
                && text.matches("Maximum 2 keybinds per action - ").count() == 1
                && text.contains(&format!(
                    "ERROR: Maximum 2 keybinds per action - {heading} mode\n{row}"
                ))
                && !text.contains("Context |")
                && text.contains(row),
            "{text}"
        );
        assert!(!text.contains("Agent/Pane Name"), "{text}");
        assert!(
            !text.contains("keymaps.jsonc") && !text.contains("/tmp/opencode"),
            "{text}"
        );
    }
}

#[test]
fn startup_overflow_list_aggregates_actions_and_separates_mixed_errors() {
    for mixed in [false, true] {
        let extra = if mixed {
            r#", "next_tab":["F1","q"],"broken":null"#
        } else {
            ""
        };
        let config = format!(
            r#"{{"select":{{"next_row":["J","J","J"],"previous_row":["K","K","K"]{extra}}},"help":{{"next_category":["A","B","C"]}}}}"#
        );
        let output = case(&[("keymaps.jsonc", &config)], b"\r");
        assert!(output.status.success());
        let text = diagnostics_without_source_rows(&output);
        assert_eq!(
            text.matches("Maximum 2 keybinds per action - ").count(),
            2,
            "{text}"
        );
        for row in [
            "next_row",
            "previous_row",
            "ERROR: Maximum 2 keybinds per action - help mode\nnext_category",
        ] {
            assert!(text.contains(row), "{text}");
        }
        assert!(
            text.starts_with("ERROR: Maximum 2 keybinds per action - select mode\n"),
            "{text}"
        );
        assert!(text.contains("next_row\nprevious_row"), "{text}");
        assert!(!text.contains("Context |"), "{text}");
        assert!(
            !text
                .split("ERROR: Keymap")
                .next()
                .unwrap()
                .contains("keymaps.jsonc"),
            "{text}"
        );
        if mixed {
            for diagnostic in [
                "ERROR: Unknown keymap keys - select mode",
                "unknown key",
                "broken: expected nonempty",
                "next_tab, close",
            ] {
                assert!(text.contains(diagnostic), "{text}");
            }
        }
        assert!(
            !text.contains("keymaps.jsonc") && !text.contains("/tmp/opencode"),
            "{text}"
        );
        assert!(!text.contains("Agent/Pane Name"), "{text}");
    }
}

#[test]
fn replacement_mapping_and_dynamic_help() {
    let output = case(
        &[(
            "keymaps.jsonc",
            r#"{"select":{"open_help":["X","!"],"next_row":["A","J"]}}"#,
        )],
        b"?Xq",
    );
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("X/!") && text.contains("A/J") && text.contains("next pane"),
        "{text}"
    );
    assert!(
        !text.contains("j/↓         next pane"),
        "replaced select default must disappear (help context retains its defaults): {text}"
    );
}

#[test]
fn invalid_keymap_is_error_page_not_defaults() {
    let output = case(
        &[(
            "keymaps.jsonc",
            r#"{"select":{"next_row":["k","q"],"open_help":[],"wat":["F1"]},"search":{}}"#,
        )],
        b"\r",
    );
    assert!(output.status.success());
    let text = diagnostics_without_source_rows(&output);
    assert!(
        text.contains("Keymap")
            && text.contains("select")
            && text.contains("previous_row")
            && text.contains("close")
            && text.contains("wat")
            && text.contains("search"),
        "{text}"
    );
    assert!(!text.contains("Agent/Pane Name"), "{text}");
}

#[test]
fn duplicate_fields_and_ctrl_bindings_are_rejected() {
    for (config, heading) in [
        (
            r#"{"select":{"next_row":["J"],"next_row":["K"]}}"#,
            "Duplicate keymap fields",
        ),
        (
            r#"{"select":{"next_row":["Ctrl+i","Ctrl+m"]}}"#,
            "Unsupported keymap keys",
        ),
    ] {
        let output = case(&[("keymaps.jsonc", config)], b"q");
        let text = diagnostics_without_source_rows(&output);
        assert!(
            text.contains(&format!("ERROR: {heading} - select mode"))
                && !text.contains("Agent/Pane Name"),
            "{text}"
        );
    }
}

#[test]
fn file_precedence_rejects_broken_jsonc_without_legacy_fallback() {
    for (broken, heading) in [
        ("{", "Keymap syntax"),
        (
            r#"{"select":{"next_row":[]}}"#,
            "Keymap structure - select mode",
        ),
        (
            r#"{"select":{"next_row":["Ctrl+Alt+j"]}}"#,
            "Unsupported keymap keys - select mode",
        ),
    ] {
        let output = case(&[("keymaps.jsonc", broken), ("keymaps.json", "{}")], b"q");
        let text = diagnostics_without_source_rows(&output);
        assert!(
            !text.contains("keymaps.jsonc")
                && text.starts_with(&format!("ERROR: {heading}\n"))
                && !text.contains("Agent/Pane Name"),
            "{text}"
        );
    }
    let output = case(
        &[("keymaps.json", r#"{"select":{"open_help":["X"]}}"#)],
        b"Xq",
    );
    assert!(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.split_whitespace().collect::<Vec<_>>() == ["X", "show", "keys"]));
    let output = case(
        &[("status-colors.jsonc", "{"), ("status-colors.json", "{}")],
        b"q",
    );
    let text = diagnostics_without_source_rows(&output);
    assert!(
        output.status.success()
            && text.starts_with("ERROR:")
            && text.contains("Status colors syntax"),
        "{text}"
    );
    assert!(
        !text.contains("status-colors.json") && !text.contains("Agent/Pane Name"),
        "{text}"
    );
    let output = case(
        &[
            ("theme.jsonc", "{"),
            ("theme.json", r#"{"theme":"dracula"}"#),
        ],
        b"q",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success()
            && text.contains("Theme configuration:")
            && text.contains("using Mocha")
            && !text.contains("theme.json")
            && text.contains("ERROR"),
        "{text}"
    );
}

#[test]
fn modifier_configuration_error_precedes_topology_failure() {
    let output = case(
        &[
            ("keymaps.jsonc", r#"{"select":{"next_row":["Ctrl+m"]}}"#),
            ("herdr", "#!/bin/sh\nexit 9\n"),
        ],
        b"q",
    );
    let text = diagnostics_without_source_rows(&output);
    assert!(
        output.status.success()
            && text.contains("Unsupported keymap keys - select mode")
            && text.contains("Ctrl/Alt bindings are not allowed"),
        "stdout={text}, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.contains("keymaps.jsonc")
            && text.starts_with("ERROR: Unsupported keymap keys - select mode\n"),
        "configuration diagnostics must be pathless: {text}"
    );
}

#[test]
fn common_page_collects_all_three_files_and_recoverable_fields() {
    let output = case(
        &[
            (
                "keymaps.jsonc",
                r#"{"select":{"bad":null,"next_row":["F1","q"]},"typo":7}"#,
            ),
            (
                "theme.jsonc",
                r#"{"theme":"bogus","theme":false,"extra":1}"#,
            ),
            (
                "status-colors.jsonc",
                r#"{"working":0,"working":"bad","idle":false,"typo":0}"#,
            ),
        ],
        b"yXjkk\r",
    );
    let text = diagnostics_without_source_rows(&output);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    for diagnostic in [
        "ERROR",
        "Keymap",
        "Theme",
        "Status colors",
        "ERROR: Unknown keymap actions - select mode\nbad",
        "bad: expected nonempty",
        "unknown key",
        "unknown context typo",
        "typo: expected object",
        "duplicate field theme",
        "unknown theme bogus",
        "theme must be a string",
        "unknown field extra",
        "duplicate field working",
        "working must be #RRGGBB",
        "idle must be #RRGGBB",
        "unknown status key typo",
    ] {
        assert!(text.contains(diagnostic), "missing {diagnostic}: {text}");
    }
    assert!(text.matches('─').count() >= 3, "{text}");
    assert!(!text.contains("Agent/Pane Name"), "{text}");
    for path in [
        "/tmp/opencode",
        "keymaps.json",
        "theme.json",
        "status-colors.json",
    ] {
        assert!(!text.contains(path), "{text}");
    }
}

#[test]
fn file_error_blocks_print_one_selected_path_row_before_diagnostics() {
    for extension in ["jsonc", "json"] {
        let keymap = format!("keymaps.{extension}");
        let status = format!("status-colors.{extension}");
        let theme = format!("theme.{extension}");
        let output = case(
            &[
                (&keymap, r#"{"select":{"broken":null}}"#),
                (&status, "{"),
                (&theme, "{"),
            ],
            b"q",
        );
        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<_> = text.lines().collect();
        for (heading, filename) in [
            ("Unknown keymap actions - select mode", &keymap),
            ("Keymap structure - select mode", &keymap),
            ("Status colors syntax", &status),
            ("Theme syntax", &theme),
        ] {
            let at = lines
                .iter()
                .position(|line| *line == format!("ERROR: {heading}"))
                .unwrap();
            assert!(
                lines[at + 1].starts_with(&format!("{filename}  /tmp/opencode/"))
                    && lines[at + 1].ends_with(filename),
                "missing selected source under {heading}: {text}"
            );
            let end = (at + 1..lines.len())
                .find(|&at| lines[at].starts_with('─'))
                .unwrap();
            assert_eq!(
                lines[at + 1..end]
                    .iter()
                    .filter(|line| line.contains(filename))
                    .count(),
                1,
                "source must occur once per block: {text}"
            );
        }
        assert!(
            !text.contains("Popup too small"),
            "piped input has no interactive size block: {text}"
        );
    }
}

#[test]
fn collision_groups_merge_only_exact_action_sets_by_mode() {
    let output = case(
        &[(
            "keymaps.jsonc",
            r#"{
        "select":{"next_row":["A","B"],"previous_row":["A","B"],
                  "next_tab":["C","D"],"previous_tab":["C","X"],"close":["D","X"]},
        "help":{"next_category":["A","B"],"previous_category":["A","B"]}
    }"#,
        )],
        b"q",
    );
    let text = diagnostics_without_source_rows(&output);
    for line in [
        "next_row, previous_row: A/B",
        "previous_tab, next_tab: C",
        "next_tab, close: D",
        "previous_tab, close: X",
        "next_category, previous_category: A/B",
    ] {
        assert!(text.contains(line), "missing {line}: {text}");
    }
    assert_eq!(text.matches("next_row, previous_row:").count(), 1, "{text}");
    assert!(
        text.contains("ERROR: Keymap collisions - select mode")
            && text.contains("ERROR: Keymap collisions - help mode"),
        "{text}"
    );
    assert!(!text.contains(" | "), "no collision table: {text}");
}

#[test]
fn syntax_errors_do_not_invent_fields_and_theme_alone_keeps_normal_ui() {
    let output = case(
        &[
            ("keymaps.jsonc", "{"),
            ("theme.jsonc", "{"),
            ("status-colors.jsonc", "{"),
        ],
        b"q",
    );
    let text = diagnostics_without_source_rows(&output);
    assert!(output.status.success(), "{text}");
    for label in ["Keymap syntax", "Theme syntax", "Status colors syntax"] {
        assert!(text.contains(label), "{text}");
    }
    assert!(
        !text.contains("missing theme") && !text.contains("must be #RRGGBB"),
        "{text}"
    );
    let output = case(&[("theme.jsonc", r#"{"theme":"bogus"}"#)], b"q");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success()
            && text.contains("Agent/Pane Name")
            && text.contains("ERROR")
            && text.contains("using Mocha"),
        "{text}"
    );
    assert!(
        !text.contains("theme.json") && !text.contains("/tmp/opencode"),
        "{text}"
    );
}
