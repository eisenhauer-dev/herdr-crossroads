use std::io;
use std::path::{Path, PathBuf};

use jsonc_parser::{
    ast, common::Ranged, parse_to_ast, CollectOptions, CommentCollectionStrategy, ParseOptions,
};
use serde_json::Value;

pub(super) struct ReadError {
    pub(super) path: PathBuf,
    error: io::Error,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

// Only absence permits legacy fallback, including for dangling symlinks.
pub(super) fn read(dir: Option<&Path>, name: &str) -> Result<Option<(PathBuf, String)>, ReadError> {
    let Some(dir) = dir else { return Ok(None) };
    for extension in ["jsonc", "json"] {
        let path = dir.join(format!("{name}.{extension}"));
        match std::fs::read_to_string(&path) {
            Ok(text) => return Ok(Some((path, text))),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && matches!(std::fs::symlink_metadata(&path), Err(ref e) if e.kind() == io::ErrorKind::NotFound) =>
                {}
            Err(error) => return Err(ReadError { path, error }),
        }
    }
    Ok(None)
}

#[derive(Default)]
pub(super) struct Diagnostics {
    pub(super) blocks: Vec<(String, Vec<String>)>,
    pub(super) source: Option<PathBuf>,
}

impl Diagnostics {
    pub(super) fn push(&mut self, kind: impl Into<String>, message: impl Into<String>) {
        let kind = kind.into();
        if let Some((_, messages)) = self.blocks.iter_mut().find(|(heading, _)| *heading == kind) {
            messages.push(message.into());
        } else {
            self.blocks.push((kind, vec![message.into()]));
        }
    }

    pub(super) fn messages(&self) -> Vec<String> {
        self.blocks
            .iter()
            .flat_map(|(_, messages)| messages.iter().cloned())
            .collect()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

pub(super) struct Parsed<'a> {
    pub(super) ast: ast::Value<'a>,
    #[cfg(test)]
    pub(super) value: Value,
    pub(super) errors: Vec<String>,
    pub(super) duplicate_fields: Vec<DuplicateField>,
}

#[derive(Clone)]
pub(super) enum PathSegment {
    Field(String),
    Index(usize),
}

pub(super) struct DuplicateField {
    pub(super) location: Vec<PathSegment>,
    pub(super) field: String,
}

pub(super) fn location(segments: &[PathSegment]) -> String {
    let mut result = String::new();
    for (at, segment) in segments.iter().enumerate() {
        match segment {
            PathSegment::Field(field) => {
                if at > 0 {
                    result.push('.');
                }
                result.push_str(field);
            }
            PathSegment::Index(index) => result.push_str(&format!("[{index}]")),
        }
    }
    result
}

pub(super) fn parse(text: &str) -> Result<Parsed<'_>, String> {
    let options = ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    let parsed = parse_to_ast(
        text,
        &CollectOptions {
            tokens: true,
            comments: CommentCollectionStrategy::AsTokens,
        },
        &options,
    )
    .map_err(|e| e.to_string())?;
    // The scanner also skips JSON5 whitespace; restrict gaps between tokens to JSON.
    let mut end = 0;
    for token in parsed.tokens.as_ref().unwrap() {
        if !text[end..token.range.start]
            .chars()
            .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
        {
            return Err(format!("non-JSON whitespace at byte {end}"));
        }
        end = token.range.end;
    }
    if !text[end..]
        .chars()
        .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
    {
        return Err(format!("non-JSON whitespace at byte {end}"));
    }
    let root = parsed.value.ok_or("expected JSON value")?;
    let mut errors = Vec::new();
    let mut duplicate_fields = Vec::new();
    let _value = convert(
        &root,
        text,
        &mut Vec::new(),
        &mut errors,
        &mut duplicate_fields,
    )?;
    Ok(Parsed {
        ast: root,
        #[cfg(test)]
        value: _value,
        errors,
        duplicate_fields,
    })
}

fn convert(
    node: &ast::Value<'_>,
    text: &str,
    path: &mut Vec<PathSegment>,
    errors: &mut Vec<String>,
    duplicate_fields: &mut Vec<DuplicateField>,
) -> Result<Value, String> {
    match node {
        ast::Value::Object(object) => {
            let mut map = serde_json::Map::new();
            for property in &object.properties {
                let range = property.name.range();
                let name: String = serde_json::from_str(&text[range.start..range.end])
                    .map_err(|e| e.to_string())?;
                path.push(PathSegment::Field(name.clone()));
                let value = convert(&property.value, text, path, errors, duplicate_fields)?;
                path.pop();
                if map.insert(name.clone(), value).is_some() {
                    let location = location(path);
                    errors.push(format!(
                        "${}{}: duplicate field {name}",
                        if path.is_empty() || matches!(path.first(), Some(PathSegment::Index(_))) {
                            ""
                        } else {
                            "."
                        },
                        location
                    ));
                    duplicate_fields.push(DuplicateField {
                        location: path.clone(),
                        field: name,
                    });
                }
            }
            Ok(Value::Object(map))
        }
        ast::Value::Array(array) => array
            .elements
            .iter()
            .enumerate()
            .map(|(i, value)| {
                path.push(PathSegment::Index(i));
                let value = convert(value, text, path, errors, duplicate_fields);
                path.pop();
                value
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        _ => {
            // Enforce JSON's exact primitive grammar as well as parser options.
            let range = node.range();
            serde_json::from_str(&text[range.start..range.end]).map_err(|e| e.to_string())
        }
    }
}
