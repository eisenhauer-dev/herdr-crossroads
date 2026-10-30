use std::path::{Path, PathBuf};

use super::NAMES;

// Only a literal, single manual choice is provable without a TOML parser or Herdr API.
pub(crate) fn auto_choice() -> (&'static str, Option<String>) {
    let ssh = ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
    let path = if let Some(path) = std::env::var_os("HERDR_CONFIG_PATH") {
        PathBuf::from(path)
    } else if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("herdr/config.toml")
    } else if cfg!(windows) && std::env::var_os("APPDATA").is_some() {
        PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("herdr/config.toml")
    } else if cfg!(windows) && std::env::var_os("USERPROFILE").is_some() {
        PathBuf::from(std::env::var_os("USERPROFILE").unwrap())
            .join("AppData/Roaming/herdr/config.toml")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/herdr/config.toml")
    } else {
        return ("catppuccin", None);
    };
    auto_choice_at(Some(&path), ssh)
}

pub(crate) fn auto_choice_at(path: Option<&Path>, ssh: bool) -> (&'static str, Option<String>) {
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

pub(crate) fn saved_choice(content: &str) -> (&'static str, Option<String>) {
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
