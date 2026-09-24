use serde_json::Value;
use std::io::{self, IsTerminal, Read, Write};
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

struct Row {
    id: String,
    workspace: String,
    tab: String,
    pane: String,
    agent: String,
    tab_id: String,
    display: String,
}

fn rows(response: &str, own_pane: Option<&str>, width: usize) -> io::Result<Vec<Row>> {
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
        let agent_full = clean(agent_name);
        let agent_name = clipped(
            &agent_full,
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
        let workspace_name = clean(field(workspace, "label")?);
        let tab_name = clean(field(tab, "label")?);
        let pane_name = clean(&pane_name);
        let names = format!("{} / {} / {}", workspace_name, tab_name, pane_name);
        let remaining = width.saturating_sub(columns(&prefix) + 3);
        let row = if remaining > 0 {
            format!("{prefix} | {}", clipped(&names, remaining))
        } else {
            prefix
        };
        rows.push((
            (wn, tn, index),
            Row {
                id: id.to_owned(),
                workspace: workspace_name,
                tab: tab_name,
                pane: pane_name,
                agent: agent_full,
                tab_id: format!("{workspace_id}/{tab_id}"),
                display: row,
            },
        ));
    }
    rows.sort_by_key(|(order, _)| *order);
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

#[cfg(test)]
fn render(response: &str, own_pane: Option<&str>, width: usize) -> io::Result<String> {
    let mut output = String::from("Herdr Crossroads\n");
    for row in rows(response, own_pane, width)? {
        output.push_str(&row.display);
        output.push('\n');
    }
    output.push_str("Press Enter to close.\n");
    Ok(output)
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
    selected: Option<String>,
    query: String,
    mode: Mode,
    closed: bool,
}

impl Ui {
    fn new(rows: Vec<Row>) -> Self {
        let selected = rows.first().map(|row| row.id.clone());
        Self {
            rows,
            selected,
            query: String::new(),
            mode: Mode::Select,
            closed: false,
        }
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

    fn key(&mut self, key: Key) {
        if self.mode == Mode::Help {
            if matches!(key, Key::Esc | Key::Char('?') | Key::Char('q')) {
                self.mode = Mode::Select;
            }
            return;
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
            return;
        }
        let visible = self.visible();
        match key {
            Key::Char('/') => self.mode = Mode::Search,
            Key::Char('?') => self.mode = Mode::Help,
            Key::Char('q') | Key::Esc => self.closed = true,
            Key::Char('r') => self.query.clear(),
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
    }

    fn frame(&self) -> String {
        let mut out = String::from("Herdr Crossroads\n");
        let visible = self.visible();
        if self.mode == Mode::Help {
            out.push_str("j/k or down/up: row  h/l or left/right: tab  /: search  r: reset  ?: help  Esc/q: close\n");
        } else if visible.is_empty() {
            out.push_str("No matching panes\n");
        } else {
            let selected = self.selected_index(&visible);
            for i in visible {
                out.push_str(if Some(i) == selected { "> " } else { "  " });
                out.push_str(&self.rows[i].display);
                out.push('\n');
            }
        }
        if self.mode == Mode::Search {
            out.push_str(&format!("Search: {}\n", self.query));
        } else {
            out.push_str(&format!(
                "Select (/: search, ?: help, q/Esc: close)  Filter: {}\n",
                self.query
            ));
        }
        out
    }

    fn terminal_frame(&self, width: usize, height: usize) -> String {
        let limit = width.saturating_sub(1);
        let visible = self.visible();
        let selected = self.selected_index(&visible);
        let available = height.saturating_sub(2);
        let start = selected
            .and_then(|id| visible.iter().position(|&i| i == id))
            .unwrap_or(0)
            .saturating_sub(available.saturating_sub(1));
        let mut lines = vec![clipped("Herdr Crossroads", limit)];
        if self.mode == Mode::Help {
            lines.push(clipped(
                "j/k: row  h/l: tab  /: search  r: reset  ?: help  Esc/q: close",
                limit,
            ));
        } else if visible.is_empty() {
            lines.push(clipped("No matching panes", limit));
        } else {
            for &i in visible.iter().skip(start).take(available) {
                lines.push(clipped(
                    &format!(
                        "{}{}",
                        if Some(i) == selected { "> " } else { "  " },
                        self.rows[i].display
                    ),
                    limit,
                ));
            }
        }
        let footer = if self.mode == Mode::Search {
            format!("Search: {}", self.query)
        } else {
            format!(
                "Select (/: search, ?: help, q/Esc: close)  Filter: {}",
                self.query
            )
        };
        lines.push(clipped(&footer, limit));
        lines.join("\r\n")
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
                _ => None,
            };
        }
        match byte {
            27 => {
                self.escape = 1;
                None
            }
            b'\r' | b'\n' => Some(Key::Enter),
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
        let (height, width) = terminal_size();
        write!(out, "\x1b[H\x1b[J{}", ui.terminal_frame(width, height))?;
    } else {
        out.write_all(ui.frame().as_bytes())?;
    }
    out.flush()
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
    let mut ui = Ui::new(rows(
        &snapshot,
        std::env::var("HERDR_PANE_ID").ok().as_deref(),
        terminal_size().1.saturating_sub(2),
    )?);
    let terminal = Terminal::enter()?;
    draw(&ui, terminal.0.is_some())?;
    let mut input = io::stdin().lock();
    let mut keys = Keys::new();
    let mut byte = [0];
    loop {
        let n = input.read(&mut byte)?;
        if n == 0 {
            if let Some(key) = keys.flush() {
                ui.key(key);
                draw(&ui, terminal.0.is_some())?;
            }
            if terminal.0.is_none() || ui.closed {
                break;
            }
            continue;
        }
        if keys.escape == 1 && byte[0] != b'[' {
            if let Some(key) = keys.flush() {
                ui.key(key);
                draw(&ui, terminal.0.is_some())?;
            }
            if ui.closed {
                break;
            }
        }
        if let Some(key) = keys.feed(byte[0]) {
            ui.key(key);
            draw(&ui, terminal.0.is_some())?;
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

    fn ui() -> Ui {
        Ui::new(rows(SNAPSHOT, Some("p-self"), 160).unwrap())
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
        assert!(state.frame().contains("> W2 T1 p-other"));
        state.key(Key::Enter);
        assert_eq!(state.mode, Mode::Select);
        assert_eq!(state.query, "codex");
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        state.key(Key::Char('/'));
        assert_eq!(state.query, "codex");
        state.key(Key::Esc);
        state.key(Key::Char('r'));
        assert_eq!(state.selected.as_deref(), Some("p-a"));
        assert!(state.frame().contains("> W1 T1 p-a"));
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
        assert!(state.frame().contains("Same / Same / pane 2"));
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
        assert_eq!(
            lines[1],
            "> W1 T1 p-a | - | idle | Workshop / Backend / Needle"
        );
        assert!(!frame.contains("p-b"));
    }

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
