use jsonc_parser::common::Ranged;
use std::collections::BTreeMap;
use std::path::Path;

use super::{config, input::Key};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    Select,
    Help,
    Themes,
}
impl Context {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "select" => Some(Self::Select),
            "help" => Some(Self::Help),
            "themes" => Some(Self::Themes),
            _ => None,
        }
    }
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Help => "help",
            Self::Themes => "themes",
        }
    }
}

pub(super) struct Binding {
    pub(super) context: Context,
    pub(super) action: &'static str,
    pub(super) keys: Vec<Key>,
}
pub(super) struct Keymap {
    pub(super) bindings: Vec<Binding>,
    pub(super) oversized_actions: Vec<OversizedAction>,
    pub(super) diagnostics: config::Diagnostics,
}

pub(super) struct OversizedAction {
    pub(super) context: String,
    pub(super) action: String,
}

pub(super) const KEY_LIMIT_HINT: &str = "Maximum 2 keybinds per action.";

impl Default for Keymap {
    fn default() -> Self {
        let mut bindings = Vec::new();
        for (context, actions) in [
            (
                Context::Select,
                &[
                    ("next_row", "j/Down"),
                    ("previous_row", "k/Up"),
                    ("previous_tab", "h/Left"),
                    ("next_tab", "l/Right"),
                    ("next_workspace", "Tab"),
                    ("previous_workspace", "Shift+Tab"),
                    ("focus_pane", "Enter"),
                    ("open_search", "/"),
                    ("reset_search", "r"),
                    ("open_themes", "t"),
                    ("open_help", "?"),
                    ("close", "Esc/q"),
                ][..],
            ),
            (
                Context::Help,
                &[
                    ("next_category", "j/Down"),
                    ("previous_category", "k/Up"),
                    ("open_search", "/"),
                    ("reset_search", "r"),
                    ("close", "Esc/q"),
                ][..],
            ),
            (
                Context::Themes,
                &[
                    ("next_theme", "j/Down"),
                    ("previous_theme", "k/Up"),
                    ("toggle_preview", "Space"),
                    ("save", "Enter"),
                    ("cancel", "Esc/q"),
                ][..],
            ),
        ] {
            for &(action, keys) in actions {
                let keys = if action == "open_search" {
                    vec![Key::Char('/')]
                } else {
                    keys.split('/').map(|s| Key::parse(s).unwrap()).collect()
                };
                bindings.push(Binding {
                    context,
                    action,
                    keys,
                });
            }
        }
        Self {
            bindings,
            oversized_actions: Vec::new(),
            diagnostics: config::Diagnostics::default(),
        }
    }
}

impl Keymap {
    pub(super) fn load(dir: Option<&Path>) -> (Self, Vec<String>) {
        let Some(dir) = dir else {
            return (Self::default(), Vec::new());
        };
        match config::read(Some(dir), "keymaps") {
            Ok(Some((path, text))) => {
                let (mut map, errors) = Self::parse(&text);
                map.diagnostics.source = Some(path);
                (map, errors)
            }
            Ok(None) => (Self::default(), Vec::new()),
            Err(error) => {
                let mut map = Self::default();
                map.diagnostics.push("Keymap IO", error.to_string());
                map.diagnostics.source = Some(error.path.clone());
                (map, vec![error.to_string()])
            }
        }
    }

    pub(super) fn parse(text: &str) -> (Self, Vec<String>) {
        let mut map = Self::default();
        let parsed = match config::parse(text) {
            Ok(parsed) => parsed,
            Err(error) => {
                map.diagnostics.push("Keymap syntax", error.clone());
                return (map, vec![error]);
            }
        };
        let mut errors = parsed.errors;
        for duplicate in parsed.duplicate_fields {
            match duplicate.location.first() {
                Some(config::PathSegment::Field(context))
                    if Context::from_name(context).is_some() =>
                {
                    let action = config::location(&duplicate.location[1..]);
                    let message = if action.is_empty() {
                        format!("{}: duplicate field", duplicate.field)
                    } else {
                        format!("{action}: duplicate field {}", duplicate.field)
                    };
                    map.push_mode_diagnostic("Duplicate keymap fields", context, message);
                }
                _ => {
                    let location = config::location(&duplicate.location);
                    map.diagnostics.push(
                        "Duplicate keymap fields",
                        if location.is_empty() {
                            format!("duplicate field {}", duplicate.field)
                        } else {
                            format!("{location}: duplicate field {}", duplicate.field)
                        },
                    );
                }
            }
        }
        let Some(contexts) = parsed.ast.as_object() else {
            map.diagnostics
                .push("Keymap structure", "expected object of contexts");
            errors.push("expected object of contexts".into());
            return (map, errors);
        };
        // Keep AST property occurrences: duplicate fields must not hide other diagnostics.
        for context_entry in &contexts.properties {
            let context = context_entry.name.as_str();
            let actions = &context_entry.value;
            let known_context = Context::from_name(context).is_some();
            if !known_context {
                let error = format!("unknown context {context}");
                map.diagnostics
                    .push("Keymap unknown contexts", error.clone());
                errors.push(error);
            }
            let Some(actions) = actions.as_object() else {
                let error = format!("{context}: expected object of actions");
                map.push_mode_diagnostic(
                    "Keymap structure",
                    context,
                    "expected object of actions".into(),
                );
                errors.push(error);
                continue;
            };
            for action_entry in &actions.properties {
                let action = action_entry.name.as_str();
                let keys = &action_entry.value;
                let target = map
                    .bindings
                    .iter()
                    .position(|b| b.context.name() == context && b.action == action);
                if known_context && target.is_none() {
                    let error = format!("{context}: unknown action {action}");
                    map.push_mode_diagnostic("Unknown keymap actions", context, action.into());
                    errors.push(error);
                }
                let Some(keys) = keys.as_array().filter(|keys| !keys.elements.is_empty()) else {
                    let error = format!("{context}.{action}: expected nonempty key list");
                    map.push_mode_diagnostic(
                        "Keymap structure",
                        context,
                        format!("{action}: expected nonempty key list"),
                    );
                    errors.push(error);
                    continue;
                };
                let mut parsed = Vec::new();
                if keys.elements.len() > 2 {
                    map.oversized_actions.push(OversizedAction {
                        context: context.into(),
                        action: action.into(),
                    });
                }
                for key in &keys.elements {
                    match key.as_string_lit().and_then(|s| Key::parse(&s.value)) {
                        Some(key) => {
                            if !parsed.contains(&key) {
                                parsed.push(key);
                            }
                        }
                        None => {
                            let spelling = &text[key.range().start..key.range().end];
                            let modified = key.as_string_lit().is_some_and(|s| {
                                s.value.starts_with("Ctrl+") || s.value.starts_with("Alt+")
                            });
                            let message = if modified {
                                format!("{action}: {spelling}: Ctrl/Alt bindings are not allowed")
                            } else {
                                format!("{action}: unknown key {spelling}")
                            };
                            let error = format!("{context}.{message}");
                            map.push_mode_diagnostic(
                                if modified {
                                    "Unsupported keymap keys"
                                } else {
                                    "Unknown keymap keys"
                                },
                                context,
                                message,
                            );
                            errors.push(error);
                        }
                    }
                }
                if let Some(target) = target {
                    map.bindings[target].keys = parsed;
                }
            }
        }
        errors.extend(map.conflicts());
        for (context, actions, keys) in map.collision_groups() {
            map.diagnostics.push(
                format!("Keymap collisions - {context} mode"),
                format!("{}: {}", actions.join(", "), keys.join("/")),
            );
        }
        if !map.oversized_actions.is_empty() {
            // One pathless marker keeps the existing startup/error-page gate;
            // individual limit failures stay structured, not human-text parsed.
            errors.insert(0, KEY_LIMIT_HINT.into());
        }
        (map, errors)
    }

    fn push_mode_diagnostic(&mut self, kind: &str, context: &str, message: String) {
        if Context::from_name(context).is_some() {
            self.diagnostics
                .push(format!("{kind} - {context} mode"), message);
        } else {
            self.diagnostics
                .push(kind, format!("unknown mode {context}: {message}"));
        }
    }

    fn conflicts(&self) -> Vec<String> {
        let mut owners: BTreeMap<(String, String), Vec<&str>> = BTreeMap::new();
        for binding in &self.bindings {
            for key in &binding.keys {
                owners
                    .entry((binding.context.name().into(), key.label()))
                    .or_default()
                    .push(binding.action);
            }
        }
        owners
            .into_iter()
            .filter(|(_, actions)| actions.len() > 1)
            .map(|((context, key), actions)| format!("{context} | {key} | {}", actions.join(", ")))
            .collect()
    }

    fn collision_groups(&self) -> Vec<(String, Vec<&'static str>, Vec<String>)> {
        let mut owners: BTreeMap<(String, String), (Key, Vec<&str>)> = BTreeMap::new();
        for binding in &self.bindings {
            for key in &binding.keys {
                owners
                    .entry((binding.context.name().into(), key.label()))
                    .or_insert_with(|| (*key, Vec::new()))
                    .1
                    .push(binding.action);
            }
        }
        let mut groups: BTreeMap<(String, Vec<&str>), Vec<String>> = BTreeMap::new();
        for ((context, _), (key, actions)) in owners {
            if actions.len() > 1 {
                groups
                    .entry((context, actions))
                    .or_default()
                    .push(key.display_label());
            }
        }
        for keys in groups.values_mut() {
            keys.sort();
        }
        groups
            .into_iter()
            .map(|((context, actions), keys)| (context, actions, keys))
            .collect()
    }

    pub(super) fn diagnostic_blocks(&self) -> Vec<(String, Vec<String>)> {
        let mut diagnostics = config::Diagnostics::default();
        for entry in &self.oversized_actions {
            let heading = if Context::from_name(&entry.context).is_some() {
                format!("Maximum 2 keybinds per action - {} mode", entry.context)
            } else {
                "Maximum 2 keybinds per action".into()
            };
            diagnostics.push(
                heading,
                if Context::from_name(&entry.context).is_some() {
                    entry.action.clone()
                } else {
                    format!("{} (unknown mode {})", entry.action, entry.context)
                },
            );
        }
        diagnostics.blocks.extend(self.diagnostics.blocks.clone());
        diagnostics.blocks
    }

    pub(super) fn action(&self, context: Context, key: Key) -> Option<&'static str> {
        self.bindings
            .iter()
            .find(|b| b.context == context && b.keys.contains(&key))
            .map(|b| b.action)
    }
    #[cfg(test)]
    pub(super) fn labels(&self, context: Context, action: &str) -> String {
        self.bindings
            .iter()
            .find(|b| b.context == context && b.action == action)
            .map(|b| {
                b.keys
                    .iter()
                    .map(|k| k.label())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    // Display only: canonical key labels and conflict identities remain unchanged.
    pub(super) fn display_labels(&self, context: Context, action: &str) -> String {
        self.bindings
            .iter()
            .find(|b| b.context == context && b.action == action)
            .map(|b| {
                if b.keys == [Key::Char('/')] {
                    return "/".into();
                }
                b.keys
                    .iter()
                    .map(|key| key.display_label())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub(super) fn footer_labels(&self, context: Context, action: &str) -> String {
        let navigation = match context {
            Context::Select => matches!(
                action,
                "next_row"
                    | "previous_row"
                    | "next_tab"
                    | "previous_tab"
                    | "next_workspace"
                    | "previous_workspace"
            ),
            Context::Help => matches!(action, "next_category" | "previous_category"),
            Context::Themes => matches!(action, "next_theme" | "previous_theme"),
        };
        self.bindings
            .iter()
            .find(|b| b.context == context && b.action == action)
            .map(|b| {
                b.keys
                    .iter()
                    .enumerate()
                    .filter(|(at, key)| *at == 0 || (navigation && key.is_arrow()))
                    .map(|(_, key)| key.display_label())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }
}
