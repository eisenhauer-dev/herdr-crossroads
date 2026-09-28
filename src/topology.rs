use std::collections::HashSet;
use std::io;
#[cfg(unix)]
use std::sync::atomic::Ordering;

use serde_json::Value;

use super::presentation::clean;
use super::transport::cli_output;
#[cfg(unix)]
use super::transport::RECONNECT_CANCEL;

pub(super) fn text<'a>(item: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| item.get(*name).and_then(Value::as_str))
}

pub(super) fn field<'a>(item: &'a Value, name: &str) -> io::Result<&'a str> {
    item.get(name).and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} in snapshot"),
        )
    })
}

pub(super) fn entries<'a>(snapshot: &'a Value, name: &str) -> io::Result<&'a [Value]> {
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

pub(super) struct Row {
    pub(super) id: String,
    pub(super) workspace_id: String,
    pub(super) workspace: String,
    pub(super) linked_worktree: bool,
    pub(super) tab: String,
    pub(super) pane: String,
    pub(super) agent: String,
    pub(super) status: String,
    pub(super) tab_id: String,
}

pub(super) fn rows(response: &str, own_pane: Option<&str>, _width: usize) -> io::Result<Vec<Row>> {
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
            .unwrap_or("");
        let status = agent
            .and_then(|a| text(a, &["agent_status"]))
            .or_else(|| text(pane, &["agent_status"]))
            .unwrap_or("");
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
        let workspace_name = clean(field(workspace, "label")?);
        let tab_name = clean(field(tab, "label")?);
        let pane_name = clean(&pane_name);
        rows.push((
            (wn, tn, index),
            Row {
                id: id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                workspace: workspace_name,
                linked_worktree: false,
                tab: tab_name,
                pane: pane_name,
                agent: clean(agent_name),
                status: if agent_name.is_empty() {
                    String::new()
                } else {
                    clean(status)
                },
                tab_id: format!("{workspace_id}/{tab_id}"),
            },
        ));
    }
    rows.sort_by_key(|(order, _)| *order);
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

pub(super) fn annotate_worktrees(rows: &mut Vec<Row>, executable: &std::ffi::OsStr, bounded: bool) {
    let workspace_ids: Vec<String> = rows.iter().map(|row| row.workspace_id.clone()).collect();
    let mut covered = HashSet::new();
    let mut annotated = HashSet::new();
    let mut links = Vec::new();
    for workspace_id in &workspace_ids {
        if !covered.insert(workspace_id.clone()) {
            continue;
        }
        let Ok(output) = cli_output(
            executable,
            &["worktree", "list", "--workspace", workspace_id],
            bounded,
        ) else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(data) = serde_json::from_slice::<Value>(&output.stdout) else {
            continue;
        };
        if data["result"]["type"] != "worktree_list" {
            continue;
        }
        let Some(worktrees) = data["result"]["worktrees"].as_array() else {
            continue;
        };
        for entry in worktrees {
            let Some(open_id) = entry["open_workspace_id"].as_str() else {
                continue;
            };
            if !workspace_ids.iter().any(|id| id == open_id) {
                continue;
            }
            covered.insert(open_id.to_owned());
            if entry["is_linked_worktree"] != true || !annotated.insert(open_id.to_owned()) {
                continue;
            }
            if let Some(source) = data["result"]["source"]["source_workspace_id"].as_str() {
                links.push((open_id.to_owned(), source.to_owned()));
            }
            for row in rows.iter_mut().filter(|row| row.workspace_id == open_id) {
                row.linked_worktree = true;
                row.workspace = match entry["branch"].as_str().filter(|branch| !branch.is_empty()) {
                    Some(branch) => format!("↳ {} ({})", row.workspace, clean(branch)),
                    None => format!("↳ {}", row.workspace),
                };
            }
        }
    }
    let mut original = Vec::new();
    for id in &workspace_ids {
        if !original.contains(id) {
            original.push(id.clone());
        }
    }
    fn append_block(
        id: &str,
        links: &[(String, String)],
        original: &[String],
        ordered: &mut Vec<String>,
    ) {
        if ordered.iter().any(|seen| seen == id) {
            return;
        }
        ordered.push(id.to_owned());
        for child in original {
            if links
                .iter()
                .any(|(linked, source)| linked == child && source == id)
            {
                append_block(child, links, original, ordered);
            }
        }
    }
    let mut ordered = Vec::new();
    for id in &original {
        if !links
            .iter()
            .any(|(linked, source)| linked == id && source != id && original.contains(source))
        {
            append_block(id, &links, &original, &mut ordered);
        }
    }
    for id in &original {
        append_block(id, &links, &original, &mut ordered);
    }
    rows.sort_by_key(|row| {
        ordered
            .iter()
            .position(|id| id == &row.workspace_id)
            .unwrap()
    });
}

pub(super) fn snapshot_rows(
    executable: &std::ffi::OsStr,
    width: usize,
    bounded: bool,
) -> io::Result<Vec<Row>> {
    let output = cli_output(executable, &["api", "snapshot"], bounded)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "herdr api snapshot failed: {}",
            output.status
        )));
    }
    let snapshot = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    let mut rows = rows(
        &snapshot,
        std::env::var("HERDR_PANE_ID").ok().as_deref(),
        width.saturating_sub(1),
    )?;
    annotate_worktrees(&mut rows, executable, bounded);
    #[cfg(unix)]
    if bounded
        && RECONNECT_CANCEL.with(|flag| {
            flag.borrow()
                .as_ref()
                .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "reconnect cancelled",
        ));
    }
    Ok(rows)
}
