use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row as TableRow, StatefulWidget, Table, TableState},
};
use serde_json::Value;
use std::cell::Cell;
use std::collections::HashSet;
use std::io::{self, IsTerminal, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::os::{
    fd::{AsRawFd, FromRawFd, OwnedFd},
    unix::ffi::OsStrExt,
};
use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::sync::mpsc::{self, Receiver, SyncSender};
#[cfg(unix)]
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

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

struct Row {
    id: String,
    workspace_id: String,
    workspace: String,
    linked_worktree: bool,
    tab: String,
    pane: String,
    agent: String,
    status: String,
    tab_id: String,
}

struct StatusColors {
    working: Color,
    blocked: Color,
    done: Color,
    idle: Color,
    unknown: Color,
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            working: Color::Rgb(249, 226, 175),
            blocked: Color::Rgb(243, 139, 168),
            done: Color::Rgb(148, 226, 213),
            idle: Color::Rgb(166, 227, 161),
            unknown: Color::Rgb(108, 112, 134),
        }
    }
}

impl StatusColors {
    fn for_status(&self, status: &str) -> Color {
        match status {
            "working" => self.working,
            "blocked" => self.blocked,
            "done" => self.done,
            "idle" => self.idle,
            _ => self.unknown,
        }
    }

    fn load(dir: Option<&Path>) -> io::Result<Self> {
        let mut colors = Self::default();
        let Some(dir) = dir else { return Ok(colors) };
        let path = dir.join("status-colors.json");
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(colors),
            Err(error) => return Err(io::Error::other(format!("{}: {error}", path.display()))),
        };
        let data: Value = serde_json::from_str(&content).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {error}", path.display()),
            )
        })?;
        let entries = data.as_object().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: expected JSON object", path.display()),
            )
        })?;
        for (key, value) in entries {
            let target = match key.as_str() {
                "working" => &mut colors.working,
                "blocked" => &mut colors.blocked,
                "done" => &mut colors.done,
                "idle" => &mut colors.idle,
                "unknown" => &mut colors.unknown,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{}: unknown status key {key}", path.display()),
                    ))
                }
            };
            let hex = value
                .as_str()
                .filter(|hex| {
                    hex.len() == 7
                        && hex.starts_with('#')
                        && hex.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{}: {key} must be #RRGGBB", path.display()),
                    )
                })?;
            *target = Color::Rgb(
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
            );
        }
        Ok(colors)
    }
}

fn rows(response: &str, own_pane: Option<&str>, _width: usize) -> io::Result<Vec<Row>> {
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

fn annotate_worktrees(rows: &mut Vec<Row>, executable: &std::ffi::OsStr, bounded: bool) {
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

#[cfg(test)]
fn render(response: &str, own_pane: Option<&str>, width: usize) -> io::Result<String> {
    Ok(Ui::new(rows(response, own_pane, width)?).frame_width(width))
}

fn matches_field(field: &str, query: &str) -> bool {
    let mut chars = field.to_lowercase().chars().collect::<Vec<_>>().into_iter();
    query
        .to_lowercase()
        .chars()
        .all(|c| chars.any(|next| next == c))
}

#[derive(PartialEq, Eq, Debug)]
enum Mode {
    Select,
    Search,
    Help,
}

struct Ui {
    rows: Vec<Row>,
    status_colors: StatusColors,
    selected: Option<String>,
    query: String,
    mode: Mode,
    closed: bool,
    width: usize,
    popup_size: Option<(usize, usize)>,
    viewport_start: Cell<usize>,
    last_selected_at: Cell<Option<usize>>,
}

impl Ui {
    fn new(rows: Vec<Row>) -> Self {
        let selected = rows.first().map(|row| row.id.clone());
        Self {
            rows,
            status_colors: StatusColors::default(),
            selected,
            query: String::new(),
            mode: Mode::Select,
            closed: false,
            width: 160,
            popup_size: None,
            viewport_start: Cell::new(0),
            last_selected_at: Cell::new(None),
        }
    }

    #[cfg(unix)]
    fn status_event(&mut self, event: &Value) -> bool {
        if event["event"] != "pane.agent_status_changed" {
            return false;
        }
        let data = &event["data"];
        let (Some(id), Some(workspace_id), Some(status)) = (
            data["pane_id"].as_str(),
            data["workspace_id"].as_str(),
            data["agent_status"].as_str(),
        ) else {
            return false;
        };
        if !matches!(status, "working" | "blocked" | "done" | "idle" | "unknown") {
            return false;
        }
        let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.id == id && row.workspace_id == workspace_id && !row.agent.is_empty())
        else {
            return false;
        };
        row.status = clean(status);
        true
    }

    fn visible(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                [&row.workspace, &row.tab, &row.pane, &row.agent]
                    .iter()
                    .any(|field| matches_field(field, &self.query))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn selected_index(&self, visible: &[usize]) -> Option<usize> {
        visible
            .iter()
            .copied()
            .find(|&i| self.selected.as_deref() == Some(&self.rows[i].id))
            .or_else(|| visible.first().copied())
    }

    fn key(&mut self, key: Key) -> Option<String> {
        if self
            .popup_size
            .is_some_and(|(width, height)| width < 80 || height < 15)
        {
            if matches!(key, Key::Enter | Key::Esc | Key::Char('q')) {
                self.closed = true;
            }
            return None;
        }
        if self.mode == Mode::Help {
            if matches!(key, Key::Esc | Key::Char('?') | Key::Char('q')) {
                self.mode = Mode::Select;
            }
            return None;
        }
        if self.mode == Mode::Search {
            match key {
                Key::Enter | Key::Esc => self.mode = Mode::Select,
                Key::Backspace => {
                    self.query.pop();
                }
                Key::Char(c) if !c.is_control() => self.query.push(c),
                _ => {}
            }
            return None;
        }
        let visible = self.visible();
        match key {
            Key::Char('/') => self.mode = Mode::Search,
            Key::Char('?') => self.mode = Mode::Help,
            Key::Char('q') | Key::Esc => self.closed = true,
            Key::Enter => {
                if let Some(i) = self.selected_index(&visible) {
                    self.closed = true;
                    return Some(self.rows[i].id.clone());
                }
            }
            Key::Char('r') => self.query.clear(),
            Key::Tab | Key::BackTab => {
                if let Some(i) = self.selected_index(&visible) {
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0
                                || self.rows[visible[at - 1]].workspace_id
                                    != self.rows[v].workspace_id)
                                .then_some(v)
                        })
                        .collect();
                    if let Some(at) = blocks.iter().rposition(|&b| b <= i).and_then(|at| {
                        if matches!(key, Key::Tab) {
                            blocks.get(at + 1)
                        } else {
                            at.checked_sub(1).and_then(|prev| blocks.get(prev))
                        }
                    }) {
                        self.selected = Some(self.rows[*at].id.clone());
                    }
                }
            }
            Key::Down | Key::Char('j') | Key::Up | Key::Char('k') => {
                if let Some(at) = self
                    .selected_index(&visible)
                    .and_then(|i| visible.iter().position(|v| *v == i))
                {
                    let next = if matches!(key, Key::Down | Key::Char('j')) {
                        (at + 1).min(visible.len() - 1)
                    } else {
                        at.saturating_sub(1)
                    };
                    self.selected = Some(self.rows[visible[next]].id.clone());
                }
            }
            Key::Left | Key::Char('h') | Key::Right | Key::Char('l') => {
                if let Some(i) = self.selected_index(&visible) {
                    let current = &self.rows[i].tab_id;
                    let blocks: Vec<usize> = visible
                        .iter()
                        .enumerate()
                        .filter_map(|(at, &v)| {
                            (at == 0 || self.rows[visible[at - 1]].tab_id != self.rows[v].tab_id)
                                .then_some(v)
                        })
                        .collect();
                    if let Some(at) = blocks.iter().position(|&b| &self.rows[b].tab_id == current) {
                        let target = if matches!(key, Key::Left | Key::Char('h')) {
                            at.checked_sub(1)
                        } else {
                            blocks.get(at + 1).map(|_| at + 1)
                        };
                        if let Some(next) = target {
                            self.selected = Some(self.rows[blocks[next]].id.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn frame(&self) -> String {
        self.frame_width(self.width)
    }

    fn frame_width(&self, width: usize) -> String {
        self.layout(width, usize::MAX, false).join("\n") + "\n"
    }

    fn terminal_frame(&self, width: usize, height: usize) -> String {
        if self.popup_size.is_some() && (width < 80 || height < 15) {
            return [
                "  Crossroads needs minimum content size 80x15.".to_owned(),
                format!("  Your content size is: {width}x{height}."),
                "  Set larger size in herdr plugin manifest.".to_owned(),
                "  Press Enter/Esc/q to close.".to_owned(),
            ]
            .into_iter()
            .take(height)
            .map(|line| clipped(&line, width.saturating_sub(1)))
            .collect::<Vec<_>>()
            .join("\r\n");
        }
        self.layout(width, height, true).join("\r\n")
    }

    fn layout(&self, width: usize, height: usize, ansi: bool) -> Vec<String> {
        let limit = width.saturating_sub(1);
        let table_text_limit = width.saturating_sub(4);
        let aligned = |text: &str| format!("  {}", clipped(text, table_text_limit));
        let visible = self.visible();
        let selected = self.selected_index(&visible);
        let mut lines = Vec::new();
        let search = if self.mode == Mode::Select && self.query.is_empty() {
            "Search: / to search".to_owned()
        } else {
            format!("Search: {}", self.query)
        };
        lines.push(aligned(&search));
        if self.mode == Mode::Help {
            lines.push(clipped(
                "j/k: pane  h/l: tab  Tab/Shift+Tab: workspace",
                limit,
            ));
            lines.push(clipped(
                "Enter: focus  /: search  r: reset  Esc/q/?: back",
                limit,
            ));
        } else {
            let table_limit = limit.saturating_sub(3);
            let inner = table_limit.saturating_sub(4);
            let roomy = inner >= 130;
            let table_margin = "  ";
            let short = ansi && height <= 6;
            let header_rule = roomy && !short && height >= 8 && !visible.is_empty();
            let footer_rows = if self.mode == Mode::Select && !roomy {
                2
            } else {
                1
            };
            let available = height.saturating_sub(
                1 + footer_rows + if short { 1 } else { 3 } + usize::from(header_rule),
            );
            let selected_at = visible.iter().position(|i| Some(*i) == selected);
            let span = |start: usize, end: usize| {
                (start..=end)
                    .map(|at| {
                        1 + usize::from(
                            at > start
                                && self.rows[visible[at - 1]].workspace_id
                                    != self.rows[visible[at]].workspace_id,
                        )
                    })
                    .sum::<usize>()
            };
            let mut start = if ansi {
                self.viewport_start
                    .get()
                    .min(visible.len().saturating_sub(1))
            } else {
                0
            };
            if let Some(at) = selected_at {
                if ansi
                    && !self
                        .last_selected_at
                        .get()
                        .is_some_and(|previous| previous.abs_diff(at) <= 1)
                {
                    start = at;
                    while start > 0 && span(start - 1, at) <= available {
                        start -= 1;
                    }
                }
                if at < start {
                    start = at;
                }
                if available > 0 {
                    while start < at && span(start, at) > available {
                        start += 1;
                    }
                }
            }
            if ansi {
                self.viewport_start.set(start);
                self.last_selected_at.set(selected_at);
            }
            let mut shown = Vec::new();
            for at in start..visible.len() {
                if ansi && (available == 0 || span(start, at) > available) {
                    break;
                }
                shown.push(visible[at]);
            }
            let full = inner >= 30 && !visible.is_empty();
            let context = inner >= 18 && !visible.is_empty();
            let workspace_width = if roomy {
                inner * 60 / 141
            } else if inner >= 30 {
                if inner >= 52 {
                    (inner * 27 / 52).clamp(5, if inner == 52 { 27 } else { 26 })
                } else {
                    (inner / 3).clamp(5, 24)
                }
            } else {
                (inner / 5).clamp(5, 22)
            };
            let tab_width = if roomy {
                inner * 38 / 141
            } else {
                if inner >= 52 {
                    (inner * 12 / 52).clamp(5, 22)
                } else {
                    (inner / 5).clamp(5, 22)
                }
            };
            let status_width = (inner / 6).clamp(6, 12);
            let separate_status = roomy && full;
            let name_width = if separate_status {
                inner.saturating_sub(workspace_width + tab_width + status_width + 3)
            } else if context {
                inner.saturating_sub(workspace_width + tab_width + 2)
            } else {
                inner
            };
            let mut table_rows = Vec::new();
            let mut separator_rows = Vec::new();
            if header_rule {
                separator_rows.push((2, false));
                table_rows.push(TableRow::new(vec![""; 4]));
            }
            for (position, &i) in shown.iter().enumerate() {
                let row = &self.rows[i];
                let previous = position.checked_sub(1).map(|at| &self.rows[shown[at]]);
                if previous.is_some_and(|prev| prev.workspace_id != row.workspace_id) {
                    separator_rows.push((
                        table_rows.len() + if short { 1 } else { 2 },
                        row.linked_worktree,
                    ));
                    table_rows.push(TableRow::new(vec![
                        "";
                        if separate_status {
                            4
                        } else if context {
                            3
                        } else {
                            1
                        }
                    ]));
                }
                let workspace =
                    if previous.is_some_and(|prev| prev.workspace_id == row.workspace_id) {
                        ""
                    } else {
                        &row.workspace
                    };
                let tab = if previous.is_some_and(|prev| prev.tab_id == row.tab_id) {
                    ""
                } else {
                    &row.tab
                };
                let name = if row.agent.is_empty() {
                    row.pane.clone()
                } else if !self.query.is_empty()
                    && matches_field(&row.pane, &self.query)
                    && row.pane != row.agent
                {
                    format!("{} ({})", row.agent, row.pane)
                } else {
                    row.agent.clone()
                };
                let mut cells = Vec::new();
                if context {
                    cells.push(Line::raw(clipped(workspace, workspace_width)));
                    cells.push(Line::raw(clipped(tab, tab_width)));
                }
                let inline = full
                    && !separate_status
                    && !row.agent.is_empty()
                    && !row.status.is_empty()
                    && name_width >= columns(&row.status) + if inner >= 52 { 4 } else { 9 };
                if inline {
                    let prefix_width = name_width - columns(&row.status) - 3;
                    let prefix = if prefix_width == 1 {
                        name.chars()
                            .next()
                            .filter(char::is_ascii)
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| ".".to_owned())
                    } else {
                        clipped(&name, prefix_width)
                    };
                    cells.push(Line::from(vec![
                        Span::raw(prefix),
                        Span::raw(" "),
                        Span::styled("|", Style::default().add_modifier(Modifier::DIM)),
                        Span::raw(" "),
                        Span::styled(
                            row.status.clone(),
                            Style::default().fg(self.status_colors.for_status(&row.status)),
                        ),
                    ]));
                } else {
                    cells.push(Line::raw(clipped(&name, name_width)));
                }
                if separate_status {
                    cells.push(Line::from(Span::styled(
                        clipped(&row.status, status_width),
                        Style::default().fg(self.status_colors.for_status(&row.status)),
                    )));
                }
                table_rows.push(TableRow::new(cells));
            }
            if shown.is_empty() {
                let mut cells = if context {
                    vec![Line::raw(""), Line::raw("")]
                } else {
                    vec![]
                };
                cells.push(Line::raw("No matching panes"));
                if separate_status {
                    cells.push(Line::raw(""));
                }
                table_rows.push(TableRow::new(cells));
            }
            let table_height = if ansi {
                height.saturating_sub(1 + footer_rows)
            } else {
                table_rows.len() + 3
            };
            if table_limit >= 8 && table_height >= if short { 2 } else { 3 } {
                let area = Rect::new(
                    0,
                    0,
                    table_limit.min(u16::MAX as usize) as u16,
                    table_height.min(u16::MAX as usize) as u16,
                );
                let mut buffer = Buffer::empty(area);
                let mut widths = Vec::new();
                let mut headers = Vec::new();
                if context {
                    widths.extend([
                        Constraint::Length(workspace_width as u16),
                        Constraint::Length(tab_width as u16),
                    ]);
                    headers.extend(["Workspace", "Tab"]);
                }
                widths.push(Constraint::Length(name_width as u16));
                headers.push("Agent/Pane Name");
                if separate_status {
                    widths.push(Constraint::Length(status_width as u16));
                    headers.push("Status");
                }
                let mut table = Table::new(table_rows, widths)
                    .block(Block::default().borders(if short {
                        Borders::TOP
                    } else {
                        Borders::TOP | Borders::BOTTOM
                    }))
                    .column_spacing(1)
                    .row_highlight_style(Style::default().bg(Color::Indexed(236)))
                    .highlight_symbol("> ");
                if !short {
                    table = table.header(TableRow::new(headers));
                }
                let selected_row = shown.iter().position(|&i| Some(i) == selected).map(|at| {
                    at + usize::from(header_rule)
                        + shown[..=at]
                            .windows(2)
                            .filter(|pair| {
                                self.rows[pair[0]].workspace_id != self.rows[pair[1]].workspace_id
                            })
                            .count()
                });
                let mut state = TableState::default().with_selected(selected_row);
                StatefulWidget::render(table, area, &mut buffer, &mut state);
                for y in 0..area.height {
                    let selected_line = ansi
                        && selected_row
                            .is_some_and(|at| y as usize == at + if short { 1 } else { 2 });
                    let mut content = String::new();
                    let mut x = 0;
                    let mut foreground = None;
                    let mut dim = false;
                    let last = (0..area.width).rfind(|&x| buffer[(x, y)].symbol() != " ");
                    while x < area.width {
                        let cell = &buffer[(x, y)];
                        if last.is_none_or(|last| x > last) {
                            break;
                        }
                        if ansi && cell.fg != foreground.unwrap_or(Color::Reset) {
                            if let Color::Rgb(r, g, b) = cell.fg {
                                content.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
                                foreground = Some(cell.fg);
                            } else if foreground.is_some() {
                                content.push_str("\x1b[39m");
                                foreground = None;
                            }
                        }
                        if ansi && cell.modifier.contains(Modifier::DIM) != dim {
                            dim = !dim;
                            content.push_str(if dim { "\x1b[2m" } else { "\x1b[22m" });
                        }
                        content.push_str(cell.symbol());
                        x += cell.cell_width().max(1);
                    }
                    if ansi && foreground.is_some() {
                        content.push_str("\x1b[39m");
                    }
                    if ansi && dim {
                        content.push_str("\x1b[22m");
                    }
                    if selected_line {
                        content.push_str(&" ".repeat(table_limit.saturating_sub(x as usize)));
                    }
                    let inset = separator_rows
                        .iter()
                        .find_map(|(at, inset)| (*at == y as usize).then_some(*inset));
                    let content = if let Some(inset) = inset {
                        let rule =
                            "┄".repeat(table_limit.saturating_sub(if inset { 4 } else { 0 }));
                        let margin = if inset { "      " } else { table_margin };
                        if ansi {
                            format!("{margin}\x1b[2m{rule}\x1b[22m")
                        } else {
                            format!("{margin}{rule}")
                        }
                    } else {
                        format!("{table_margin}{content}")
                    };
                    if selected_line {
                        lines.push(format!(
                            "{table_margin}\x1b[48;5;236m{}\x1b[0m",
                            &content[table_margin.len()..]
                        ));
                    } else {
                        lines.push(content);
                    }
                }
            }
        }
        let footer = if self.mode == Mode::Help {
            "Esc/q/?: back"
        } else if self.mode == Mode::Search {
            "Enter/Esc: select  Backspace: erase"
        } else if width >= 138 {
            "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit"
        } else {
            "Enter: focus · Esc/q: exit"
        };
        let compact_select = self.mode == Mode::Select && width < 138;
        if ansi {
            let body_height = height.saturating_sub(if compact_select { 2 } else { 1 });
            lines.truncate(body_height);
            lines.resize(body_height, String::new());
        }
        if !ansi || height > 0 {
            if self.mode == Mode::Select {
                if compact_select && (!ansi || height > 1) {
                    lines.push(aligned("j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace"));
                }
                lines.push(aligned(&format!("{footer} · ?: keys")));
            } else {
                lines.push(aligned(footer));
            }
        }
        lines
    }
}

#[derive(Clone, Copy)]
enum Key {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
}

struct Keys {
    escape: u8,
    utf8: Vec<u8>,
}
impl Keys {
    fn new() -> Self {
        Self {
            escape: 0,
            utf8: Vec::new(),
        }
    }
    fn feed(&mut self, byte: u8) -> Option<Key> {
        if self.escape == 1 {
            self.escape = 0;
            if byte == b'[' {
                self.escape = 2;
                return None;
            }
            return Some(Key::Esc);
        }
        if self.escape == 2 {
            self.escape = 0;
            return match byte {
                b'A' => Some(Key::Up),
                b'B' => Some(Key::Down),
                b'C' => Some(Key::Right),
                b'D' => Some(Key::Left),
                b'Z' => Some(Key::BackTab),
                _ => None,
            };
        }
        match byte {
            27 => {
                self.escape = 1;
                None
            }
            b'\r' | b'\n' => Some(Key::Enter),
            b'\t' => Some(Key::Tab),
            127 | 8 => Some(Key::Backspace),
            _ => {
                self.utf8.push(byte);
                match std::str::from_utf8(&self.utf8) {
                    Ok(s) => {
                        let c = s.chars().next().map(Key::Char);
                        self.utf8.clear();
                        c
                    }
                    Err(e) if e.error_len().is_none() && self.utf8.len() < 4 => None,
                    Err(_) => {
                        self.utf8.clear();
                        None
                    }
                }
            }
        }
    }
    fn flush(&mut self) -> Option<Key> {
        if self.escape == 1 {
            self.escape = 0;
            Some(Key::Esc)
        } else {
            None
        }
    }
}

fn terminal_size() -> (usize, usize) {
    let measured = Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let mut fields = s.split_whitespace();
            Some((
                fields.next()?.parse::<usize>().ok()?,
                fields.next()?.parse::<usize>().ok()?,
            ))
        });
    let width = measured
        .map(|(_, width)| width)
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok()))
        .filter(|n| *n > 0)
        .unwrap_or(80);
    (
        measured
            .map(|(height, _)| height)
            .filter(|n| *n > 0)
            .unwrap_or(24),
        width,
    )
}

struct Terminal(Option<String>);
impl Terminal {
    fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() {
            return Ok(Self(None));
        }
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()?;
        if !saved.status.success() {
            return Err(io::Error::other("stty -g failed"));
        }
        let state = String::from_utf8(saved.stdout).map_err(io::Error::other)?;
        let status = Command::new("stty")
            .args(["raw", "-echo", "min", "0", "time", "1"])
            .stdin(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(io::Error::other("stty raw failed"));
        }
        Ok(Self(Some(state.trim().to_owned())))
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(state) = &self.0 {
            let mut out = io::stdout().lock();
            let _ = out.write_all(b"\x1b[?25h");
            let _ = out.flush();
            let _ = Command::new("stty")
                .arg(state)
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

fn draw(ui: &Ui, terminal: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if terminal {
        let (width, height) = ui.popup_size.expect("interactive terminal size");
        let frame = ui.terminal_frame(width, height);
        write!(out, "\x1b[H\x1b[J{frame}")?;
        if ui.mode == Mode::Search && width >= 80 && height >= 15 {
            let x = frame.split("\r\n").next().unwrap_or("").cell_width() as usize;
            write!(
                out,
                "\x1b[1;{}H\x1b[?25h",
                x.min(width.saturating_sub(1)) + 1
            )?;
        } else {
            out.write_all(b"\x1b[?25l")?;
        }
    } else {
        out.write_all(ui.frame().as_bytes())?;
    }
    out.flush()
}

#[cfg(unix)]
fn focus(pane_id: &str) -> io::Result<()> {
    let path = std::env::var_os("HERDR_SOCKET_PATH")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERDR_SOCKET_PATH is missing"))?;
    let mut stream = UnixStream::connect(path)?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({"id":"crossroads-focus","method":"pane.focus","params":{"pane_id":pane_id}}),
    )?;
    stream.write_all(b"\n")?;
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut reply = Vec::new();
    loop {
        if reply.len() >= 16 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pane.focus reply too long",
            ));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "pane.focus reply timed out"))?;
        stream.set_read_timeout(Some(remaining))?;
        let mut byte = [0];
        let n = stream.read(&mut byte).map_err(|error| {
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) {
                io::Error::new(io::ErrorKind::TimedOut, "pane.focus reply timed out")
            } else {
                error
            }
        })?;
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "pane.focus reply timed out",
            ));
        }
        if n == 0 {
            break;
        }
        reply.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    let response: Value = serde_json::from_slice(&reply).map_err(io::Error::other)?;
    if response["id"] != "crossroads-focus"
        || response["error"].is_object()
        || response["result"]["type"] != "pane_info"
        || response["result"]["pane"]["pane_id"] != pane_id
        || response["result"]["pane"]["focused"] != true
    {
        return Err(io::Error::other(format!("pane.focus failed: {response}")));
    }
    Ok(())
}

#[cfg(not(unix))]
fn focus(_pane_id: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "pane focus requires a Unix socket",
    ))
}

fn handle_key(ui: &mut Ui, key: Key, terminal: bool) -> io::Result<()> {
    if let Some(id) = ui.key(key) {
        focus(&id)?;
    }
    if !ui.closed {
        draw(ui, terminal)?;
    }
    Ok(())
}

#[cfg(unix)]
enum StatusMessage {
    Event(Value),
    Disconnected,
}

#[cfg(unix)]
thread_local! {
    static RECONNECT_CANCEL: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(unix)]
struct ReconnectAttempt {
    rx: Receiver<io::Result<(StatusSubscription, Vec<Row>)>>,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl Drop for ReconnectAttempt {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(unix)]
struct StatusSubscription {
    rx: Receiver<StatusMessage>,
    socket: UnixStream,
}

#[cfg(unix)]
impl Drop for StatusSubscription {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}

// Reconnect CLI output goes to a file rather than a pipe: a stuck child (or a child
// retaining stdout after its parent exits) cannot keep a pipe reader thread alive.
fn cli_output(
    executable: &std::ffi::OsStr,
    args: &[&str],
    bounded: bool,
) -> io::Result<std::process::Output> {
    if !bounded {
        return Command::new(executable).args(args).output();
    }
    #[cfg(unix)]
    {
        let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let temp_dir = Path::new("/tmp/opencode");
        let dir = if temp_dir.is_dir() {
            temp_dir.to_path_buf()
        } else {
            std::env::temp_dir()
        };
        let path = dir.join(format!(
            "crossroads-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let result = (|| {
            let mut child = Command::new(executable)
                .args(args)
                .stdout(Stdio::from(file))
                .stderr(Stdio::null())
                .spawn()?;
            let deadline = Instant::now() + Duration::from_secs(2);
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => {}
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(error);
                    }
                }
                if cancel
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Relaxed))
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "reconnect cancelled",
                    ));
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "reconnect CLI timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            };
            if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reconnect CLI output too large",
                ));
            }
            Ok(std::process::Output {
                status,
                stdout: std::fs::read(&path)?,
                stderr: Vec::new(),
            })
        })();
        let _ = std::fs::remove_file(path);
        result
    }
    #[cfg(not(unix))]
    unreachable!("bounded CLI is only used by Unix reconnects")
}

#[cfg(unix)]
fn socket_line(stream: &mut UnixStream, deadline: Option<Instant>) -> io::Result<Value> {
    let mut bytes = Vec::new();
    let mut byte = [0];
    let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
    loop {
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        if let Some(deadline) = deadline {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "subscription ack timed out")
                })?;
            stream.set_read_timeout(Some(if cancel.is_some() {
                remaining.min(Duration::from_millis(20))
            } else {
                remaining
            }))?;
        }
        if bytes.len() >= 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "event line too long",
            ));
        }
        let read = match stream.read(&mut byte) {
            Err(error)
                if cancel.is_some()
                    && matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
            {
                continue
            }
            result => result?,
        };
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "event socket closed",
            ));
        }
        if byte[0] == b'\n' {
            return serde_json::from_slice(&bytes).map_err(io::Error::other);
        }
        bytes.push(byte[0]);
    }
}

#[cfg(unix)]
fn pane_ids(executable: &std::ffi::OsStr, bounded: bool) -> io::Result<Vec<String>> {
    let output = cli_output(executable, &["pane", "list"], bounded)?;
    if !output.status.success() {
        return Err(io::Error::other("herdr pane list failed"));
    }
    let data: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    if data["result"]["type"] != "pane_list" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a pane list",
        ));
    }
    let panes = entries(&data["result"], "panes")?;
    let mut ids = Vec::new();
    for pane in panes {
        let id = field(pane, "pane_id")?;
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

// The Linux x86_64 socket constants and sockaddr_un layout match the local
// sys/socket.h and sys/un.h ABI; other targets keep the existing path.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn reconnect_socket(path: &std::ffi::OsStr) -> io::Result<UnixStream> {
    #[repr(C)]
    struct SocketAddress {
        family: u16,
        path: [u8; 108],
    }
    unsafe extern "C" {
        fn socket(
            domain: std::ffi::c_int,
            kind: std::ffi::c_int,
            protocol: std::ffi::c_int,
        ) -> std::ffi::c_int;
        fn connect(
            fd: std::ffi::c_int,
            address: *const std::ffi::c_void,
            length: u32,
        ) -> std::ffi::c_int;
    }
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes.len() >= 108 || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Unix socket path",
        ));
    }
    let mut address = SocketAddress {
        family: 1,
        path: [0; 108],
    }; // AF_UNIX
    address.path[..bytes.len()].copy_from_slice(bytes);
    let length = (std::mem::offset_of!(SocketAddress, path) + bytes.len() + 1) as u32;
    let deadline = Instant::now() + Duration::from_millis(500);
    let cancel = RECONNECT_CANCEL.with(|flag| flag.borrow().clone());
    loop {
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "reconnect cancelled",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "reconnect connect timed out",
            ));
        }
        // SAFETY: these are the verified Linux x86_64 AF_UNIX, SOCK_STREAM,
        // SOCK_NONBLOCK and SOCK_CLOEXEC constants; socket returns an owned fd.
        let raw = unsafe { socket(1, 1 | 0o4000 | 0o2000000, 0) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: socket returned a fresh valid descriptor, transferred exactly once.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: address is a live repr(C) sockaddr_un prefix, length includes
        // its terminating NUL and never exceeds the allocated address.
        let result = unsafe {
            connect(
                fd.as_raw_fd(),
                (&address as *const SocketAddress).cast(),
                length,
            )
        };
        if result == 0 {
            let stream = UnixStream::from(fd);
            stream.set_nonblocking(false)?;
            if cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "reconnect cancelled",
                ));
            }
            return Ok(stream);
        }
        let error = io::Error::last_os_error();
        // A failed nonblocking connect leaves socket state unspecified: drop it
        // and create a fresh socket for every retry, including EAGAIN.
        drop(fd);
        if error.raw_os_error() != Some(11) {
            return Err(error);
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(20)),
        );
    }
}

#[cfg(unix)]
fn subscribe(executable: &std::ffi::OsStr, bounded: bool) -> io::Result<StatusSubscription> {
    let path = std::env::var_os("HERDR_SOCKET_PATH")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERDR_SOCKET_PATH missing"))?;
    let ids = pane_ids(executable, bounded)?;
    let mut stream = if bounded {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            reconnect_socket(&path)?
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            UnixStream::connect(&path)?
        }
    } else {
        UnixStream::connect(&path)?
    };
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({
            "id":"crossroads-status", "method":"events.subscribe",
            "params":{"subscriptions":ids.iter().map(|id| serde_json::json!({"type":"pane.agent_status_changed","pane_id":id})).collect::<Vec<_>>()}
        }),
    )?;
    stream.write_all(b"\n")?;
    let reply = socket_line(
        &mut stream,
        Some(Instant::now() + Duration::from_millis(500)),
    )?;
    if reply["id"] != "crossroads-status" || reply["result"]["type"] != "subscription_started" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "status subscription rejected",
        ));
    }
    stream.set_read_timeout(None)?;
    let socket = stream.try_clone()?;
    let (tx, rx): (SyncSender<StatusMessage>, _) = mpsc::sync_channel(1024);
    thread::spawn(move || {
        while let Ok(event) = socket_line(&mut stream, None) {
            if tx.send(StatusMessage::Event(event)).is_err() {
                return;
            }
        }
        let _ = tx.send(StatusMessage::Disconnected);
    });
    Ok(StatusSubscription { rx, socket })
}

fn snapshot_rows(
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

fn main() -> io::Result<()> {
    let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").filter(|dir| !dir.is_empty());
    let status_colors = StatusColors::load(config_dir.as_deref().map(Path::new))?;
    let executable = std::env::var_os("HERDR_BIN_PATH")
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "herdr".into());
    let width = terminal_size().1;
    #[cfg(unix)]
    let live_status_enabled =
        std::env::var_os("HERDR_SOCKET_PATH").is_some_and(|path| !path.is_empty());
    #[cfg(unix)]
    let mut status = if live_status_enabled {
        subscribe(&executable, false).ok()
    } else {
        None
    };
    let mut ui = Ui::new(snapshot_rows(&executable, width, false)?);
    ui.status_colors = status_colors;
    ui.width = width;
    let terminal = Terminal::enter()?;
    if terminal.0.is_some() {
        let (height, width) = terminal_size();
        ui.popup_size = Some((width, height));
    }
    #[cfg(unix)]
    if let Some(receiver) = &status {
        for _ in 0..1024 {
            match receiver.rx.try_recv() {
                Ok(StatusMessage::Event(event)) => {
                    ui.status_event(&event);
                }
                Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                    status = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }
    draw(&ui, terminal.0.is_some())?;
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    let raw_terminal = terminal.0.is_some();
    thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut byte = [0];
        loop {
            match input.read(&mut byte) {
                Ok(0) => {
                    if input_tx.send(None).is_err() || !raw_terminal {
                        break;
                    }
                }
                Ok(_) => {
                    if input_tx.send(Some(byte[0])).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = input_tx.send(None);
                    break;
                }
            }
        }
    });
    let mut keys = Keys::new();
    #[cfg(unix)]
    let mut next_size_check = Instant::now() + Duration::from_millis(250);
    #[cfg(unix)]
    let mut retry_at = Instant::now() + Duration::from_millis(500);
    #[cfg(unix)]
    let mut reconnect: Option<ReconnectAttempt> = None;
    #[cfg(unix)]
    let mut pending: Option<(StatusSubscription, Ui)> = None;
    loop {
        #[cfg(unix)]
        if terminal.0.is_some() && Instant::now() >= next_size_check {
            let (height, width) = terminal_size();
            if ui.popup_size != Some((width, height)) {
                let was_small = ui.popup_size.is_some_and(|(w, h)| w < 80 || h < 15);
                let is_small = width < 80 || height < 15;
                ui.popup_size = Some((width, height));
                if was_small != is_small {
                    // Do not apply buffered keys from the previous popup mode after resizing.
                    keys = Keys::new();
                    while input_rx.try_recv().is_ok() {}
                }
                draw(&ui, true)?;
            }
            next_size_check = Instant::now() + Duration::from_millis(250);
        }
        #[cfg(unix)]
        {
            let mut disconnected = false;
            if let Some(receiver) = &status {
                let mut changed = false;
                for _ in 0..1024 {
                    match receiver.rx.try_recv() {
                        Ok(StatusMessage::Event(event)) => {
                            changed |= ui.status_event(&event);
                        }
                        Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if changed {
                    draw(&ui, terminal.0.is_some())?;
                }
            }
            if disconnected {
                status = None;
                retry_at = Instant::now() + Duration::from_millis(500);
            }
            if let Some(worker) = &reconnect {
                match worker.rx.try_recv() {
                    Ok(Ok((receiver, rows))) => {
                        pending = Some((receiver, Ui::new(rows)));
                        reconnect = None;
                    }
                    Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                        reconnect = None;
                        retry_at = Instant::now() + Duration::from_millis(500);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some((receiver, staged)) = &mut pending {
                for _ in 0..1024 {
                    match receiver.rx.try_recv() {
                        Ok(StatusMessage::Event(event)) => {
                            staged.status_event(&event);
                        }
                        Ok(StatusMessage::Disconnected) | Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if disconnected {
                    pending = None;
                    retry_at = Instant::now() + Duration::from_millis(500);
                } else {
                    let (receiver, staged) = pending.take().unwrap();
                    ui.rows = staged.rows;
                    status = Some(receiver);
                    draw(&ui, terminal.0.is_some())?;
                }
            }
            if live_status_enabled
                && status.is_none()
                && reconnect.is_none()
                && pending.is_none()
                && Instant::now() >= retry_at
            {
                let (tx, rx) = mpsc::sync_channel(1);
                let executable = executable.clone();
                let cancel = Arc::new(AtomicBool::new(false));
                let worker_cancel = cancel.clone();
                let worker = thread::spawn(move || {
                    RECONNECT_CANCEL.with(|flag| *flag.borrow_mut() = Some(worker_cancel));
                    let result = subscribe(&executable, true).and_then(|receiver| {
                        snapshot_rows(&executable, width, true).map(|rows| (receiver, rows))
                    });
                    let _ = tx.send(result);
                });
                reconnect = Some(ReconnectAttempt {
                    rx,
                    cancel,
                    worker: Some(worker),
                });
                retry_at = Instant::now() + Duration::from_millis(500);
            }
        }
        let byte = match input_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(byte)) => byte,
            Ok(None) => {
                if let Some(key) = keys.flush() {
                    handle_key(&mut ui, key, terminal.0.is_some())?;
                }
                if terminal.0.is_none() || ui.closed {
                    break;
                }
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        };
        if keys.escape == 1 && byte != b'[' {
            if let Some(key) = keys.flush() {
                handle_key(&mut ui, key, terminal.0.is_some())?;
            }
            if ui.closed {
                break;
            }
        }
        if let Some(key) = keys.feed(byte) {
            handle_key(&mut ui, key, terminal.0.is_some())?;
            if ui.closed {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = include_str!("../tests/fixtures/topology.json");

    #[cfg(unix)]
    #[test]
    fn reconnect_cli_kills_a_stalled_command_at_its_deadline() {
        let started = Instant::now();
        let error =
            cli_output(std::ffi::OsStr::new("sh"), &["-c", "exec sleep 5"], true).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    fn ui() -> Ui {
        Ui::new(rows(SNAPSHOT, Some("p-self"), 160).unwrap())
    }

    #[test]
    fn undersized_popup_displays_only_notice_and_ignores_keys_without_losing_state() {
        let mut state = ui();
        state.key(Key::Down);
        state.key(Key::Char('/'));
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        state.key(Key::Enter);
        let selected = state.selected.clone();
        let query = state.query.clone();
        let ids: Vec<_> = state.rows.iter().map(|row| row.id.clone()).collect();
        for (width, height) in [(67, 12), (79, 15), (80, 14)] {
            state.popup_size = Some((width, height));
            let piped = state.frame_width(width);
            assert!(piped.contains("Search: codex") && !piped.contains("Crossroads needs"));
            let frame = state.terminal_frame(width, height);
            assert_eq!(
                frame.split("\r\n").collect::<Vec<_>>(),
                [
                    "  Crossroads needs minimum content size 80x15.".to_owned(),
                    format!("  Your content size is: {width}x{height}."),
                    "  Set larger size in herdr plugin manifest.".to_owned(),
                    "  Press Enter/Esc/q to close.".to_owned(),
                ],
                "{frame}"
            );
            assert!(
                !frame.contains("Search:") && !frame.contains("Claude") && !frame.contains("focus"),
                "{frame}"
            );
            for key in [Key::Char('/'), Key::Down, Key::Char('?')] {
                assert!(state.key(key).is_none());
                assert!(!state.closed);
            }
            assert_eq!(state.query, query);
            assert_eq!(state.selected, selected);
            assert_eq!(
                state
                    .rows
                    .iter()
                    .map(|row| row.id.clone())
                    .collect::<Vec<_>>(),
                ids
            );
        }
        state.popup_size = Some((80, 15));
        assert!(state.terminal_frame(80, 15).contains("Search: codex"));
        assert_eq!(state.query, query);
        assert_eq!(state.selected, selected);
        for key in [Key::Enter, Key::Esc, Key::Char('q')] {
            state.closed = false;
            state.popup_size = Some((79, 15));
            assert!(state.key(key).is_none(), "must not request focus");
            assert!(state.closed);
        }
        for width in [1, 2, 20] {
            state.popup_size = Some((width, 12));
            let frame = state.terminal_frame(width, 12);
            assert_eq!(frame.split("\r\n").count(), 4, "{frame}");
            assert!(frame
                .split("\r\n")
                .all(|line| usize::from(line.cell_width()) < width));
        }
    }

    #[test]
    fn undersized_popup_enter_closes_without_requesting_focus() {
        let mut state = ui();
        state.popup_size = Some((79, 15));
        assert!(state.key(Key::Enter).is_none());
        assert!(state.closed);
    }

    #[cfg(unix)]
    #[test]
    fn live_status_only_changes_matching_agent_row_not_navigation() {
        let mut state = ui();
        state.key(Key::Char('/'));
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        state.key(Key::Enter);
        let original_ids: Vec<_> = state.rows.iter().map(|row| row.id.clone()).collect();
        let selected = state.selected.clone();
        for (id, workspace) in [("p-other", "w1"), ("closed", "w2")] {
            assert!(!state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":id,"workspace_id":workspace,"agent_status":"blocked"}})));
        }
        assert!(state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-other","workspace_id":"w2","agent_status":"blocked"}})));
        assert_eq!(
            state
                .rows
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>(),
            original_ids
        );
        assert_eq!(state.selected, selected);
        assert_eq!(state.query, "codex");
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
        assert!(state.frame_width(65).contains("codex | blocked"));
        assert!(!state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-a","workspace_id":"w1","agent_status":"working","display_agent":"new agent"}})));
        state.query.clear();
        assert!(!state.frame_width(150).contains("new agent"));
        assert!(state
            .rows
            .iter()
            .find(|row| row.id == "p-a")
            .unwrap()
            .status
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn status_metadata_cannot_rename_filtered_agent_or_redirect_focus() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["agents"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"pane_id":"p-a","display_agent":"Claude Extra","agent_status":"idle"}));
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        state.query = "Claude".into();
        let visible_ids = |state: &Ui| {
            state
                .visible()
                .iter()
                .map(|&i| state.rows[i].id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(visible_ids(&state), ["p-b", "p-a"]);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        assert!(state.status_event(&serde_json::json!({"event":"pane.agent_status_changed","data":{"pane_id":"p-b","workspace_id":"w1","agent_status":"blocked","display_agent":"Renamed","agent":"Other"}})));
        assert_eq!(state.query, "Claude");
        assert_eq!(visible_ids(&state), ["p-b", "p-a"]);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        assert_eq!(
            state.rows.iter().find(|row| row.id == "p-b").unwrap().agent,
            "Claude"
        );
        assert!(state.frame_width(150).contains("Claude"));
        assert!(!state.frame_width(150).contains("Renamed"));
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-b"));
    }

    fn without_status_ansi(line: &str) -> String {
        line.replace("\x1b[38;2;249;226;175m", "")
            .replace("\x1b[38;2;166;227;161m", "")
            .replace("\x1b[39m", "")
    }

    fn selected_prefix(line: &str) -> bool {
        line.starts_with("  \x1b[48;5;236m> ")
    }

    #[test]
    fn search_and_keyboard_footer_share_table_extent_in_both_modes() {
        use ratatui::buffer::CellWidth;
        for width in [12, 20, 60, 150] {
            let mut state = ui();
            for mode in [Mode::Select, Mode::Search] {
                state.mode = mode;
                state.query = "界".repeat(100);
                let frame = state.terminal_frame(width, 15);
                let lines: Vec<_> = frame.split("\r\n").collect();
                let border = lines[1].cell_width() as usize;
                for line in [lines[0], lines[14]] {
                    assert!(line.starts_with("  "), "{frame}");
                    assert!(line.cell_width() as usize <= border, "{frame}");
                }
                if width >= 20 {
                    assert!(lines[0].starts_with("  Search:"), "{frame}");
                }
                assert!(!frame.contains("Filter:"), "{frame}");
            }
            state.mode = Mode::Select;
            state.query.clear();
            let frame = state.terminal_frame(width, 15);
            if width >= 24 {
                assert!(frame.starts_with("  Search: / to search"), "{frame}");
            }
            assert!(!frame.contains("Filter:"), "{frame}");
        }
    }

    #[test]
    fn sixty_columns_allocate_workspace_tab_and_minimal_inline_agent_status() {
        let state = ui();
        let frame = state.terminal_frame(60, 15);
        let header = frame.split("\r\n").nth(2).unwrap();
        assert_eq!(
            ["Workspace", "Tab", "Agent/Pane"].map(|label| header.find(label).unwrap()),
            [4, 32, 45],
            "{frame}"
        );
        let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
        assert!(
            selected.contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[39m"),
            "{frame}"
        );
        assert!(!header.contains("Status"), "{frame}");
        assert!(
            frame
                .lines()
                .any(|line| line.contains("pane 3") && !line.contains('|')),
            "{frame}"
        );
        assert!(state.terminal_frame(30, 15).contains("Agent/Pane"));
        assert!(!state.terminal_frame(30, 15).contains("working"));
        let mut too_long = ui();
        too_long.rows[0].status = "busy_now".into();
        let frame = too_long.terminal_frame(60, 15);
        let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
        assert!(
            selected.contains("Claude") && !selected.contains("busy_now"),
            "{frame}"
        );
    }

    #[test]
    fn compact_sixty_column_budget_preserves_workspace_and_status() {
        let state = ui();
        let frame = state.terminal_frame(60, 15);
        let header = frame.split("\r\n").nth(2).unwrap();
        assert_eq!(
            ["Workspace", "Tab", "Agent/Pane"].map(|s| header.find(s).unwrap()),
            [4, 32, 45],
            "{frame}"
        );
        assert!(
            frame
                .lines()
                .any(|line| line.contains("C \x1b[2m|") && line.contains("working")),
            "{frame}"
        );
        assert!(state.terminal_frame(40, 15).contains("Agent/Pane"));
        assert!(state.terminal_frame(150, 15).contains("Status"));
    }

    #[test]
    fn ninety_percent_wide_terminal_header_rule_is_present_only_when_roomy_and_tall() {
        let state = ui();
        let frame = state.terminal_frame(162, 36);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert!(lines[2].contains("Status"), "{frame}");
        assert_eq!(
            lines[3],
            format!("  \x1b[2m{}\x1b[22m", "┄".repeat(158)),
            "{frame}"
        );
        assert!(!state
            .terminal_frame(135, 36)
            .split("\r\n")
            .nth(3)
            .unwrap()
            .contains('┄'));
        assert!(!state.terminal_frame(162, 7).contains('┄'));
    }

    #[test]
    fn titleless_layout_preserves_selection_filters_and_short_viewport() {
        for width in [60, 150] {
            let mut state = ui();
            for mode in [Mode::Select, Mode::Search, Mode::Help] {
                let search = mode == Mode::Search;
                let help = mode == Mode::Help;
                state.mode = mode;
                let frame = state.terminal_frame(width, 15);
                let lines: Vec<_> = frame.split("\r\n").collect();
                assert_eq!(lines.len(), 15, "{frame}");
                assert!(
                    lines[0].starts_with(if search || help {
                        "  Search: "
                    } else {
                        "  Search: / to search"
                    }),
                    "{frame}"
                );
                assert!(!frame.contains("Herdr Crossroads"), "{frame}");
                if !help {
                    assert!(lines[1].starts_with("  ─"), "{frame}");
                    assert!(
                        lines[2].contains(if width == 60 {
                            "Agent/Pane"
                        } else {
                            "Agent/Pane Name"
                        }),
                        "{frame}"
                    );
                    assert!(
                        lines[if width == 150 { 4 } else { 3 }].contains(if width == 60 {
                            "C \x1b[2m|"
                        } else {
                            "Claude"
                        }),
                        "{frame}"
                    );
                }
            }
            state.mode = Mode::Select;
            state.query = "nomatch".into();
            let empty = state.terminal_frame(width, 15);
            assert!(
                empty.contains("No matching panes") && !empty.contains("Herdr Crossroads"),
                "{empty}"
            );
            state.query.clear();
            state.selected = Some("p-other".into());
            let short = state.terminal_frame(width, 5);
            let lines: Vec<_> = short.split("\r\n").collect();
            assert_eq!(lines.len(), 5, "{short}");
            assert!(
                lines[0].starts_with("  Search:") && lines[1].starts_with("  ─"),
                "{short}"
            );
            assert!(
                lines.iter().any(|line| selected_prefix(line)
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{short}"
            );
            assert!(!short.contains('┄'), "{short}");
        }
    }

    #[test]
    fn select_footer_uses_dot_groups_without_right_padding() {
        use ratatui::buffer::CellWidth;
        for width in [60, 76, 137, 138, 139, 150] {
            for height in [5, 15] {
                let frame = ui().terminal_frame(width, height);
                let lines: Vec<_> = frame.split("\r\n").collect();
                let last = lines[height - 1];
                let expected = if width >= 138 {
                    "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit"
                } else {
                    assert_eq!(
                        lines[height - 2],
                        "  j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace",
                        "{frame}"
                    );
                    "Enter: focus · Esc/q: exit"
                };
                assert_eq!(last, format!("  {expected} · ?: keys"), "{frame}");
                assert!((last.cell_width() as usize) < width, "{frame}");
                assert!(!last.contains('|'), "{frame}");
            }
        }
    }

    #[test]
    fn roomy_header_rule_is_dim_unselectable_and_respects_viewport_and_filter() {
        let mut state = ui();
        let frame = state.terminal_frame(150, 45);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 45);
        assert!(lines[2].contains("Status"), "{frame}");
        assert_eq!(
            lines[3],
            format!("  \x1b[2m{}\x1b[22m", "┄".repeat(146)),
            "{frame}"
        );
        assert!(
            selected_prefix(lines[4]) && lines[4].contains("Claude"),
            "{frame}"
        );
        assert!(!lines[3].contains("\x1b[48;5;236m"), "{frame}");
        state.key(Key::Down);
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-a"));

        let mut state = ui();
        let short = state.terminal_frame(150, 5);
        assert!(!short.contains("Status") && !short.contains('┄'), "{short}");
        state.query = "codex".into();
        let filtered = state.terminal_frame(150, 12);
        let lines: Vec<_> = filtered.split("\r\n").collect();
        assert!(
            lines[3].contains('┄') && lines[4].contains("codex"),
            "{filtered}"
        );
        assert_eq!(filtered.matches('┄').count(), 146, "{filtered}");
        state.query = "no matching pane".into();
        let empty = state.terminal_frame(150, 12);
        assert!(
            empty.contains("No matching panes") && !empty.contains('┄'),
            "{empty}"
        );

        let mut state = ui();
        state.key(Key::Down);
        let one_pane = state.terminal_frame(150, 8);
        let lines: Vec<_> = one_pane.split("\r\n").collect();
        assert!(lines[3].contains('┄'), "{one_pane}");
        assert!(
            selected_prefix(lines[5]) && lines[5].contains("Same"),
            "{one_pane}"
        );
        assert!(one_pane.contains("Claude"), "{one_pane}");

        let mut state = ui();
        state.terminal_frame(150, 7);
        for _ in 0..4 {
            state.key(Key::Down);
            state.terminal_frame(150, 7);
        }
        let scrolled = state.terminal_frame(150, 7);
        assert!(
            scrolled
                .lines()
                .any(|line| selected_prefix(line) && line.contains("pane 1")),
            "{scrolled}"
        );
        assert!(!scrolled.contains('┄'), "{scrolled}");
    }

    #[test]
    fn compact_and_roomy_footers_show_navigation_with_final_help_group() {
        use ratatui::buffer::CellWidth;
        for width in [60, 76, 137, 138, 139, 150] {
            let state = ui();
            let frame = state.terminal_frame(width, 15);
            let footer = frame.split("\r\n").last().unwrap();
            let navigation = if width >= 138 {
                "j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace · Enter: focus · Esc/q: exit"
            } else {
                assert_eq!(
                    frame.split("\r\n").nth(13),
                    Some("  j/k: ↕pane · h/l: ↕tab · Tab/⇧Tab: ↕workspace"),
                    "{frame}"
                );
                "Enter: focus · Esc/q: exit"
            };
            assert_eq!(
                footer.matches('·').count(),
                if width >= 138 { 5 } else { 2 },
                "{frame}"
            );
            assert!(!footer.contains('|'), "{frame}");
            assert!(footer.starts_with(&format!("  {navigation}")), "{frame}");
            assert_eq!(
                footer.matches('↕').count(),
                usize::from(width >= 138) * 3,
                "{frame}"
            );
            assert!(!footer.contains('↑') && !footer.contains('↓'), "{frame}");
            assert_eq!(footer, format!("  {navigation} · ?: keys"), "{frame}");
            assert!((footer.cell_width() as usize) < width, "{frame}");
            assert_eq!(frame.split("\r\n").count(), 15, "{frame}");
        }
    }

    #[test]
    fn compact_status_shares_name_column_and_reclaims_workspace_width() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][1]["label"] =
            Value::from("Long workspace feature branch name");
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        let compact = state.frame_width(60);
        let header = compact.lines().nth(2).unwrap();
        assert!(
            header.contains("Workspace") && header.contains("Agent/Pane"),
            "{compact}"
        );
        assert!(!header.contains("Status"), "{compact}");
        assert!(
            compact
                .lines()
                .any(|line| line.contains("Long workspace") && line.contains("C | working")),
            "{compact}"
        );
        assert!(
            compact.lines().any(|line| line.contains("co… | idle")),
            "{compact}"
        );
        assert!(
            compact
                .lines()
                .any(|line| line.contains("pane 3") && !line.contains('|')),
            "{compact}"
        );
        let wide = state.frame_width(150);
        assert!(wide.lines().nth(2).unwrap().contains("Status"), "{wide}");
        assert!(
            wide.contains("Long workspace feature branch name"),
            "{wide}"
        );
        assert!(!wide.contains("Claude | working"), "{wide}");
    }

    #[test]
    fn roomy_columns_allocate_workspace_tab_name_and_status_without_overflow() {
        use ratatui::buffer::CellWidth;
        for (width, expected) in [(150, [4, 65, 104, 134]), (139, [4, 60, 96, 123])] {
            let state = ui();
            let frame = state.terminal_frame(width, 15);
            let lines: Vec<_> = frame.split("\r\n").collect();
            let header = lines[2];
            let positions = ["Workspace", "Tab", "Agent/Pane Name", "Status"]
                .map(|label| header.find(label).expect("roomy column header"));
            assert_eq!(positions, expected, "{frame}");
            let selected = without_status_ansi(lines[4])
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "");
            assert!(selected.cell_width() as usize <= width - 1, "{frame}");
            assert!(selected[expected[2]..].contains("Claude"), "{frame}");
        }
    }

    #[test]
    fn inline_status_survives_search_name_expansion_and_truncation_without_escape_injection() {
        let mut state = ui();
        state.query = "suffix".into();
        let frame = state.frame_width(60);
        assert!(frame.contains("co… | idle"), "{frame}");
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["agents"][0]["agent_status"] =
            Value::from("working\u{1b}[31m");
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        let frame = state.terminal_frame(60, 15);
        assert!(!frame.contains("\u{1b}[31m"), "{frame}");
        assert!(!frame.contains("\u{1b}[38;2;249;226;175m"), "{frame}");
    }

    #[test]
    fn status_rgb_only_on_status_and_selected_background_survives_fg_reset() {
        for width in [60, 150] {
            let state = ui();
            let frame = state.terminal_frame(width, 15);
            let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
            assert!(selected_prefix(selected), "{selected}");
            assert!(
                selected.contains("\u{1b}[38;2;249;226;175mworking\u{1b}[39m"),
                "{selected}"
            );
            assert!(selected.ends_with("\u{1b}[0m"), "{selected}");
            assert!(
                !selected.contains("\u{1b}[0mworking")
                    && !selected.contains("\u{1b}[38;2;249;226;175mClaude"),
                "{selected}"
            );
            let idle = frame.lines().find(|line| line.contains("idle")).unwrap();
            assert!(
                idle.contains("\u{1b}[38;2;166;227;161midle\u{1b}[39m"),
                "{idle}"
            );
            assert!(!state.frame_width(width).contains('\u{1b}'));
        }
    }

    #[test]
    fn compact_pipe_is_dim_only_between_unstyled_spaces() {
        let mut state = ui();
        let frame = state.terminal_frame(60, 15);
        let selected = frame.lines().find(|line| selected_prefix(line)).unwrap();
        assert!(selected.starts_with("  \x1b[48;5;236m> "), "{selected}");
        assert!(
            selected.contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking\x1b[39m"),
            "{selected}"
        );
        assert!(selected.ends_with("\x1b[0m"), "{selected}");
        let idle = frame.lines().find(|line| line.contains("idle")).unwrap();
        assert!(
            idle.contains("co… \x1b[2m|\x1b[22m \x1b[38;2;166;227;161midle\x1b[39m"),
            "{idle}"
        );
        state.key(Key::Char('/'));
        state.query = "Claude".into();
        let filtered = state.terminal_frame(60, 15);
        assert_eq!(filtered.lines().next().unwrap(), "  Search: Claude");
        assert!(filtered.contains("C \x1b[2m|\x1b[22m \x1b[38;2;249;226;175mworking"));
        let wide = state.terminal_frame(150, 15);
        assert!(wide.lines().nth(2).unwrap().contains("Status"), "{wide}");
        let wide_selected = wide
            .lines()
            .find(|line| selected_prefix(line) && line.contains("Claude"))
            .unwrap();
        assert!(
            !wide_selected.contains("\x1b[2m") && !wide_selected.contains('|'),
            "{wide_selected}"
        );
        assert!(
            wide_selected.contains("\x1b[38;2;249;226;175mworking\x1b[39m"),
            "{wide_selected}"
        );
    }

    #[test]
    fn compact_selected_background_matches_horizontal_table_borders() {
        for (width, height) in [
            (60, 15),
            (76, 15),
            (40, 15),
            (137, 15),
            (138, 15),
            (139, 15),
            (150, 15),
            (60, 5),
            (40, 5),
            (138, 5),
        ] {
            let mut state = ui();
            if height == 5 {
                state.selected = Some("p-other".into());
            }
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height, "{frame}");
            let selected = lines.iter().find(|line| selected_prefix(line)).unwrap();
            let border: Vec<_> = lines[1]
                .chars()
                .enumerate()
                .filter_map(|(x, c)| (c == '─').then_some(x))
                .collect();
            assert!(!border.is_empty(), "{frame}");
            assert_eq!(border, (2..width - 2).collect::<Vec<_>>(), "{frame}");
            if height > 6 {
                let bottom: Vec<_> = lines[height - if width >= 138 { 2 } else { 3 }]
                    .chars()
                    .enumerate()
                    .filter_map(|(x, c)| (c == '─').then_some(x))
                    .collect();
                assert_eq!(border, bottom, "{frame}");
            }
            assert!(selected.starts_with("  \x1b[48;5;236m> "), "{selected:?}");
            assert!(selected.ends_with("\x1b[0m"), "{selected:?}");
            let painted = selected.strip_prefix("  \x1b[48;5;236m").unwrap();
            let painted = painted.strip_suffix("\x1b[0m").unwrap();
            let painted = without_status_ansi(painted)
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "");
            assert_eq!(
                (2..2 + painted.cell_width() as usize).collect::<Vec<_>>(),
                border,
                "{selected:?}"
            );
            let plain = without_status_ansi(selected)
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "")
                .replace("\x1b[0m", "");
            assert_eq!(
                plain.cell_width() as usize,
                2 + border.len(),
                "{selected:?}"
            );
            assert!(plain.starts_with("  > "), "{selected:?}");
            assert!(
                painted.ends_with(' '),
                "last painted cell should be blank: {selected:?}"
            );
            assert!(
                lines.iter().all(|line| {
                    without_status_ansi(line)
                        .replace("\x1b[48;5;236m", "")
                        .replace("\x1b[2m", "")
                        .replace("\x1b[22m", "")
                        .replace("\x1b[0m", "")
                        .cell_width() as usize
                        <= width - 1
                }),
                "phantom terminal column: {frame}"
            );
            if height == 15 {
                let idle = lines.iter().find(|line| line.contains("idle")).unwrap();
                assert!(!idle.contains("\x1b[48;5;236m"), "{idle:?}");
                assert!(!without_status_ansi(idle).ends_with(' '), "{idle:?}");
                assert!(
                    lines.iter().any(|line| line.starts_with("  \x1b[2m┄")),
                    "{frame}"
                );
            }
        }
    }

    #[test]
    fn all_status_defaults_and_partial_override_are_read_once() {
        let colors = StatusColors::default();
        for (status, expected) in [
            ("working", Color::Rgb(249, 226, 175)),
            ("blocked", Color::Rgb(243, 139, 168)),
            ("done", Color::Rgb(148, 226, 213)),
            ("idle", Color::Rgb(166, 227, 161)),
            ("unknown", Color::Rgb(108, 112, 134)),
            ("unrecognized", Color::Rgb(108, 112, 134)),
        ] {
            assert_eq!(colors.for_status(status), expected);
        }
        let dir = std::path::Path::new("/tmp/opencode")
            .join(format!("crossroads-palette-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("status-colors.json");
        assert_eq!(
            StatusColors::load(Some(&dir)).unwrap().working,
            colors.working
        );
        std::fs::write(&path, r##"{"working":"#123abc","unknown":"#010203"}"##).unwrap();
        let loaded = StatusColors::load(Some(&dir)).unwrap();
        assert_eq!(loaded.working, Color::Rgb(18, 58, 188));
        assert_eq!(loaded.unknown, Color::Rgb(1, 2, 3));
        assert_eq!(loaded.idle, colors.idle);
        let mut state = ui();
        state.status_colors = loaded;
        std::fs::write(&path, r##"{"working":"#FFFFFF"}"##).unwrap();
        for width in [60, 150] {
            let rendered = state.terminal_frame(width, 15);
            assert!(
                rendered.contains("\x1b[38;2;18;58;188mworking\x1b[39m"),
                "{rendered}"
            );
            assert!(!rendered.contains("\x1b[38;2;255;255;255m"), "{rendered}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn status_palette_renders_all_known_and_unknown_values_in_both_layouts() {
        for (status, rgb) in [
            ("working", "249;226;175"),
            ("blocked", "243;139;168"),
            ("done", "148;226;213"),
            ("idle", "166;227;161"),
            ("other", "108;112;134"),
        ] {
            let mut state = ui();
            state.rows[0].status = status.into();
            for width in [60, 150] {
                let frame = state.terminal_frame(width, 15);
                assert!(
                    frame.contains(&format!("\x1b[38;2;{rgb}m{status}\x1b[39m")),
                    "{frame}"
                );
            }
        }
    }

    #[test]
    fn workspace_separators_are_single_dim_rows_only_between_visible_blocks() {
        for (width, height) in [(60, 15), (150, 45)] {
            let mut state = ui();
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            let separators: Vec<_> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains('┄'))
                .collect();
            assert_eq!(
                separators.len(),
                if width == 150 { 2 } else { 1 },
                "{frame}"
            );
            let (at, line) = separators[separators.len() - 1];
            assert!(
                line.starts_with("  \x1b[2m┄") && line.ends_with("\x1b[22m"),
                "{frame}"
            );
            assert!(
                lines[at - 1].contains("pane 1")
                    && lines[at + 1].contains(if width == 60 { "idle" } else { "codex" }),
                "{frame}"
            );
            assert!(!frame.contains('▏') && !frame.contains('│'), "{frame}");
            assert!(
                lines[1].starts_with("  ─")
                    && lines[height - if width == 150 { 2 } else { 3 }].starts_with("  ─"),
                "{frame}"
            );
            state.query = "codex".into();
            let only = state.terminal_frame(width, height);
            assert_eq!(
                only.lines().filter(|line| line.contains('┄')).count(),
                usize::from(width == 150),
                "{only}"
            );
            assert!(!only.contains('▏'), "{only}");
            assert!(
                only.lines().any(|line| line.contains("Same")
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{only}"
            );
        }
    }

    #[test]
    fn workspace_separator_consumes_viewport_height_without_hiding_selection() {
        for width in [60, 150] {
            let mut state = ui();
            state.terminal_frame(width, 10);
            for _ in 0..5 {
                state.key(Key::Down);
                state.terminal_frame(width, 10);
            }
            let frame = state.terminal_frame(width, 10);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), 10, "{frame}");
            assert!(
                lines.iter().any(|line| selected_prefix(line)
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{frame}"
            );
            assert_eq!(
                frame.lines().filter(|line| line.contains('┄')).count(),
                if width == 150 { 2 } else { 1 },
                "{frame}"
            );
            assert_eq!(state.selected.as_deref(), Some("p-other"));
            let short = state.terminal_frame(width, 5);
            assert_eq!(short.split("\r\n").count(), 5, "{short}");
            assert!(
                short.lines().any(|line| selected_prefix(line)
                    && line.contains("Same")
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{short}"
            );
            assert!(!short.contains('┄'), "{short}");
            state.query = "codex".into();
            let filtered = state.terminal_frame(width, 5);
            assert!(
                !filtered.contains('┄')
                    && filtered.contains(if width == 60 { "idle" } else { "codex" }),
                "{filtered}"
            );
        }
    }

    #[test]
    fn tab_bytes_navigate_visible_workspaces_without_focusing() {
        let mut keys = Keys::new();
        let mut state = ui();
        let tab = keys.feed(b'\t').unwrap();
        assert!(state.key(tab).is_none());
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.key(Key::Down);
        assert!(keys.feed(27).is_none());
        assert!(keys.feed(b'[').is_none());
        let back = keys.feed(b'Z').expect("Shift+Tab must parse");
        assert!(state.key(back).is_none());
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.query = "codex".into();
        state.key(tab);
        state.key(back);
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.key(Key::Char('/'));
        state.key(tab);
        state.key(back);
        assert_eq!(state.mode, Mode::Search);
        assert_eq!(state.query, "codex");
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.key(Key::Esc);
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
        assert!(keys.feed(27).is_none());
        assert!(matches!(keys.flush(), Some(Key::Esc)));
    }

    #[test]
    fn workspace_jumps_skip_hidden_blocks_and_stop_at_both_ends() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        let data = &mut snapshot["result"]["snapshot"];
        data["workspaces"][0]["label"] = Value::from("Middle");
        data["tabs"][0]["label"] = Value::from("Middle");
        data["workspaces"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"workspace_id":"w3","number":3,"label":"Same Third"}));
        data["tabs"].as_array_mut().unwrap().push(
            serde_json::json!({"tab_id":"t4","workspace_id":"w3","number":1,"label":"Third tab"}),
        );
        data["panes"].as_array_mut().unwrap().push(
            serde_json::json!({"pane_id":"p-third","workspace_id":"w3","tab_id":"t4","label":"Third pane"}),
        );
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 150).unwrap());
        state.key(Key::BackTab);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        state.key(Key::Tab);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.key(Key::Tab);
        assert_eq!(state.selected.as_deref(), Some("p-third"));
        state.key(Key::Tab);
        assert_eq!(state.selected.as_deref(), Some("p-third"));
        state.query = "Third".into();
        state.key(Key::BackTab);
        assert_eq!(state.selected.as_deref(), Some("p-third"));
        state.query = "Same".into();
        state.key(Key::BackTab);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        state.query.clear();
        state.key(Key::BackTab);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        state.key(Key::Char('?'));
        state.key(Key::Tab);
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        let help = state.terminal_frame(60, 15);
        assert!(help.contains("Tab/Shift+Tab: workspace"), "{help}");
        state.key(Key::Esc);
        let frame = state.terminal_frame(60, 15);
        assert_eq!(
            frame.lines().filter(|line| line.contains('┄')).count(),
            1,
            "{frame}"
        );
    }

    #[test]
    fn outer_border_is_horizontal_and_workspace_rows_have_no_margin_marker() {
        use ratatui::buffer::CellWidth;
        for (width, height) in [(60, 15), (150, 45)] {
            let mut state = ui();
            for height in [5, height] {
                let frame = state.terminal_frame(width, height);
                let lines: Vec<_> = frame.split("\r\n").collect();
                assert_eq!(lines.len(), height, "{frame}");
                assert!(
                    lines.iter().all(|line| without_status_ansi(line)
                        .replace("\x1b[48;5;236m", "")
                        .replace("\x1b[0m", "")
                        .replace("\x1b[2m", "")
                        .replace("\x1b[22m", "")
                        .cell_width()
                        < width as u16),
                    "{frame}"
                );
                assert!(lines[1].starts_with("  ─"), "{frame}");
                if height > 6 {
                    assert!(
                        lines[height - if width == 150 { 2 } else { 3 }].starts_with("  ─"),
                        "{frame}"
                    );
                }
                assert!(!frame.contains('│') && !frame.contains('├'), "{frame}");
                assert!(
                    selected_prefix(
                        lines[if height <= 6 {
                            2
                        } else if width == 150 {
                            4
                        } else {
                            3
                        }]
                    ),
                    "{frame}"
                );
                if height > 6 {
                    let codex = lines
                        .iter()
                        .position(|line| line.contains(if width == 60 { "idle" } else { "codex" }))
                        .unwrap();
                    assert!(lines[codex].starts_with("    "), "{frame}");
                    assert!(lines[codex].contains("Same"), "{frame}");
                    assert_eq!(
                        frame.lines().filter(|line| line.contains('┄')).count(),
                        if width == 150 { 2 } else { 1 },
                        "{frame}"
                    );
                } else {
                    assert!(!frame.contains('┄'), "{frame}");
                }
                assert!(!frame.contains('▏'), "{frame}");
            }
            state.query = "codex".into();
            let only = state.terminal_frame(width, height);
            assert!(!only.contains('▏'), "{only}");
            assert_eq!(
                only.lines().filter(|line| line.contains('┄')).count(),
                usize::from(width == 150),
                "{only}"
            );
            assert!(
                only.lines().any(|line| selected_prefix(line)
                    && line.contains("Same")
                    && line.contains(if width == 60 { "idle" } else { "codex" })
                    && line.ends_with("\x1b[0m")),
                "{only}"
            );
        }
    }

    #[test]
    fn roomy_workspace_column_shows_long_label_without_changing_compact_column() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        let label = "Long workspace feature branch name";
        snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from(label);
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        let wide = state.terminal_frame(150, 16);
        assert!(
            wide.lines()
                .any(|line| line.contains(label) && line.contains("Claude")),
            "{wide}"
        );
        let compact = state.terminal_frame(60, 16);
        assert!(!compact.contains(label), "{compact}");
        assert!(
            compact
                .lines()
                .any(|line| line.contains("Long workspace feature br…")
                    && line.contains("C \x1b[2m|")
                    && line.contains("working")),
            "{compact}"
        );
    }

    #[test]
    fn compact_workspace_blocks_have_one_separator_and_preserve_navigation() {
        let mut state = ui();
        let frame = state.frame_width(60);
        assert_eq!(
            frame.lines().filter(|line| line.contains('┄')).count(),
            1,
            "{frame}"
        );
        let lines: Vec<_> = frame.lines().collect();
        let first = lines
            .iter()
            .position(|line| line.contains("Second") && line.contains("pane 1"))
            .unwrap();
        let next = lines.iter().position(|line| line.contains("idle")).unwrap();
        assert_eq!(next, first + 2, "{frame}");
        assert!(lines[first + 1].starts_with("  ┄"), "{frame}");
        assert_eq!(lines.len(), 14, "{frame}");
        assert!(lines[next].starts_with("    "), "{frame}");
        assert!(
            lines[first].starts_with("  ") && !lines[first].contains('┄'),
            "{frame}"
        );
        assert!(lines[3].starts_with("  > "), "{frame}");

        let compact = state.terminal_frame(60, 15);
        let lines: Vec<_> = compact.split("\r\n").collect();
        assert_eq!(lines.len(), 15, "{compact}");
        assert_eq!(
            compact.lines().filter(|line| line.contains('┄')).count(),
            1,
            "{compact}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("    ") && line.contains("idle")),
            "{compact}"
        );
        state.query = "pane".into();
        let filtered = state.terminal_frame(60, 15);
        let lines: Vec<_> = filtered.split("\r\n").collect();
        let first = lines
            .iter()
            .position(|line| line.contains("pane 1"))
            .unwrap();
        let next = lines.iter().position(|line| line.contains("idle")).unwrap();
        assert_eq!(next, first + 2, "{filtered}");
        assert_eq!(
            filtered.lines().filter(|line| line.contains('┄')).count(),
            1,
            "{filtered}"
        );
        state.query = "codex".into();
        let only = state.terminal_frame(60, 15);
        assert!(!only.contains('┄') && !only.contains('▏'), "{only}");
        assert!(
            only.lines()
                .any(|line| selected_prefix(line) && line.contains("Same")),
            "{only}"
        );
        state.query.clear();

        state.key(Key::Right);
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        let scrolled = state.terminal_frame(60, 10);
        assert_eq!(scrolled.split("\r\n").count(), 10, "{scrolled}");
        assert!(!scrolled.contains('▏'), "{scrolled}");
        assert!(
            scrolled.lines().any(|line| selected_prefix(line)
                && line.contains("idle")
                && line.ends_with("\x1b[0m")),
            "{scrolled}"
        );
        state.key(Key::Up);
        let previous = state.terminal_frame(60, 10);
        assert!(
            previous
                .lines()
                .any(|line| selected_prefix(line) && line.contains("pane 1")),
            "{previous}"
        );
        state.key(Key::Down);
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    }

    #[test]
    fn roomy_workspace_separator_is_one_physical_row() {
        use ratatui::buffer::CellWidth;
        let mut state = ui();
        let frame = state.terminal_frame(150, 20);
        let lines: Vec<_> = frame.split("\r\n").collect();
        let first = lines
            .iter()
            .position(|line| line.contains("Second") && line.contains("pane 1"))
            .unwrap();
        let second = lines
            .iter()
            .position(|line| line.contains("codex"))
            .unwrap();
        assert_eq!(second, first + 2, "{frame}");
        assert!(
            lines[second].starts_with("    ") && lines[second].contains("codex"),
            "{frame}"
        );
        assert!(lines[first + 1].starts_with("  \x1b[2m┄"), "{frame}");
        assert_eq!(
            frame.lines().filter(|line| line.contains('┄')).count(),
            2,
            "{frame}"
        );
        assert!(!lines[first].contains('┄'), "{frame}");
        assert!(lines[4].starts_with("  \x1b[48;5;236m> "), "{frame}");
        for line in &lines {
            let plain = without_status_ansi(line)
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "");
            assert!(plain.cell_width() < 150, "phantom column: {line:?}");
        }
        let plain = state.frame_width(150);
        assert_eq!(plain.lines().count(), 14, "{plain}");
        assert!(!plain.contains('\x1b'), "{plain}");
        assert!(
            plain
                .lines()
                .any(|line| line.starts_with("    ") && line.contains("codex")),
            "{plain}"
        );
        state.query = "pane".into();
        let filtered = state.terminal_frame(150, 20);
        let lines: Vec<_> = filtered.split("\r\n").collect();
        let first = lines
            .iter()
            .position(|line| line.contains("pane 1"))
            .unwrap();
        let second = lines
            .iter()
            .position(|line| line.contains("codex"))
            .unwrap();
        assert_eq!(second, first + 2, "{filtered}");
        assert!(lines[first + 1].starts_with("  \x1b[2m┄"), "{filtered}");
        state.query = "codex".into();
        let only = state.frame_width(150);
        assert!(!only.contains('▏'), "{only}");
        assert_eq!(
            only.lines().filter(|line| line.contains('┄')).count(),
            1,
            "{only}"
        );
        assert!(
            only.lines()
                .any(|line| line.contains("Same") && line.contains("codex")),
            "{only}"
        );
        state.query.clear();
        state.key(Key::Right);
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
    }

    #[test]
    fn scrolling_across_roomy_workspace_border_keeps_selected_pane_visible() {
        let mut state = ui();
        let width = 150;
        let height = 10;
        state.terminal_frame(width, height);
        for _ in 0..5 {
            state.key(Key::Down);
            state.terminal_frame(width, height);
        }
        let frame = state.terminal_frame(width, height);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), height, "{frame}");
        let selected = lines
            .iter()
            .position(|line| selected_prefix(line) && line.contains("codex"))
            .unwrap();
        assert!(selected > 4, "{frame}");
        assert!(lines[selected - 1].contains('┄'), "{frame}");
        assert!(lines[selected - 2].contains("pane 1"), "{frame}");
        assert!(lines[selected].starts_with("  \x1b[48;5;236m> "), "{frame}");
        assert!(lines[selected].ends_with("\x1b[0m"), "{frame}");
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        let short = state.terminal_frame(width, 5);
        let lines: Vec<_> = short.split("\r\n").collect();
        assert_eq!(lines.len(), 5, "{short}");
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains("Same")
                && line.contains("codex")),
            "{short}"
        );
        state.key(Key::Up);
        let back = state.terminal_frame(width, height);
        assert!(
            back.lines()
                .any(|line| line.starts_with("  \x1b[48;5;236m> ")
                    && line.contains("Second")
                    && line.contains("pane 1")),
            "{back}"
        );
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-second-tab"));
    }

    #[test]
    fn viewport_start_inside_workspace_repeats_context_without_selecting_it() {
        for (width, height) in [(60, 15), (150, 45)] {
            let mut state = ui();
            state.selected = Some("p-untitled".into());
            state.viewport_start.set(1);
            state.last_selected_at.set(Some(2));
            let frame = state.terminal_frame(width, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), height, "{frame}");
            assert_eq!(state.viewport_start.get(), 1, "{frame}");
            assert!(
                lines[if width == 150 { 4 } else { 3 }].starts_with("    Same")
                    && lines[if width == 150 { 4 } else { 3 }].contains("Same"),
                "{frame}"
            );
            assert!(
                selected_prefix(lines[if width == 150 { 5 } else { 4 }])
                    && lines[if width == 150 { 5 } else { 4 }].contains("pane 3"),
                "{frame}"
            );
            assert_eq!(
                frame.lines().filter(|line| line.contains('┄')).count(),
                if width == 150 { 2 } else { 1 },
                "{frame}"
            );
            assert!(!frame.contains('▏'), "{frame}");
        }
    }

    #[test]
    fn wide_workspace_glyph_does_not_add_a_phantom_terminal_column() {
        use ratatui::buffer::CellWidth;
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][1]["label"] =
            Value::from("↳ 界 (feature/demo)");
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        let frame = state.terminal_frame(76, 12);
        assert!(
            frame.contains("↳ 界 ("),
            "wide glyph acquired an extra cell: {frame}"
        );
        for line in frame.split("\r\n") {
            let plain = without_status_ansi(line)
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "");
            assert!(plain.cell_width() < 76, "phantom column: {plain:?}");
        }
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
        data["tabs"].as_array_mut().unwrap().push(
            serde_json::json!({"tab_id":"t4","workspace_id":"w3","number":1,"label":"Other"}),
        );
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
            let mut state = Ui::new(items);
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
    fn ordinary_workspace_label_with_link_marker_keeps_normal_separator() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][0]["label"] = Value::from("↳ Ordinary");
        let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 150).unwrap());
        for width in [60, 150] {
            for frame in [state.frame_width(width), state.terminal_frame(width, 20)] {
                let (dim, reset) = if frame.contains('\x1b') {
                    ("\x1b[2m", "\x1b[22m")
                } else {
                    ("", "")
                };
                let expected = format!("  {dim}{}{reset}", "┄".repeat(width - 4));
                let rule = frame
                    .lines()
                    .filter(|line| line.contains('┄'))
                    .last()
                    .unwrap();
                assert_eq!(rule, expected, "{frame}");
                assert!(frame.contains("↳ Ordinary"), "{frame}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn duplicate_linked_entries_mark_workspace_once_and_indent_its_separator() {
        use ratatui::buffer::CellWidth;
        use std::os::unix::fs::PermissionsExt;
        let fake = std::path::Path::new("/tmp/opencode")
            .join(format!("crossroads-duplicate-{}", std::process::id()));
        let response = serde_json::json!({"result":{"type":"worktree_list","worktrees":[
            {"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"},
            {"open_workspace_id":"w2","is_linked_worktree":true,"branch":"feature/demo"}
        ]}});
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", response)).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut items = rows(SNAPSHOT, Some("p-self"), 120).unwrap();
        annotate_worktrees(&mut items, fake.as_os_str(), false);
        std::fs::remove_file(fake).unwrap();

        for row in items.iter().filter(|row| row.workspace_id == "w2") {
            assert_eq!(row.workspace, "↳ Same (feature/demo)");
        }
        let mut state = Ui::new(items);
        for width in [60, 150] {
            let rule_width = width - 4;
            for frame in [state.frame_width(width), state.terminal_frame(width, 20)] {
                let (dim, reset) = if frame.contains('\x1b') {
                    ("\x1b[2m", "\x1b[22m")
                } else {
                    ("", "")
                };
                let expected = format!("      {dim}{}{reset}", "┄".repeat(rule_width - 4));
                // The roomy header rule is unchanged; only the rule before the linked row moves.
                let separators: Vec<_> = frame.lines().filter(|line| line.contains('┄')).collect();
                let linked_rule = separators.last().unwrap();
                let linked_row = frame.lines().find(|line| line.contains("↳ Same")).unwrap();
                let arrow_col = linked_row
                    .split_once('↳')
                    .unwrap()
                    .0
                    .replace("\x1b[48;5;236m", "")
                    .cell_width();
                let rule_col = linked_rule
                    .split_once('┄')
                    .unwrap()
                    .0
                    .replace(dim, "")
                    .cell_width();
                assert_eq!(rule_col, arrow_col + "↳ ".cell_width(), "{frame}");
                assert_eq!(
                    linked_rule.replace(dim, "").replace(reset, "").cell_width(),
                    (width - 2) as u16,
                    "{frame}"
                );
                assert!(!linked_rule.contains('↳'), "{frame}");
                assert_eq!(*linked_rule, expected, "{frame}");
                if width == 150 {
                    assert_eq!(
                        separators[0],
                        format!("  {dim}{}{reset}", "┄".repeat(rule_width)),
                        "{frame}"
                    );
                }
                assert_eq!(separators.len(), if width == 150 { 2 } else { 1 });
                assert_eq!(frame.matches("↳ Same").count(), 1, "{frame}");
                assert!(
                    frame.lines().any(|line| line.contains("↳ Same")
                        && line.contains(if width == 60 { "idle" } else { "codex" })),
                    "{frame}"
                );
            }
            state.query = "pane".into();
            let filtered = state.terminal_frame(width, 20);
            assert!(
                filtered
                    .lines()
                    .any(|line| line.starts_with("      \x1b[2m┄")),
                "{filtered}"
            );
            state.query = "codex".into();
            let only = state.terminal_frame(width, 20);
            assert_eq!(
                only.lines().filter(|line| line.contains('┄')).count(),
                usize::from(width == 150),
                "{only}"
            );
            state.query.clear();
            state.key(Key::Tab);
            assert_eq!(state.selected.as_deref(), Some("p-other"));
            let scrolled = state.terminal_frame(width, 10);
            assert!(
                scrolled
                    .lines()
                    .any(|line| line.starts_with("      \x1b[2m┄")),
                "{scrolled}"
            );
            assert!(
                scrolled.lines().any(|line| selected_prefix(line)
                    && line.contains(if width == 60 { "idle" } else { "codex" })),
                "{scrolled}"
            );
            assert_eq!(state.key(Key::Enter).as_deref(), Some("p-other"));
        }
    }

    #[test]
    fn four_columns_deduplicate_context_and_repeat_it_at_viewport_and_filter_start() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][0]["label"] = Value::from("Studio");
        snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from("Workshop");
        snapshot["result"]["snapshot"]["tabs"][0]["label"] = Value::from("Ops");
        snapshot["result"]["snapshot"]["tabs"][1]["label"] = Value::from("Backend");
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        for width in [76, 140] {
            let frame = state.terminal_frame(width, 16);
            assert!(
                frame.contains("Workspace")
                    && frame.contains("Tab")
                    && frame.contains("Agent/Pane Name")
                    && frame.lines().nth(2).unwrap().contains("Status") == (width == 140),
                "{frame}"
            );
            assert!(
                !frame.contains("Pane ID")
                    && !frame.contains("p-b")
                    && !frame.contains("p-a")
                    && !frame.contains("W1/T1")
                    && !frame.contains("popup"),
                "{frame}"
            );
            assert_eq!(frame.matches("Workshop").count(), 1, "{frame}");
            assert_eq!(frame.matches("Backend").count(), 1, "{frame}");
            assert_eq!(frame.matches("Studio").count(), 1, "{frame}");
            assert!(
                frame.lines().any(|line| line.contains("Workshop")
                    && line.contains("Backend")
                    && line.contains("Claude")
                    && line.contains("working")),
                "{frame}"
            );
            assert!(
                frame.lines().any(|line| line.contains("Same")
                    && !line.contains("working")
                    && !line.contains("Backend")),
                "{frame}"
            );
            assert!(
                frame.lines().all(|line| without_status_ansi(line)
                    .replace("\x1b[48;5;236m", "")
                    .replace("\x1b[0m", "")
                    .replace("\x1b[2m", "")
                    .replace("\x1b[22m", "")
                    .chars()
                    .count()
                    < width),
                "{frame}"
            );
            state.selected = Some("p-a".into());
            let scrolled = state.terminal_frame(width, 7);
            assert!(
                scrolled.lines().any(|line| line.contains("Workshop")
                    && line.contains("Backend")
                    && (line.contains("Claude") || line.contains("Same")))
                    && scrolled
                        .lines()
                        .any(|line| selected_prefix(line) && line.contains("Same")),
                "{scrolled}"
            );
            assert!(!scrolled.contains("p-a"), "{scrolled}");
            state.query = "pane 3".into();
            let filtered = state.terminal_frame(width, 16);
            assert!(
                filtered.lines().any(|line| line.contains("Workshop")
                    && line.contains("Backend")
                    && line.contains("pane 3")),
                "{filtered}"
            );
            assert!(
                !filtered.contains("Claude") && !filtered.contains("p-untitled"),
                "{filtered}"
            );
            state.query.clear();
            state.selected = Some("p-b".into());
            state.viewport_start.set(0);
            state.last_selected_at.set(None);
        }
    }

    #[test]
    fn duplicate_visible_names_still_focus_selected_stable_pane_id() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["panes"][3]["label"] = Value::from("Claude");
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 76).unwrap());
        let frame = state.terminal_frame(76, 12);
        assert_eq!(frame.matches("Claude").count(), 2, "{frame}");
        assert!(!frame.contains("p-b") && !frame.contains("p-a"), "{frame}");
        state.key(Key::Down);
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert_eq!(state.key(Key::Enter).as_deref(), Some("p-a"));
    }

    #[test]
    fn ratatui_table_has_headers_border_and_full_row_selection() {
        let state = ui();
        let frame = state.terminal_frame(76, 12);
        assert!(
            frame.contains("Workspace")
                && frame.contains("Tab")
                && frame.contains("Agent/Pane Name")
                && !frame.lines().nth(2).unwrap().contains("Status"),
            "{frame}"
        );
        assert!(frame.lines().nth(1).unwrap().starts_with("  ─"), "{frame}");
        assert!(frame.lines().nth(9).unwrap().starts_with("  ─"), "{frame}");
        assert!(
            frame.contains("\x1b[48;5;236m")
                && frame.lines().any(|line| selected_prefix(line)
                    && line.contains("Claude")
                    && line.ends_with("\x1b[0m")),
            "{frame}"
        );
        assert!(
            !frame.contains("p-b") && !frame.contains("Pane ID"),
            "{frame}"
        );
    }

    #[test]
    fn compact_layouts_keep_visible_context_and_inline_status() {
        let mut state = ui();
        for width in [76, 120] {
            let frame = state.terminal_frame(width, 20);
            assert!(
                frame.contains("Workspace")
                    && frame.contains("Tab")
                    && frame.contains("Agent/Pane Name")
                    && !frame.lines().nth(2).unwrap().contains("Status"),
                "{frame}"
            );
            assert!(
                frame.lines().any(|line| selected_prefix(line)
                    && line.contains("Claude")
                    && line.contains("working")),
                "{frame}"
            );
            assert!(
                frame
                    .lines()
                    .any(|line| line.contains("codex") && line.contains("idle")),
                "{frame}"
            );
            assert!(
                !frame.contains("p-b") && !frame.contains("W1/T1") && !frame.contains(" / T"),
                "{frame}"
            );
        }
        assert!(!state.frame_width(76).contains('\x1b'));
        state.query = "Second".into();
        let filtered = state.terminal_frame(120, 20);
        assert!(
            filtered
                .lines()
                .any(|line| line.contains("Second") && line.contains("pane 1")),
            "{filtered}"
        );
        assert!(
            !filtered.contains("Claude") && !filtered.contains("Backend"),
            "{filtered}"
        );
    }

    #[test]
    fn compact_status_uses_spare_columns_but_yields_to_name() {
        let state = ui();
        let roomy = state.terminal_frame(76, 20);
        assert!(
            roomy
                .lines()
                .any(|line| line.contains("Claude") && line.contains("working")),
            "{roomy}"
        );
        assert!(
            roomy
                .lines()
                .any(|line| line.contains("codex") && line.contains("idle")),
            "{roomy}"
        );
        assert!(
            roomy.lines().any(|line| line.contains("Same")
                && !line.contains("idle")
                && !line.contains("Claude")),
            "{roomy}"
        );
        let constrained = state.terminal_frame(29, 20);
        assert!(
            constrained.contains("Claude")
                && !constrained.contains("Status")
                && !constrained.contains("working"),
            "{constrained}"
        );
        assert!(!constrained.contains("p-b"), "{constrained}");
    }

    #[test]
    fn table_block_uses_nearly_full_width_without_moving_chrome() {
        for width in [40, 86, 96, 137, 138, 140] {
            let state = ui();
            let frame = state.terminal_frame(width, 16);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines[0], "  Search: / to search");
            assert!(
                lines[15].starts_with(if width >= 138 {
                    "  j/k: ↕pane"
                } else {
                    "  Enter: focus"
                }),
                "{frame}"
            );
            assert!(lines[1].starts_with("  ─"), "{frame}");
            assert!(frame.lines().any(|line| line.contains("Claude")), "{frame}");
            assert_eq!(lines[1].chars().count(), width - 2, "{frame}");
            assert!(
                lines[if width >= 138 { 14 } else { 13 }].starts_with("  ─"),
                "{frame}"
            );
        }
    }

    #[test]
    fn scrolled_context_stays_on_selected_row_in_short_view() {
        let mut state = ui();
        for _ in 0..5 {
            state.key(Key::Down);
        }
        for width in [86, 137, 138, 140] {
            let frame = state.terminal_frame(width, 5);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert_eq!(lines.len(), 5, "{frame}");
            assert!(
                lines[4].starts_with(if width >= 138 {
                    "  j/k: ↕pane"
                } else {
                    "  Enter: focus"
                }),
                "{frame}"
            );
            assert!(lines[1].starts_with("  ─"), "{frame}");
            assert!(
                lines.iter().any(|line| selected_prefix(line)
                    && line.ends_with("\x1b[0m")
                    && line.contains("codex")
                    && line.contains("Same")),
                "{frame}"
            );
            assert!(!frame.contains("p-other"), "{frame}");
        }
    }

    #[test]
    fn long_names_clip_within_wide_table_without_losing_status() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["agents"][0]["display_agent"] = Value::from("A".repeat(200));
        for width in [86, 140] {
            let state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), width).unwrap());
            let frame = state.terminal_frame(width, 8);
            let line = frame
                .split("\r\n")
                .find(|line| selected_prefix(line))
                .unwrap();
            let plain = without_status_ansi(line)
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "");
            assert!(plain.starts_with("  > "), "{frame}");
            assert!(plain.contains('A'), "{frame}");
            assert!(plain.contains('…'), "{frame}");
            assert!(
                plain.chars().count() < width && plain.contains("working"),
                "{frame}"
            );
            assert!(!plain.contains("p-b"), "{frame}");
        }
    }

    #[test]
    fn wide_scroll_repeats_context_on_selected_row_in_short_viewport() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        let panes = snapshot["result"]["snapshot"]["panes"]
            .as_array_mut()
            .unwrap();
        for n in 0..12 {
            panes.push(serde_json::json!({"pane_id": format!("p-extra-{n}"), "workspace_id": "w1", "tab_id": "t1", "label": "filler"}));
        }
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        state.selected = Some("p-extra-11".into());
        let frame = state.terminal_frame(120, 5);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert_eq!(lines.len(), 5, "{frame}");
        assert!(lines[1].starts_with("  ─"), "{frame}");
        assert!(
            lines.iter().any(|line| selected_prefix(line)
                && line.contains("Same")
                && line.contains("filler")),
            "{frame}"
        );
        assert_eq!(state.selected.as_deref(), Some("p-extra-11"));
        state.selected = Some("p-second-tab".into());
        let other = state.terminal_frame(120, 5);
        assert!(
            other
                .lines()
                .any(|line| line.contains("Second") && line.contains("pane 1")),
            "{other}"
        );
        assert!(
            !other.contains("filler") && !other.contains("p-second-tab"),
            "{other}"
        );
    }

    #[test]
    fn wide_scroll_shows_only_current_tab_context_without_group_rows() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["panes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"pane_id": "p-second-2", "workspace_id": "w1", "tab_id": "t3", "label": "Second pane"}));
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        for (id, height) in [("p-second-tab", 7), ("p-second-2", 8)] {
            state.selected = Some(id.into());
            let frame = state.terminal_frame(120, height);
            let lines: Vec<_> = frame.split("\r\n").collect();
            assert!(lines.len() <= height, "{frame}");
            assert!(lines[1].starts_with("  ─"), "{frame}");
            assert!(
                !frame.contains(" / T") && !frame.contains("p-second"),
                "{frame}"
            );
            assert!(
                frame.lines().any(|line| selected_prefix(line)
                    && line.contains("Second")
                    && line.contains("pane")),
                "{frame}"
            );
            assert_eq!(state.selected.as_deref(), Some(id));
        }
    }

    #[test]
    fn terminal_footer_stays_on_bottom_row_for_short_content() {
        for width in [76, 120] {
            let mut state = ui();
            for height in [5, 16] {
                let frame = state.terminal_frame(width, height);
                let lines: Vec<_> = frame.split("\r\n").collect();
                assert_eq!(lines.len(), height, "{frame}");
                assert!(lines[height - 1].starts_with("  Enter: focus"), "{frame}");
                assert!(
                    lines.iter().all(|line| without_status_ansi(line)
                        .replace("\x1b[48;5;236m", "")
                        .replace("\x1b[0m", "")
                        .replace("\x1b[2m", "")
                        .replace("\x1b[22m", "")
                        .chars()
                        .count()
                        < width),
                    "{frame}"
                );
            }
            state.query = "not found".into();
            let empty = state.terminal_frame(width, 16);
            assert!(empty.contains("No matching panes"), "{empty}");
            assert_eq!(empty.split("\r\n").count(), 16, "{empty}");
            state.mode = Mode::Help;
            let help = state.terminal_frame(width, 16);
            assert_eq!(help.split("\r\n").count(), 16, "{help}");
            assert!(help.ends_with("  Esc/q/?: back"), "{help}");
        }
    }

    #[test]
    fn search_hint_appears_only_in_select_with_empty_query() {
        let mut state = ui();
        for width in [76, 120] {
            assert_eq!(
                state.terminal_frame(width, 8).split("\r\n").next(),
                Some("  Search: / to search")
            );
        }
        state.key(Key::Char('/'));
        assert_eq!(
            state.terminal_frame(120, 8).split("\r\n").next(),
            Some("  Search: ")
        );
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        assert_eq!(
            state.terminal_frame(120, 8).split("\r\n").next(),
            Some("  Search: codex")
        );
        state.key(Key::Enter);
        assert_eq!(
            state.terminal_frame(120, 8).split("\r\n").next(),
            Some("  Search: codex")
        );
        state.key(Key::Char('r'));
        assert_eq!(
            state.terminal_frame(120, 8).split("\r\n").next(),
            Some("  Search: / to search")
        );
        state.key(Key::Char('?'));
        assert_eq!(
            state.terminal_frame(120, 8).split("\r\n").next(),
            Some("  Search: ")
        );
    }

    #[test]
    fn down_moves_marker_until_viewport_bottom_before_scrolling() {
        for (width, height) in [(76, 5), (120, 6)] {
            let mut state = ui();
            state.terminal_frame(width, height);
            state.key(Key::Down);
            let moved = state.terminal_frame(width, height);
            let lines: Vec<_> = moved.split("\r\n").collect();
            assert!(
                lines.iter().any(|line| line.contains("Claude")) == (height > 5),
                "{moved}"
            );
            assert!(
                lines
                    .iter()
                    .any(|line| selected_prefix(line) && line.contains("Same")),
                "{moved}"
            );
        }
        for (width, height) in [(76, 10), (120, 11)] {
            let mut state = ui();
            let initial = state.terminal_frame(width, height);
            assert!(
                initial
                    .split("\r\n")
                    .any(|line| selected_prefix(line) && line.contains("Claude")),
                "{initial}"
            );
            state.key(Key::Down);
            let next = state.terminal_frame(width, height);
            let lines: Vec<_> = next.split("\r\n").collect();
            assert!(lines.iter().any(|line| line.contains("Claude")), "{next}");
            assert!(
                lines
                    .iter()
                    .any(|line| selected_prefix(line) && line.contains("Same")),
                "{next}"
            );
            state.key(Key::Down);
            let third = state.terminal_frame(width, height);
            let lines: Vec<_> = third.split("\r\n").collect();
            assert!(lines.iter().any(|line| line.contains("Claude")), "{third}");
            assert!(
                lines
                    .iter()
                    .any(|line| selected_prefix(line) && line.contains("pane 3")),
                "{third}"
            );
            state.key(Key::Down);
            let fourth = state.terminal_frame(width, height);
            let lines: Vec<_> = fourth.split("\r\n").collect();
            assert!(lines.iter().any(|line| line.contains("Claude")), "{fourth}");
            assert!(
                lines
                    .iter()
                    .any(|line| selected_prefix(line) && line.contains("pane 4")),
                "{fourth}"
            );
            for _ in 0..2 {
                state.key(Key::Down);
                state.terminal_frame(width, height);
            }
            let scrolled = state.terminal_frame(width, height);
            assert!(
                scrolled.contains("> ") && scrolled.contains("codex"),
                "{scrolled}"
            );
            assert!(
                !scrolled.contains("Claude") && !scrolled.contains("p-other"),
                "{scrolled}"
            );
            assert_eq!(scrolled.split("\r\n").count(), height, "{scrolled}");
        }
    }

    #[test]
    fn moving_up_inside_scrolled_window_does_not_jump_to_first_row() {
        for (width, height) in [(76, 6), (120, 7)] {
            let mut state = ui();
            state.terminal_frame(width, height);
            for _ in 0..3 {
                state.key(Key::Down);
                state.terminal_frame(width, height);
            }
            let before = state.terminal_frame(width, height);
            assert!(!before.contains("Claude"), "{before}");
            state.key(Key::Up);
            let after = state.terminal_frame(width, height);
            assert!(!after.contains("Claude"), "{after}");
            assert!(after.contains("> ") && after.contains("pane 3"), "{after}");
            assert_eq!(after.split("\r\n").count(), height);
        }
    }

    #[test]
    fn crossing_tab_keeps_visible_rows_until_viewport_is_full() {
        let mut state = ui();
        state.terminal_frame(120, 12);
        for _ in 0..4 {
            state.key(Key::Down);
            state.terminal_frame(120, 12);
        }
        let frame = state.terminal_frame(120, 12);
        let lines: Vec<_> = frame.split("\r\n").collect();
        assert!(lines[1].starts_with("  ─"), "{frame}");
        assert!(
            frame.contains("Same") && frame.contains("Second"),
            "{frame}"
        );
        assert!(
            frame.contains("pane 4") && frame.contains("Same"),
            "{frame}"
        );
        assert!(
            frame.lines().any(|line| selected_prefix(line)
                && line.contains("Second")
                && line.contains("pane 1")),
            "{frame}"
        );
    }

    #[test]
    fn scrolling_across_tab_keeps_last_old_pane_until_new_row_needs_room() {
        let mut state = ui();
        for (width, height) in [(76, 9), (120, 10)] {
            state.viewport_start.set(0);
            state.selected = Some("p-b".into());
            state.terminal_frame(width, height);
            for _ in 0..3 {
                state.key(Key::Down);
                state.terminal_frame(width, height);
            }
            assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
            state.key(Key::Down);
            let frame = state.terminal_frame(width, height);
            assert!(frame.contains("pane 4"), "{frame}");
            assert!(
                frame.lines().any(|line| selected_prefix(line)
                    && line.contains("Second")
                    && line.contains("pane 1")),
                "{frame}"
            );
            assert!(
                !frame.contains("p-second-tab") && !frame.contains(" / T"),
                "{frame}"
            );
            assert_eq!(frame.split("\r\n").count(), height, "{frame}");
            for _ in 0..4 {
                state.key(Key::Up);
                state.terminal_frame(width, height);
            }
            let top = state.terminal_frame(width, height);
            assert!(top.contains("> ") && top.contains("Claude"), "{top}");
            state.key(Key::Down);
            let again = state.terminal_frame(width, height);
            assert!(
                again.contains("Claude") && again.contains("Same"),
                "{again}"
            );
        }
    }

    #[test]
    fn tall_filled_viewport_scrolls_one_pane_per_down_and_resets_after_up() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        for n in 0..12 {
            snapshot["result"]["snapshot"]["panes"].as_array_mut().unwrap().push(serde_json::json!({"pane_id": format!("p-extra-{n}"), "workspace_id": "w1", "tab_id": "t1", "label": "filler"}));
        }
        for (width, height) in [(76, 10), (120, 11)] {
            let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
            state.terminal_frame(width, height);
            let mut previous_first = 0;
            for step in 1..13 {
                state.key(Key::Down);
                let frame = state.terminal_frame(width, height);
                let first = state.viewport_start.get();
                assert!(first <= previous_first + 1, "step {step}: {frame}");
                previous_first = first;
                assert!(
                    frame.contains("> ")
                        && frame.contains("Same")
                        && frame.split("\r\n").count() == height
                        && !frame.contains("p-extra"),
                    "{frame}"
                );
            }
            assert!(previous_first > 0);
            for _ in 0..12 {
                state.key(Key::Up);
                state.terminal_frame(width, height);
            }
            let top = state.terminal_frame(width, height);
            assert!(top.contains("> ") && top.contains("Claude"), "{top}");
            state.key(Key::Down);
            assert!(state.terminal_frame(width, height).contains("Claude"));
        }
    }

    #[test]
    fn hidden_pane_match_reveals_name_and_tiny_width_keeps_marker() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["panes"][2]["label"] = Value::from("Hidden Needle");
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        state.query = "Needle".into();
        let frame = state.terminal_frame(120, 20);
        assert!(
            frame.contains("Claude (Hidden Needle)") && frame.contains("working"),
            "{frame}"
        );
        let compact = state.terminal_frame(76, 20);
        assert!(
            compact.contains("Claude (Hidde… \x1b[2m|\x1b[22m "),
            "{compact}"
        );
        let tiny = state.terminal_frame(16, 5);
        assert!(tiny.contains("> ") && !tiny.contains("p-b"), "{tiny}");
        assert!(
            tiny.split("\r\n").all(|line| line
                .replace("\x1b[48;5;236m", "")
                .replace("\x1b[0m", "")
                .replace("\x1b[2m", "")
                .replace("\x1b[22m", "")
                .chars()
                .count()
                <= 15),
            "{tiny}"
        );
    }

    #[test]
    fn query_above_list_and_help_describes_actual_keys() {
        let mut state = ui();
        state.key(Key::Char('/'));
        state.key(Key::Char('x'));
        let frame = state.terminal_frame(120, 20);
        assert!(frame.starts_with("  Search: x\r\n"), "{frame}");
        state.key(Key::Esc);
        assert!(state.terminal_frame(120, 20).contains("  Search: x\r\n"));
        state.key(Key::Char('?'));
        let help = state.terminal_frame(120, 20);
        assert!(help.contains("Enter: focus"), "{help}");
        assert!(help.contains("Esc/q/?: back"), "{help}");
        assert!(!help.contains("Esc/q: close"), "{help}");
    }

    #[test]
    fn navigation_uses_visible_rows_and_adjacent_tab_blocks() {
        let mut state = ui();
        assert_eq!(state.selected.as_deref(), Some("p-b"));
        state.key(Key::Char('j'));
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.key(Key::Char('l'));
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.key(Key::Left);
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.key(Key::Up);
        assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
        state.key(Key::Char('h'));
        assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
        state.key(Key::Char('k'));
        assert_eq!(state.selected.as_deref(), Some("p-untitled"));
        state.key(Key::Down);
        assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
    }

    #[test]
    fn search_retains_text_and_selection_and_reset_restores_id() {
        let mut state = ui();
        state.key(Key::Char('j'));
        state.key(Key::Char('/'));
        for c in "codex".chars() {
            state.key(Key::Char(c));
        }
        assert_eq!(state.visible().len(), 1);
        assert!(state
            .frame()
            .lines()
            .any(|line| line.contains("> ") && line.contains("codex")));
        state.key(Key::Enter);
        assert_eq!(state.mode, Mode::Select);
        assert_eq!(state.query, "codex");
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        state.key(Key::Char('/'));
        assert_eq!(state.query, "codex");
        state.key(Key::Esc);
        state.key(Key::Char('r'));
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert!(state
            .frame()
            .lines()
            .any(|line| line.contains("  > ") && line.contains("Same")));
        state.key(Key::Char('/'));
        for c in "rq".chars() {
            state.key(Key::Char(c));
        }
        assert_eq!(state.query, "rq");
        assert!(!state.closed);
        assert!(state.frame().contains("No matching panes"));
        state.key(Key::Backspace);
        assert_eq!(state.query, "r");
        state.key(Key::Esc);
        assert!(!state.closed);
        state.key(Key::Char('?'));
        assert_eq!(state.mode, Mode::Help);
        state.key(Key::Esc);
        state.key(Key::Char('q'));
        assert!(state.closed);
    }

    #[test]
    fn subsequence_is_case_insensitive_and_never_spans_fields() {
        assert!(matches_field("Moped Frontend", "operont"));
        assert!(matches_field("Moped Frontend", "OPERONT"));
        let mut state = ui();
        state.query = "amesame".to_owned();
        assert!(state.visible().is_empty());
        state.query = "SECOND".to_owned();
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|&i| state.rows[i].id.as_str())
                .collect::<Vec<_>>(),
            vec!["p-second-tab"]
        );
        state.query = "codex".to_owned();
        assert_eq!(state.visible().len(), 1);
    }

    #[test]
    fn arrow_parser_and_empty_state() {
        let mut keys = Keys::new();
        assert!(keys.feed(27).is_none());
        assert!(keys.feed(b'[').is_none());
        assert!(matches!(keys.feed(b'B'), Some(Key::Down)));
        assert!(keys.feed(27).is_none());
        assert!(matches!(keys.flush(), Some(Key::Esc)));
        let empty = Ui::new(Vec::new());
        assert!(empty.frame().contains("No matching panes"));
    }

    #[test]
    fn filtered_navigation_skips_hidden_rows_and_retains_topology_order() {
        let mut state = ui();
        state.query = "Same".into();
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|&i| state.rows[i].id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "p-b",
                "p-a",
                "p-untitled",
                "p-untitled-2",
                "p-second-tab",
                "p-other",
                "p-other-untitled"
            ]
        );
        state.query = "pane".into();
        state.key(Key::Down);
        assert_eq!(state.selected.as_deref(), Some("p-untitled-2"));
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-second-tab"));
        state.key(Key::Right);
        assert_eq!(state.selected.as_deref(), Some("p-other"));
        state.key(Key::Down);
        assert_eq!(state.selected.as_deref(), Some("p-other-untitled"));
        assert!(state.frame().lines().any(|line| line.contains("pane 2")));
    }

    #[test]
    fn first_remaining_row_shows_its_workspace_and_tab_context() {
        let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
        snapshot["result"]["snapshot"]["workspaces"][1]["label"] = Value::from("Workshop");
        snapshot["result"]["snapshot"]["tabs"][1]["label"] = Value::from("Backend");
        snapshot["result"]["snapshot"]["panes"][3]["label"] = Value::from("Needle");
        let mut state = Ui::new(rows(&snapshot.to_string(), Some("p-self"), 160).unwrap());
        state.query = "Needle".into();
        let frame = state.frame();
        let lines: Vec<_> = frame.lines().collect();
        assert_eq!(lines[0], "  Search: Needle");
        assert!(lines[1].starts_with("  ─"));
        assert!(
            frame.lines().any(|line| line.contains("Workshop")
                && line.contains("Backend")
                && line.contains("Needle")),
            "{frame}"
        );
        assert!(frame
            .lines()
            .any(|line| line.starts_with("  > ") && line.contains("Needle")));
        assert!(!frame.contains("Claude") && !frame.contains("p-a"));
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
        assert_eq!(lines.len(), 14);
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
        assert!(output.contains('…'));
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
            empty_output.contains("No matching panes")
                && empty_output.contains("Agent/Pane")
                && empty_output.contains("j/k: ↕pane"),
            "{empty_output}"
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
