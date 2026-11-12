use super::test_support::*;
use crate::input::Key;
use crate::selection::Ui;
use crate::topology::{annotate_worktrees, rows};
use serde_json::Value;

#[test]
fn snapshot_repository_uses_only_unique_explicit_workspace_worktree_identity() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    for (worktree, expected) in [
        (
            serde_json::json!({"repo_key":"key", "repo_name":"Repo\tname"}),
            Some(("key", "Repo name")),
        ),
        (
            serde_json::json!({"repo_name":"Repo", "repo_root":"/private/repo"}),
            None,
        ),
        (
            serde_json::json!({"repo_key":"key", "repo_name":" \n"}),
            None,
        ),
        (Value::Null, None),
    ] {
        snapshot["result"]["snapshot"]["workspaces"][1]["worktree"] = worktree;
        let items = rows(&snapshot.to_string(), None, 160).unwrap();
        let metadata = &items.iter().find(|r| r.id == "p-b").unwrap().agent_metadata;
        assert_eq!(
            metadata
                .repository
                .as_ref()
                .map(|(key, name)| (key.as_str(), name.as_str())),
            expected
        );
        assert!(metadata.branch.is_none());
    }
    snapshot["result"]["snapshot"]["workspaces"][1]["worktree"] =
        serde_json::json!({"repo_key":"key", "repo_name":"Repo"});
    let duplicate = snapshot["result"]["snapshot"]["workspaces"][1].clone();
    snapshot["result"]["snapshot"]["workspaces"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(rows(&snapshot.to_string(), None, 160)
        .unwrap()
        .iter()
        .find(|r| r.id == "p-b")
        .unwrap()
        .agent_metadata
        .repository
        .is_none());
}

#[cfg(unix)]
#[test]
fn worktree_metadata_is_optional_unique_and_does_not_change_select_or_queries() {
    use std::os::unix::fs::PermissionsExt;
    let fake = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-metadata-{}", std::process::id()));
    let log = fake.with_extension("calls");
    std::fs::write(&log, "").unwrap();
    let entry = serde_json::json!({"open_workspace_id":"w1", "branch":"metadata\nbranch", "is_linked_worktree":false,"is_detached":false,"is_prunable":false});
    let source = serde_json::json!({"repo_key":"key", "repo_name":"Metadata\tRepo", "repo_root":"/private/never-infer"});
    for (entries, source, repository, branch) in [
        (
            vec![entry.clone()],
            source.clone(),
            Some(("key", "Metadata Repo")),
            Some("metadata branch"),
        ),
        (
            vec![entry.clone(), entry.clone()],
            source.clone(),
            None,
            None,
        ),
        (
            vec![serde_json::json!({"open_workspace_id":"w1", "branch":"main","is_detached":true})],
            source.clone(),
            Some(("key", "Metadata Repo")),
            None,
        ),
        (
            vec![serde_json::json!({"open_workspace_id":"w1", "branch":"main","is_prunable":true})],
            source.clone(),
            None,
            None,
        ),
        (
            vec![serde_json::json!({"open_workspace_id":"absent", "branch":"main"})],
            source.clone(),
            None,
            None,
        ),
        (
            vec![entry.clone()],
            serde_json::json!({"repo_root":"/private/never-infer"}),
            None,
            Some("metadata branch"),
        ),
        (
            vec![serde_json::json!({"open_workspace_id":"w1", "branch":" \t"})],
            source.clone(),
            Some(("key", "Metadata Repo")),
            None,
        ),
        (vec![], source, None, None),
    ] {
        let response = serde_json::json!({"result":{"type":"worktree_list", "source":source, "worktrees":entries}});
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nprintf '%s\\n' '{}'\n",
                log.display(),
                response
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut items = rows(SNAPSHOT, Some("p-self"), 160).unwrap();
        annotate_worktrees(&mut items, fake.as_os_str(), false);
        let metadata = &items.iter().find(|r| r.id == "p-b").unwrap().agent_metadata;
        assert_eq!(
            metadata
                .repository
                .as_ref()
                .map(|(key, name)| (key.as_str(), name.as_str())),
            repository
        );
        assert_eq!(metadata.branch.as_deref(), branch);
        let mut state = Ui::with_theme_choice(items, "catppuccin", None);
        let original = render(SNAPSHOT, Some("p-self"), 160).unwrap();
        assert_eq!(
            state.terminal_frame(160, 20),
            Ui::with_theme_choice(
                rows(SNAPSHOT, Some("p-self"), 160).unwrap(),
                "catppuccin",
                None
            )
            .terminal_frame(160, 20)
        );
        assert!(!original.contains("Metadata Repo"));
        for query in ["Metadata Repo", "metadata branch", "never-infer"] {
            state.query = query.into();
            assert!(state.visible().is_empty());
        }
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 16);
    std::fs::remove_file(fake).unwrap();
    std::fs::remove_file(log).unwrap();
}

#[test]
fn optional_agent_title_requires_explicit_consistent_uniquely_associated_titles() {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["result"]["snapshot"]["agents"][0]["title"] = Value::from("Metadata-only task");
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-self"), 160).unwrap(),
        "catppuccin",
        None,
    );
    assert_eq!(
        state.terminal_frame(160, 20),
        Ui::with_theme_choice(
            rows(SNAPSHOT, Some("p-self"), 160).unwrap(),
            "catppuccin",
            None
        )
        .terminal_frame(160, 20)
    );
    state.query = "Metadata-only task".into();
    assert!(state.visible().is_empty());
    for (agent_title, pane_title, expected) in [
        (Value::Null, Value::Null, None),
        (Value::from("Task\nname"), Value::Null, Some("Task name")),
        (Value::Null, Value::from("Task"), Some("Task")),
        (Value::from("Task"), Value::from("Task"), Some("Task")),
        (Value::from("Task"), Value::from("Other"), None),
        (Value::from(" \n"), Value::from(""), None),
        (Value::from(42), Value::Null, None),
    ] {
        snapshot["result"]["snapshot"]["agents"][0]["title"] = agent_title;
        snapshot["result"]["snapshot"]["panes"][2]["title"] = pane_title;
        let items = rows(&snapshot.to_string(), Some("p-self"), 160).unwrap();
        let row = items.iter().find(|r| r.id == "p-b").unwrap();
        assert_eq!(row.agent_metadata.title.as_deref(), expected);
        assert!(items
            .iter()
            .find(|r| r.id == "p-untitled")
            .unwrap()
            .agent_metadata
            .title
            .is_none());
    }
    snapshot["result"]["snapshot"]["agents"][0]["title"] = Value::from("Task");
    snapshot["result"]["snapshot"]["agents"][0]["workspace_id"] = Value::from("wrong");
    assert!(rows(&snapshot.to_string(), None, 160)
        .unwrap()
        .iter()
        .find(|r| r.id == "p-b")
        .unwrap()
        .agent_metadata
        .title
        .is_none());
    snapshot["result"]["snapshot"]["agents"][0]["workspace_id"] = Value::from("w1");
    let duplicate = snapshot["result"]["snapshot"]["agents"][0].clone();
    snapshot["result"]["snapshot"]["agents"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(rows(&snapshot.to_string(), None, 160)
        .unwrap()
        .iter()
        .find(|r| r.id == "p-b")
        .unwrap()
        .agent_metadata
        .title
        .is_none());
}

#[cfg(unix)]
#[test]
fn contradictory_repository_identity_and_branch_are_not_first_match_wins() {
    use std::os::unix::fs::PermissionsExt;
    let fake = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-metadata-conflict-{}",
        std::process::id()
    ));
    for (other_key, other_branch, expected_repository, expected_branch) in [
        ("key", "main", Some(("key", "Repo")), Some("main")),
        ("other-key", "main", None, Some("main")),
        ("key", "other", Some(("key", "Repo")), None),
    ] {
        let response = |key: &str, branch: &str| serde_json::json!({"result":{"type":"worktree_list","source":{"repo_key":key,"repo_name":"Repo"},"worktrees":[{"open_workspace_id":"w1","branch":branch}]}});
        std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$4\" = w1 ]; then printf '%s\\n' '{}'; else printf '%s\\n' '{}'; fi\n", response("key", "main"), response(other_key, other_branch))).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][1]["worktree"] =
            serde_json::json!({"repo_key":"key","repo_name":"Repo"});
        let mut items = rows(&snapshot.to_string(), None, 160).unwrap();
        annotate_worktrees(&mut items, fake.as_os_str(), false);
        let metadata = &items.iter().find(|r| r.id == "p-b").unwrap().agent_metadata;
        assert_eq!(
            metadata
                .repository
                .as_ref()
                .map(|(key, name)| (key.as_str(), name.as_str())),
            expected_repository
        );
        assert_eq!(metadata.branch.as_deref(), expected_branch);
    }
    std::fs::remove_file(fake).unwrap();
}

#[cfg(unix)]
#[test]
fn incomplete_repository_candidates_still_detect_key_conflicts_in_both_directions() {
    use std::os::unix::fs::PermissionsExt;
    let fake = std::path::Path::new("/tmp/opencode").join(format!(
        "crossroads-incomplete-repository-{}",
        std::process::id()
    ));
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for (snapshot_key, source_key, expected_key) in [
        ("A", "B", None),
        ("A", "A", Some("A")),
        ("A", "", Some("A")),
        ("A", " \t", Some("A")),
    ] {
        for incomplete_name in [Value::from(""), Value::from(" \n"), Value::Null] {
            for incomplete_snapshot in [false, true] {
                let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
                let mut worktree = serde_json::json!({"repo_key":snapshot_key,"repo_name":"Repo"});
                let mut source = serde_json::json!({"repo_key":source_key,"repo_name":"Repo"});
                if incomplete_snapshot {
                    worktree["repo_name"] = incomplete_name.clone();
                } else {
                    source["repo_name"] = incomplete_name.clone();
                }
                snapshot["result"]["snapshot"]["workspaces"][1]["worktree"] = worktree;
                let response = serde_json::json!({"result":{"type":"worktree_list","source":source,"worktrees":[{"open_workspace_id":"w1","branch":"main"}]}});
                std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response))
                    .unwrap();
                std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
                let mut items = rows(&snapshot.to_string(), None, 160).unwrap();
                annotate_worktrees(&mut items, fake.as_os_str(), false);
                let metadata = &mut items
                    .iter_mut()
                    .find(|r| r.id == "p-b")
                    .unwrap()
                    .agent_metadata;
                actual.push(metadata.repository.take());
                expected.push(expected_key.and_then(|key| {
                    (!incomplete_snapshot || source_key == key)
                        .then(|| (key.to_owned(), "Repo".to_owned()))
                }));
                assert_eq!(metadata.branch.as_deref(), Some("main"));
            }
        }
    }
    std::fs::remove_file(fake).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn agent_membership_uses_existing_detection_and_excludes_own_pane() {
    use crate::selection::TableView;
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    let data = &mut snapshot["result"]["snapshot"];
    data["panes"][3]["display_agent"] = Value::from("Pane fallback");
    data["agents"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"pane_id":"p-untitled","name":"Named agent"}));
    let mut state = Ui::with_theme_choice(
        rows(&snapshot.to_string(), Some("p-b"), 160).unwrap(),
        "catppuccin",
        None,
    );
    state.set_table_view(TableView::Agent);
    assert_eq!(
        state
            .visible()
            .iter()
            .map(|&i| state.rows[i].id.as_str())
            .collect::<Vec<_>>(),
        ["p-a", "p-untitled", "p-other"]
    );
    state.query = "PNFLLBCK".into();
    assert_eq!(state.visible().len(), 1);
    assert_eq!(state.rows[state.visible()[0]].id, "p-a");
}

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
    assert_eq!(lines[3], format!("  {}", "─".repeat(96)), "{output}");
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
        13 + super::test_support::ui()
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
    let lines: Vec<_> = empty_output.lines().collect();
    assert_eq!(lines[3], format!("  {}", "─".repeat(36)), "{empty_output}");
    assert_eq!(lines[4], "            No matching panes", "{empty_output}");
    assert_eq!(
        lines[5], "       r: reset search · a: agent",
        "{empty_output}"
    );
    assert_eq!(lines[6], lines[3], "{empty_output}");
    let footer = empty_output
        .lines()
        .skip(7)
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("/ ", "/");
    assert_eq!(footer.replace(" · ", " "), "j/↓: ↓pane k/↑: ↑pane h/←: ↑tab l/→: ↓tab Tab: ↓ws Shift+Tab: ↑ws Enter: focus Esc: exit ?: keys a: agent", "{empty_output}");
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

#[cfg(unix)]
#[test]
fn linked_workspace_filter_preserves_annotation_order_and_remembered_selection() {
    use std::os::unix::fs::PermissionsExt;
    let fake = std::path::Path::new("/tmp/opencode")
        .join(format!("crossroads-linked-filter-{}", std::process::id()));
    let response = serde_json::json!({"result":{"type":"worktree_list","source":{"source_workspace_id":"w1"},"worktrees":[{"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"}]}});
    std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response)).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut items = rows(SNAPSHOT, Some("p-self"), 160).unwrap();
    annotate_worktrees(&mut items, fake.as_os_str(), false);
    std::fs::remove_file(fake).unwrap();
    let mut state = Ui::with_theme_choice(items, "catppuccin", None);
    let whole_order = [
        "p-b",
        "p-a",
        "p-untitled",
        "p-untitled-2",
        "p-second-tab",
        "p-other",
        "p-other-untitled",
    ];
    let linked_order = ["p-other", "p-other-untitled"];
    for (query, expected) in [
        ("", whole_order.as_slice()),
        ("↳ Same", linked_order.as_slice()),
        ("feature/demo", linked_order.as_slice()),
        ("FTR/DM", linked_order.as_slice()),
        ("no-such-workspace", [].as_slice()),
    ] {
        state.query = query.into();
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|&i| state.rows[i].id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "query {query:?}"
        );
    }

    state.selected = Some("p-a".into());
    state.query = "feature/demo".into();
    assert_eq!(
        state.selected_index(&state.visible()),
        state.visible().first().copied()
    );
    let frame = state.layout(160, 20, true).join("\n");
    let plain = state.layout(160, 20, false).join("\n");
    let linked_line = frame
        .lines()
        .find(|line| selected_prefix(line))
        .unwrap_or_else(|| panic!("{frame:?}"));
    assert!(plain.contains("↳ Same (feature/demo)"), "{frame}");
    assert!(linked_line.contains("codex"), "{frame}");
    assert_eq!(plain.matches("↳ Same (feature/demo)").count(), 1, "{frame}");
    assert!(
        without_name_accent(linked_line).contains("↳ Same (\x1b[4mfeature/demo"),
        "{frame}"
    );
    assert!(
        !plain.contains("Claude") && !plain.contains("Second"),
        "{frame}"
    );
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    state.key(Key::Char('r'));
    assert!(state.query.is_empty());
    assert_eq!(state.selected.as_deref(), Some("p-a"));
    assert_eq!(
        state.rows[state.selected_index(&state.visible()).unwrap()].id,
        "p-a"
    );

    state.query = "feature/demo".into();
    state.key(Key::Down);
    assert_eq!(state.selected.as_deref(), Some("p-other-untitled"));
    state.key(Key::Char('r'));
    assert_eq!(state.selected.as_deref(), Some("p-other-untitled"));

    state.query = "no-such-workspace".into();
    assert!(state.selected_index(&state.visible()).is_none());
    let empty = state.layout(160, 20, true).join("\n");
    assert!(empty.contains("No matching panes"), "{empty}");
    assert!(!empty.lines().any(selected_prefix), "{empty}");
    assert_eq!(state.key(Key::Enter), None);
    assert!(!state.closed);
    state.key(Key::Char('r'));
    state.query = "feature/demo".into();
    state.selected = Some("p-a".into());
    assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
}
