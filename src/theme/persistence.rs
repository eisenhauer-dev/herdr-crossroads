use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::NAMES;
use crate::config;

fn config_path(dir: Option<&Path>) -> Option<PathBuf> {
    dir.map(|dir| dir.join("theme.jsonc"))
}

#[cfg(test)]
pub(crate) fn load(dir: Option<&Path>) -> (usize, Option<String>) {
    let (index, diagnostics) = load_diagnostics(dir);
    (
        index,
        (!diagnostics.is_empty()).then(|| {
            format!(
                "Theme configuration: {}; using Mocha",
                diagnostics.messages().join("; ")
            )
        }),
    )
}

pub(crate) fn load_diagnostics(dir: Option<&Path>) -> (usize, config::Diagnostics) {
    let mut diagnostics = config::Diagnostics::default();
    let content = match config::read(dir, "theme") {
        Ok(Some((path, content))) => {
            diagnostics.source = Some(path);
            content
        }
        Ok(None) => return (0, diagnostics),
        Err(error) => {
            diagnostics.push("Theme IO", error.to_string());
            diagnostics.source = Some(error.path);
            return (0, diagnostics);
        }
    };
    let parsed = match config::parse(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            diagnostics.push("Theme syntax", error);
            return (0, diagnostics);
        }
    };
    for duplicate in parsed.duplicate_fields {
        let location = config::location(&duplicate.location);
        diagnostics.push(
            "Theme duplicate fields",
            if location.is_empty() {
                format!("duplicate field {}", duplicate.field)
            } else {
                format!("{location}: duplicate field {}", duplicate.field)
            },
        );
    }
    let Some(object) = parsed.ast.as_object() else {
        diagnostics.push("Theme structure", "expected JSON object");
        return (0, diagnostics);
    };
    let mut index = 0;
    let mut has_theme = false;
    for entry in &object.properties {
        if entry.name.as_str() != "theme" {
            diagnostics.push(
                "Theme unknown fields",
                format!("unknown field {}", entry.name.as_str()),
            );
            continue;
        }
        has_theme = true;
        match entry.value.as_string_lit() {
            Some(name) => match NAMES
                .iter()
                .position(|entry| *entry == name.value)
                .filter(|&i| i > 0)
            {
                Some(found) => index = found,
                None => diagnostics.push(
                    "Theme invalid values",
                    format!("unknown theme {}", name.value),
                ),
            },
            None => diagnostics.push("Theme invalid values", "theme must be a string"),
        }
    }
    if !has_theme {
        diagnostics.push("Theme structure", "missing theme field");
    }
    (if diagnostics.is_empty() { index } else { 0 }, diagnostics)
}

pub(crate) fn save(dir: Option<&Path>, index: usize) -> io::Result<()> {
    let dir = dir.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "plugin config directory unavailable",
        )
    })?;
    let path = config_path(Some(dir)).unwrap();
    if index == 0 {
        let mut errors = Vec::new();
        for path in [path, dir.join("theme.json")] {
            if let Err(error) = std::fs::remove_file(&path) {
                if error.kind() != io::ErrorKind::NotFound
                    || std::fs::symlink_metadata(&path).is_ok()
                {
                    errors.push(error.to_string());
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    } else {
        std::fs::create_dir_all(dir)?;
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let (temp, mut file) = loop {
            let temp = dir.join(format!(
                ".theme.jsonc.{}.{}.tmp",
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
        let result = written.and_then(|()| std::fs::rename(&temp, &path));
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
}
