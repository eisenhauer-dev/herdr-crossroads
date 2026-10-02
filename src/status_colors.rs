#[cfg(test)]
use std::io;
use std::path::Path;

use ratatui::style::Color;

use super::theme::Theme;

pub(super) struct StatusColors {
    pub(super) working: Color,
    blocked: Color,
    done: Color,
    pub(super) idle: Color,
    pub(super) unknown: Color,
    overrides: [bool; 5],
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            working: Color::Rgb(249, 226, 175),
            blocked: Color::Rgb(243, 139, 168),
            done: Color::Rgb(148, 226, 213),
            idle: Color::Rgb(166, 227, 161),
            unknown: Color::Rgb(108, 112, 134),
            overrides: [false; 5],
        }
    }
}

impl StatusColors {
    pub(super) fn for_status(&self, status: &str) -> Color {
        match status {
            "working" => self.working,
            "blocked" => self.blocked,
            "done" => self.done,
            "idle" => self.idle,
            _ => self.unknown,
        }
    }

    pub(super) fn themed(&self, status: &str, theme: Theme) -> Color {
        let (index, color) = match status {
            "working" => (0, theme.working),
            "blocked" => (1, theme.blocked),
            "done" => (2, theme.done),
            "idle" => (3, theme.idle),
            _ => (4, theme.unknown),
        };
        if self.overrides[index] {
            self.for_status(status)
        } else {
            color
        }
    }

    #[cfg(test)]
    pub(super) fn load(dir: Option<&Path>) -> io::Result<Self> {
        let (colors, diagnostics) = Self::load_diagnostics(dir);
        if diagnostics.is_empty() {
            Ok(colors)
        } else {
            Err(io::Error::other(diagnostics.messages().join("; ")))
        }
    }

    pub(super) fn load_diagnostics(dir: Option<&Path>) -> (Self, super::config::Diagnostics) {
        let mut colors = Self::default();
        let mut diagnostics = super::config::Diagnostics::default();
        let content = match super::config::read(dir, "status-colors") {
            Ok(Some((path, content))) => {
                diagnostics.source = Some(path);
                content
            }
            Ok(None) => return (colors, diagnostics),
            Err(error) => {
                diagnostics.push("Status colors IO", error.to_string());
                diagnostics.source = Some(error.path);
                return (colors, diagnostics);
            }
        };
        let parsed = match super::config::parse(&content) {
            Ok(parsed) => parsed,
            Err(error) => {
                diagnostics.push("Status colors syntax", error);
                return (colors, diagnostics);
            }
        };
        for duplicate in parsed.duplicate_fields {
            let location = super::config::location(&duplicate.location);
            diagnostics.push(
                "Status colors duplicate fields",
                if location.is_empty() {
                    format!("duplicate field {}", duplicate.field)
                } else {
                    format!("{location}: duplicate field {}", duplicate.field)
                },
            );
        }
        let Some(entries) = parsed.ast.as_object() else {
            diagnostics.push("Status colors structure", "expected JSON object");
            return (colors, diagnostics);
        };
        for entry in &entries.properties {
            let key = entry.name.as_str();
            if !["working", "blocked", "done", "idle", "unknown"].contains(&key) {
                diagnostics.push(
                    "Status colors unknown keys",
                    format!("unknown status key {key}"),
                );
            }
            let hex = entry
                .value
                .as_string_lit()
                .map(|literal| literal.value.as_ref())
                .filter(|hex| {
                    hex.len() == 7
                        && hex.starts_with('#')
                        && hex.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                });
            let Some(hex) = hex else {
                diagnostics.push(
                    "Status colors invalid values",
                    format!("{key} must be #RRGGBB"),
                );
                continue;
            };
            let target = match key {
                "working" => &mut colors.working,
                "blocked" => &mut colors.blocked,
                "done" => &mut colors.done,
                "idle" => &mut colors.idle,
                "unknown" => &mut colors.unknown,
                _ => continue,
            };
            *target = Color::Rgb(
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
            );
            colors.overrides[match key {
                "working" => 0,
                "blocked" => 1,
                "done" => 2,
                "idle" => 3,
                _ => 4,
            }] = true;
        }
        (colors, diagnostics)
    }
}
