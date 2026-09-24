use serde_json::Value;
use std::io;
use std::process::{Command, Stdio};

fn field<'a>(item: &'a Value, name: &str) -> io::Result<&'a str> {
    item.get(name).and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} in snapshot"),
        )
    })
}

fn entries<'a>(snapshot: &'a Value, name: &str) -> io::Result<&'a [Value]> {
    snapshot
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} in snapshot"),
            )
        })
}

fn text<'a>(item: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| item.get(*name).and_then(Value::as_str))
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

// Without a terminal-width crate, count non-ASCII characters conservatively as two columns.
fn columns(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

fn clipped(s: &str, limit: usize) -> String {
    if columns(s) <= limit {
        return s.to_owned();
    }
    if limit < 2 {
        return ".".repeat(limit);
    }
    let mut result = String::new();
    for c in s.chars() {
        if columns(&result) + if c.is_ascii() { 1 } else { 2 } > limit - 2 {
            break;
        }
        result.push(c);
    }
    result.push('…');
    result
}

fn render(response: &str, own_pane: Option<&str>, width: usize) -> io::Result<String> {
    let data: Value = serde_json::from_str(response).map_err(io::Error::other)?;
    if field(&data["result"], "type")? != "session_snapshot" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a snapshot response",
        ));
    }
    let snapshot = &data["result"]["snapshot"];
    let workspaces = entries(snapshot, "workspaces")?;
    let tabs = entries(snapshot, "tabs")?;
    let panes = entries(snapshot, "panes")?;
    let agents = entries(snapshot, "agents")?;

    let mut rows = Vec::new();
    for (index, pane) in panes.iter().enumerate() {
        let id = field(pane, "pane_id")?;
        if Some(id) == own_pane {
            continue;
        }
        let workspace_id = field(pane, "workspace_id")?;
        let tab_id = field(pane, "tab_id")?;
        let workspace = workspaces
            .iter()
            .find(|w| w["workspace_id"] == workspace_id)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "pane workspace not found")
            })?;
        let tab = tabs
            .iter()
            .find(|t| t["tab_id"] == tab_id && t["workspace_id"] == workspace_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pane tab not found"))?;
        let number = |value: &Value| -> io::Result<u64> {
            value.as_u64().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing workspace/tab number")
            })
        };
        let wn = number(&workspace["number"])?;
        let tn = number(&tab["number"])?;
        let agent = agents.iter().find(|a| a["pane_id"] == id);
        let agent_name = agent
            .and_then(|a| text(a, &["display_agent", "name", "agent"]))
            .or_else(|| text(pane, &["display_agent", "agent"]))
            .unwrap_or("-");
        let status = agent
            .and_then(|a| text(a, &["agent_status"]))
            .or_else(|| text(pane, &["agent_status"]))
            .unwrap_or("-");
        let base = format!("W{wn} T{tn} {}", clean(id));
        let status = clipped(&clean(status), width.saturating_sub(columns(&base) + 6 + 1));
        let agent_name = clipped(
            &clean(agent_name),
            width.saturating_sub(columns(&base) + 6 + columns(&status)),
        );
        let prefix = format!("{base} | {agent_name} | {status}");
        let pane_name = text(pane, &["label", "title"])
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let position = panes[..=index]
                    .iter()
                    .filter(|candidate| {
                        candidate["workspace_id"] == workspace_id
                            && candidate["tab_id"] == tab_id
                            && candidate["pane_id"].as_str() != own_pane
                    })
                    .count();
                format!("pane {position}")
            });
        let names = format!(
            "{} / {} / {}",
            clean(field(workspace, "label")?),
            clean(field(tab, "label")?),
            clean(&pane_name)
        );
        let remaining = width.saturating_sub(columns(&prefix) + 3);
        let row = if remaining > 0 {
            format!("{prefix} | {}", clipped(&names, remaining))
        } else {
            prefix
        };
        rows.push(((wn, tn, index), row));
    }
    rows.sort_by_key(|(order, _)| *order);
    let mut output = String::from("Herdr Crossroads\n");
    for (_, row) in rows {
        output.push_str(&row);
        output.push('\n');
    }
    output.push_str("Press Enter to close.\n");
    Ok(output)
}

fn terminal_width() -> usize {
    let measured = Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|n| n.parse::<usize>().ok())
        });
    measured
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok()))
        .filter(|n| *n > 0)
        .unwrap_or(80)
}

fn main() -> io::Result<()> {
    let executable = std::env::var_os("HERDR_BIN_PATH")
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "herdr".into());
    let output = Command::new(executable)
        .args(["api", "snapshot"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "herdr api snapshot failed: {}",
            output.status
        )));
    }
    let snapshot = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    print!(
        "{}",
        render(
            &snapshot,
            std::env::var("HERDR_PANE_ID").ok().as_deref(),
            terminal_width()
        )?
    );
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = include_str!("../tests/fixtures/topology.json");

    #[test]
    fn sorts_context_but_preserves_pane_order_and_full_ids() {
        let output = render(SNAPSHOT, Some("p-self"), 100).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert!(lines[1].contains("W1 T1 p-b | Claude | working | Same / Same / Same"));
        assert!(lines[2].contains("W1 T1 p-a | - | idle | Same / Same / Same"));
        assert!(lines[3].contains("W1 T1 p-untitled"));
        assert!(lines[4].contains("W1 T1 p-untitled-2"));
        assert!(lines[5].contains("W1 T2 p-second-tab"));
        assert!(lines[6].contains("W2 T1 p-other | codex | idle | Same / Same / An extremely long pane name with a suffix"));
        assert!(lines[7].contains("W2 T1 p-other-untitled"));
        assert_eq!(lines.len(), 9);
        assert!(!output.contains("p-self"));
    }

    #[test]
    fn unnamed_pane_does_not_display_terminal_title_path() {
        let output = render(SNAPSHOT, Some("p-self"), 160).unwrap();
        assert!(!output.contains("PROJECT_SECRET_PATH_MARKER"), "{output}");
        assert!(!output.contains("/private/example"), "{output}");
        assert!(
            output.contains("W1 T1 p-untitled | - | idle | Same / Same / pane 3"),
            "{output}"
        );
        assert!(
            output.contains("W1 T1 p-a | - | idle | Same / Same / Same"),
            "{output}"
        );
    }

    #[test]
    fn explicit_title_then_label_take_precedence_without_using_terminal_title() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        let pane = &mut snapshot["result"]["snapshot"]["panes"][4];
        assert_eq!(pane["pane_id"], "p-untitled");
        pane["title"] = Value::from("opencode");
        let titled = render(&snapshot.to_string(), Some("p-self"), 160).unwrap();
        assert!(
            titled.contains("p-untitled | - | idle | Same / Same / opencode"),
            "{titled}"
        );
        assert!(!titled.contains("PROJECT_SECRET_PATH_MARKER"));

        snapshot["result"]["snapshot"]["panes"][4]["label"] = Value::from("Named pane");
        let labeled = render(&snapshot.to_string(), Some("p-self"), 160).unwrap();
        assert!(
            labeled.contains("p-untitled | - | idle | Same / Same / Named pane"),
            "{labeled}"
        );
        assert!(!labeled.contains("PROJECT_SECRET_PATH_MARKER"));
        assert!(!labeled.contains("Same / Same / opencode"));
    }

    #[test]
    fn unnamed_pane_numbers_count_named_visible_rows_and_reset_per_tab() {
        let output = render(SNAPSHOT, Some("p-self"), 160).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert!(lines[1].ends_with("Same / Same / Same"), "{output}");
        assert!(lines[2].ends_with("Same / Same / Same"), "{output}");
        assert!(
            lines[3].contains("p-untitled | - | idle | Same / Same / pane 3"),
            "{output}"
        );
        assert!(
            lines[4].contains("p-untitled-2 | - | idle | Same / Same / pane 4"),
            "{output}"
        );
        assert!(
            lines[5].contains("W1 T2 p-second-tab | - | idle | Same / Second / pane 1"),
            "{output}"
        );
        assert!(
            lines[7].contains("W2 T1 p-other-untitled | - | idle | Same / Same / pane 2"),
            "{output}"
        );
        assert!(!output.contains("PROJECT_SECRET_PATH_MARKER"));
        assert!(!output.contains("p-self"));

        let without_named_pane = render(SNAPSHOT, Some("p-a"), 160).unwrap();
        assert!(!without_named_pane.contains("W1 T1 p-a |"));
        assert!(without_named_pane.contains("p-untitled | - | idle | Same / Same / pane 2"));
        assert!(without_named_pane.contains("p-untitled-2 | - | idle | Same / Same / pane 3"));
    }

    #[test]
    fn narrow_width_clips_names_but_retains_identifying_ids_agent_and_status() {
        let output = render(SNAPSHOT, Some("p-self"), 40).unwrap();
        for line in output.lines().skip(1).take(3) {
            assert!(columns(line) <= 40, "{line}");
        }
        assert!(output.contains("p-b | Claude | working"));
        assert!(output.contains("p-other | codex | idle"));
        assert!(output.contains('…'));
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
                .all(|line| columns(line) <= 40),
            "{output}"
        );
        assert!(output.contains("p-b"));
        assert!(output.contains("working"));
    }

    #[test]
    fn absent_own_pane_id_and_empty_topology() {
        assert!(render(SNAPSHOT, None, 100).unwrap().contains("p-self"));
        let empty = r#"{"id":"test","result":{"type":"session_snapshot","snapshot":{"workspaces":[],"tabs":[],"panes":[],"agents":[],"layouts":[]}}}"#;
        assert_eq!(
            render(empty, None, 40).unwrap(),
            "Herdr Crossroads\nPress Enter to close.\n"
        );
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
}
