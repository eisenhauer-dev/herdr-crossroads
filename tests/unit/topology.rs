use super::test_support::*;
use crate::input::Key;
use crate::selection::Ui;
use crate::topology::{annotate_worktrees, rows};
use serde_json::Value;

#[cfg(unix)]
#[test]
fn linked_workspace_follows_source_without_reordering_other_blocks_or_panes() {
    use std::os::unix::fs::PermissionsExt;
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let data = &mut snapshot["result"]["snapshot"];
    data["workspaces"][0]["number"] = Value::from(3);
    data["workspaces"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"workspace_id":"w3","number":2,"label":"Third"}));
    data["tabs"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"tab_id":"t4","workspace_id":"w3","number":1,"label":"Other"}));
    data["panes"].as_array_mut().unwrap().push(serde_json::json!({"pane_id":"p-third","workspace_id":"w3","tab_id":"t4","label":"third pane"}));
    let fake = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-order-{}", std::process::id()));
    let response = serde_json::json!({"result":{"type":"worktree_list","source":{"source_workspace_id":"w1"},"worktrees":[{"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"}]}});
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response)).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    for (source, expected) in [
        (
            "w1",
            [
                "p-b",
                "p-a",
                "p-second-tab",
                "p-other",
                "p-other-untitled",
                "p-third",
            ],
        ),
        (
            "absent",
            [
                "p-b",
                "p-a",
                "p-second-tab",
                "p-third",
                "p-other",
                "p-other-untitled",
            ],
        ),
    ] {
        let mut response = response.clone();
        response["result"]["source"]["source_workspace_id"] = Value::from(source);
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response)).unwrap();
        let mut items = rows(&snapshot.to_string(), Some("p-self"), 120).unwrap();
        annotate_worktrees(&mut items, fake.as_os_str(), false);
        assert_eq!(
            items
                .iter()
                .find(|r| r.workspace_id == "w2")
                .unwrap()
                .workspace,
            "↳ Same (feature/demo)"
        );
        assert_eq!(
            items
                .iter()
                .filter(|r| expected.contains(&r.id.as_str()))
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "source {source}"
        );
        let mut state = Ui::with_theme_choice(items, "catppuccin", None);
        let frame = state.terminal_frame(120, 20);
        assert!(frame.contains("↳ Same (feature/demo)"), "{frame}");
        assert!(!frame.contains("w1") && !frame.contains("w2"), "{frame}");
        state.selected = Some("p-second-tab".into());
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some(expected[3]));
        state.query = "Third".into();
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-third"));
    }
    std::fs::remove_file(fake).unwrap();
}

#[test]
fn sorts_context_but_preserves_pane_order_without_visible_ids() {
    let output = render(SNAPSHOT, Some("p-self"), 100).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert!(output.contains("Same") && output.contains("Second"));
    assert!(lines
        .iter()
        .any(|line| line.contains("Claude") && line.contains("working")));
    for (first, second) in [
        ("Claude", "pane 3"),
        ("pane 3", "pane 4"),
        ("pane 4", "Second"),
        ("Second", "codex"),
    ] {
        assert!(output.find(first) < output.find(second), "{output}");
    }
    assert!(lines
        .iter()
        .any(|line| line.contains("codex") && line.contains("idle")));
    assert_eq!(
        lines.len(),
        12 + super::test_support::ui()
            .hints(crate::keymap::Context::Select, 100, 2, false)
            .len()
    );
    assert!(!output.contains("p-self") && !output.contains("p-b"));
}

#[test]
fn unnamed_pane_does_not_display_terminal_title_path() {
    let output = render(SNAPSHOT, Some("p-self"), 160).unwrap();
    assert!(!output.contains("PROJECT_SECRET_PATH_MARKER"), "{output}");
    assert!(!output.contains("/private/example"), "{output}");
    assert!(
        output.lines().any(|line| line.contains("pane 3")),
        "{output}"
    );
    assert!(output.lines().any(|line| line.contains("Same")), "{output}");
}

#[test]
fn explicit_title_then_label_take_precedence_without_using_terminal_title() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let pane = &mut snapshot["result"]["snapshot"]["panes"][4];
    assert_eq!(pane["pane_id"], "p-untitled");
    pane["title"] = Value::from("opencode");
    let titled = render(&snapshot.to_string(), Some("p-self"), 160).unwrap();
    assert!(
        titled.lines().any(|line| line.contains("opencode")),
        "{titled}"
    );
    assert!(!titled.contains("PROJECT_SECRET_PATH_MARKER"));

    snapshot["result"]["snapshot"]["panes"][4]["label"] = Value::from("Named pane");
    let labeled = render(&snapshot.to_string(), Some("p-self"), 160).unwrap();
    assert!(
        labeled.lines().any(|line| line.contains("Named pane")),
        "{labeled}"
    );
    assert!(!labeled.contains("PROJECT_SECRET_PATH_MARKER"));
    assert!(!labeled.contains("opencode"));
}

#[test]
fn unnamed_pane_numbers_count_named_visible_rows_and_reset_per_tab() {
    let output = render(SNAPSHOT, Some("p-self"), 160).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Claude") && line.contains("working")),
        "{output}"
    );
    for name in ["pane 3", "pane 4", "pane 1", "pane 2"] {
        assert!(lines.iter().any(|line| line.contains(name)), "{output}");
    }
    assert!(!output.contains("PROJECT_SECRET_PATH_MARKER"));
    assert!(!output.contains("p-self"));

    let without_named_pane = render(SNAPSHOT, Some("p-a"), 160).unwrap();
    assert!(!without_named_pane.contains("p-a"));
    assert!(without_named_pane
        .lines()
        .any(|line| line.contains("pane 2")));
    assert!(without_named_pane
        .lines()
        .any(|line| line.contains("pane 3")));
}

#[test]
fn narrow_width_clips_names_without_rendering_ids_or_unused_status() {
    let output = render(SNAPSHOT, Some("p-self"), 40).unwrap();
    for line in output.lines().skip(1).take(3) {
        assert!(line.chars().count() <= 40, "{line}");
    }
    assert!(output.lines().any(|line| line.contains("Claude")));
    assert!(output.lines().any(|line| line.contains("codex")));
    let long = SNAPSHOT.replace(
        "\"Claude\"",
        "\"An exceedingly verbose agent display name\"",
    );
    assert!(render(&long, Some("p-self"), 40).unwrap().contains('…'));
    assert!(!output.contains("p-b") && !output.lines().nth(2).unwrap().contains("Status"));
}

#[test]
fn narrow_width_also_clips_long_agent_names() {
    let long_agent = SNAPSHOT.replace(
        "\"Claude\"",
        "\"An exceedingly verbose agent display name\"",
    );
    let output = render(&long_agent, Some("p-self"), 40).unwrap();
    assert!(
        output
            .lines()
            .skip(1)
            .take(3)
            .all(|line| line.chars().count() <= 40),
        "{output}"
    );
    assert!(output.contains("An"));
    assert!(output
        .lines()
        .any(|line| line.contains("An") && line.contains('…')));
}

#[test]
fn absent_own_pane_id_and_empty_topology() {
    assert!(render(SNAPSHOT, None, 100).unwrap().contains("popup"));
    let empty = r#"{"id":"test","result":{"type":"session_snapshot","snapshot":{"workspaces":[],"tabs":[],"panes":[],"agents":[],"layouts":[]}}}"#;
    let empty_output = render(empty, None, 40).unwrap();
    assert!(
        empty_output.contains("No matching panes") && empty_output.contains("Agent/Pane"),
        "{empty_output}"
    );
    let footer = empty_output
        .lines()
        .skip(5)
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("/ ", "/");
    assert_eq!(footer.replace(" · ", " "), "j/↓: ↓pane k/↑: ↑pane h/←: ↑tab l/→: ↓tab Tab: ↓ws Shift+Tab: ↑ws Enter: focus Esc: exit ?: keys", "{empty_output}");
}

#[test]
fn rejects_invalid_response_instead_of_displaying_partial_topology() {
    assert!(render(&SNAPSHOT.replace("session_snapshot", "snapshot"), None, 80).is_err());
    for bad in [
        "not json",
        r#"{"result":{"type":"error"}}"#,
        r#"{"result":{"type":"session_snapshot","snapshot":{"workspaces":[],"tabs":[],"panes":[]}}}"#,
    ] {
        assert!(render(bad, None, 80).is_err(), "{bad}");
    }
}

#[test]
fn sanitizes_terminal_control_characters_in_labels() {
    let unsafe_name = SNAPSHOT.replace("\"Same\"", "\"\\u001b[31mSame\"");
    let output = render(&unsafe_name, Some("p-self"), 100).unwrap();
    assert!(!output.contains('\u{1b}'));
}
