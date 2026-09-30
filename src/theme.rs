use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ratatui::style::Color;

// Herdr v0.9.1: src/config/theme.rs THEME_NAMES and src/app/state.rs Palette.
pub(super) const NAMES: [&str; 19] = [
    "Auto (Herdr)",
    "catppuccin",
    "catppuccin-latte",
    "terminal",
    "tokyo-night",
    "tokyo-night-day",
    "dracula",
    "nord",
    "gruvbox",
    "gruvbox-light",
    "one-dark",
    "one-light",
    "solarized",
    "solarized-light",
    "kanagawa",
    "kanagawa-lotus",
    "rose-pine",
    "rose-pine-dawn",
    "vesper",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Theme {
    pub(super) background: Color,
    pub(super) accent: Color,
    pub(super) text: Color,
    pub(super) dim: Color,
    pub(super) selection: Color,
    pub(super) working: Color,
    pub(super) blocked: Color,
    pub(super) done: Color,
    pub(super) idle: Color,
    pub(super) unknown: Color,
}

fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    pub(super) fn named(name: &str) -> Self {
        // Herdr v0.9.1 Palette::panel_bg (terminal retains the host default).
        let background = match name {
            "catppuccin-latte" => 0xeff1f5,
            "tokyo-night" => 0x1a1b26,
            "tokyo-night-day" => 0xe1e2e7,
            "dracula" => 0x282a36,
            "nord" => 0x2e3440,
            "gruvbox" => 0x282828,
            "gruvbox-light" => 0xfbf1c7,
            "one-dark" => 0x282c34,
            "one-light" => 0xfafafa,
            "solarized" => 0x002b36,
            "solarized-light" => 0xfdf6e3,
            "kanagawa" => 0x1f1f28,
            "kanagawa-lotus" => 0xf2ecbc,
            "rose-pine" => 0x191724,
            "rose-pine-dawn" => 0xfaf4ed,
            "vesper" => 0x1a1a1a,
            _ => 0x181825,
        };
        // accent, text, overlay0, selection_bg, yellow, red, teal, green.
        let values = match name {
            "catppuccin-latte" => [
                0x1e66f5, 0x4c4f69, 0x9ca0b0, 0xbdd0f5, 0xdf8e1d, 0xd20f39, 0x179299, 0x40a02b,
            ],
            "tokyo-night" => [
                0x7aa2f7, 0xc0caf5, 0x565f89, 0x2d3650, 0xe0af68, 0xf7768e, 0x7dcfff, 0x9ece6a,
            ],
            "tokyo-night-day" => [
                0x2e7de9, 0x3760bf, 0x8990b3, 0xb6cae7, 0x8c6c3e, 0xf52a65, 0x118c74, 0x587539,
            ],
            "dracula" => [
                0xbd93f9, 0xf8f8f2, 0x6272a4, 0x463f5d, 0xf1fa8c, 0xff5555, 0x8be9fd, 0x50fa7b,
            ],
            "nord" => [
                0x88c0d0, 0xeceff4, 0x4c566a, 0x40505d, 0xebcb8b, 0xbf616a, 0x8fbcbb, 0xa3be8c,
            ],
            "gruvbox" => [
                0xd79921, 0xebdbb2, 0x928374, 0x4b3f27, 0xfabd2f, 0xfb4934, 0x8ec07c, 0xb8bb26,
            ],
            "gruvbox-light" => [
                0x076678, 0x3c3836, 0x928374, 0xebdbb2, 0xb57614, 0x9d0006, 0x427b58, 0x79740e,
            ],
            "one-dark" => [
                0x61afef, 0xabb2bf, 0x5c6370, 0x334659, 0xe5c07b, 0xe06c75, 0x56b6c2, 0x98c379,
            ],
            "one-light" => [
                0x4078f2, 0x383a42, 0xa0a1a7, 0xcddbf8, 0xc18401, 0xe45649, 0x0184bc, 0x50a14f,
            ],
            "solarized" => [
                0x268bd2, 0x93a1a1, 0x586e75, 0x083e55, 0xb58900, 0xdc322f, 0x2aa198, 0x859900,
            ],
            "solarized-light" => [
                0x268bd2, 0x657b83, 0x93a1a1, 0xc9dcdf, 0xb58900, 0xdc322f, 0x2aa198, 0x859900,
            ],
            "kanagawa" => [
                0x7e9cd8, 0xdcd7ba, 0x727169, 0x32384b, 0xc0a36e, 0xc34043, 0x7fb4ca, 0x76946a,
            ],
            "kanagawa-lotus" => [
                0x4d699b, 0x545464, 0xa09cac, 0xdcd5ac, 0x77713f, 0xc84053, 0x4e8ca2, 0x6f894e,
            ],
            "rose-pine" => [
                0xc4a7e7, 0xe0def4, 0x6e6a86, 0x3b344b, 0xf6c177, 0xeb6f92, 0x9ccfd8, 0x31748f,
            ],
            "rose-pine-dawn" => [
                0x907aa9, 0x464261, 0x9893a5, 0xf2e9e1, 0xea9d34, 0xb4637a, 0x56949f, 0x286983,
            ],
            "vesper" => [
                0xffc799, 0xffffff, 0x5c5c5c, 0x232323, 0xffc799, 0xff8080, 0x66ddcc, 0x99ffe4,
            ],
            _ => [
                0x89b4fa, 0xcdd6f4, 0x6c7086, 0x313244, 0xf9e2af, 0xf38ba8, 0x94e2d5, 0xa6e3a1,
            ],
        };
        if name == "terminal" {
            return Self {
                background: Color::Reset,
                accent: Color::Blue,
                text: Color::Reset,
                dim: Color::Gray,
                selection: Color::Reset,
                working: Color::Yellow,
                blocked: Color::LightRed,
                done: Color::Cyan,
                idle: Color::Green,
                unknown: Color::Gray,
            };
        }
        Self {
            background: rgb(background),
            accent: rgb(values[0]),
            text: rgb(values[1]),
            dim: rgb(values[2]),
            selection: rgb(values[3]),
            working: rgb(values[4]),
            blocked: rgb(values[5]),
            done: rgb(values[6]),
            idle: rgb(values[7]),
            unknown: rgb(values[2]),
        }
    }
}

pub(super) fn config_path(dir: Option<&Path>) -> Option<PathBuf> {
    dir.map(|dir| dir.join("theme.json"))
}

pub(super) fn load(dir: Option<&Path>) -> (usize, Option<String>) {
    let Some(path) = config_path(dir) else {
        return (0, None);
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && std::fs::symlink_metadata(&path).is_err() =>
        {
            return (0, None)
        }
        Err(_) => return (0, Some("theme.json unreadable; using Mocha".into())),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(parsed) => parsed,
        Err(_) => return (0, Some("theme.json invalid; using Mocha".into())),
    };
    match parsed.as_object() {
        Some(object) if object.len() == 1 => match object.get("theme").and_then(|v| v.as_str()) {
            Some(name) => match NAMES
                .iter()
                .position(|entry| *entry == name)
                .filter(|&i| i > 0)
            {
                Some(index) => (index, None),
                None => (0, Some("theme.json invalid; using Mocha".into())),
            },
            None => (0, Some("theme.json invalid; using Mocha".into())),
        },
        _ => (0, Some("theme.json invalid; using Mocha".into())),
    }
}

pub(super) fn save(dir: Option<&Path>, index: usize) -> io::Result<()> {
    let dir = dir.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "plugin config directory unavailable",
        )
    })?;
    let path = dir.join("theme.json");
    if index == 0 {
        match std::fs::remove_file(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    } else {
        std::fs::create_dir_all(dir)?;
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let (temp, mut file) = loop {
            let temp = dir.join(format!(
                ".theme.json.{}.{}.tmp",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
            {
                Ok(file) => break (temp, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        let written = (|| {
            write!(file, "{{\"theme\":\"{}\"}}\n", NAMES[index])?;
            file.sync_all()
        })();
        drop(file);
        let result = written.and_then(|()| std::fs::rename(&temp, path));
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
}

// Only a literal, single manual choice is provable without a TOML parser or Herdr API.
pub(super) fn auto_choice() -> (&'static str, Option<String>) {
    let ssh = ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
    let path = if let Some(path) = std::env::var_os("HERDR_CONFIG_PATH") {
        PathBuf::from(path)
    } else if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("herdr/config.toml")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/herdr/config.toml")
    } else {
        return ("catppuccin", None);
    };
    auto_choice_at(Some(&path), ssh)
}

pub(super) fn auto_choice_at(path: Option<&Path>, ssh: bool) -> (&'static str, Option<String>) {
    if ssh {
        return ("catppuccin", None);
    }
    let Some(path) = path else {
        return ("catppuccin", None);
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return ("catppuccin", None);
    };
    saved_choice(&content)
}

pub(super) fn saved_choice(content: &str) -> (&'static str, Option<String>) {
    use std::collections::HashSet;
    let mut in_theme = false;
    let mut seen_theme = false;
    let mut seen_auto_switch = false;
    let mut choice = None;
    let mut tables = HashSet::new();
    let mut keys = HashSet::new();
    let mut has_array_table = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // Recognize only simple bare table headers; an incomplete or unsupported
            // header makes the manual choice unprovable, even outside [theme].
            let header = if let Some(header) = line.strip_prefix("[[") {
                header.strip_suffix("]]")
            } else {
                line.strip_prefix('[')
                    .and_then(|header| header.strip_suffix(']'))
            };
            let Some(header) = header else {
                return ("catppuccin", None);
            };
            if !header.split('.').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            }) {
                return ("catppuccin", None);
            }
            has_array_table |= line.starts_with("[[");
            if !tables.insert(header) {
                return ("catppuccin", None);
            }
            keys.clear();
            in_theme = line == "[theme]";
            if in_theme {
                if seen_theme {
                    return ("catppuccin", None);
                }
                seen_theme = true;
            }
            if line.starts_with("[theme.") || line == "[[theme]]" {
                return ("catppuccin", None);
            }
        } else if in_theme && !line.is_empty() && !line.starts_with('#') {
            let Some((key, value)) = line.split_once('=') else {
                return ("catppuccin", None);
            };
            if !keys.insert(key.trim()) {
                return ("catppuccin", None);
            }
            match (key.trim(), value.trim()) {
                ("name", value)
                    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') =>
                {
                    let name = &value[1..value.len() - 1];
                    if name.is_empty()
                        || name.chars().any(char::is_control)
                        || name.contains(['"', '\\'])
                        || choice.replace(name.to_owned()).is_some()
                    {
                        return ("catppuccin", None);
                    }
                }
                ("auto_switch", "false") if !seen_auto_switch => {
                    seen_auto_switch = true;
                }
                _ => return ("catppuccin", None),
            }
        } else if !line.is_empty() && !line.starts_with('#') {
            // Unknown syntax outside [theme] cannot prove a manual choice.
            let Some((key, value)) = line.split_once('=') else {
                return ("catppuccin", None);
            };
            let key = key.trim();
            let value = value.trim();
            if !keys.insert(key)
                || key.is_empty()
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
                || !(value == "true"
                    || value == "false"
                    || (!value.is_empty()
                        && (value.len() == 1 || !value.starts_with('0'))
                        && value.bytes().all(|byte| byte.is_ascii_digit()))
                    || (value.len() >= 2
                        && value.starts_with('"')
                        && value.ends_with('"')
                        && !value[1..value.len() - 1].contains(['"', '\\'])
                        && !value[1..value.len() - 1].chars().any(char::is_control)))
            {
                return ("catppuccin", None);
            }
        }
    }
    match NAMES
        .iter()
        .skip(1)
        .copied()
        .find(|name| Some(*name) == choice.as_deref())
    {
        Some(name) => (name, None),
        None => ("catppuccin", if has_array_table { None } else { choice }),
    }
}
