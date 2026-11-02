//! Original PreferencesKeyBinds.PostLoad normalization, before native binding policy.
//! Empty alternatives are preserved: source held queries treat them as true.
use crate::Error;
use serde_json::Value;
fn invalid(s: impl Into<String>) -> Error {
    Error::InvalidFormat(s.into())
}
#[derive(Debug)]
pub struct Removed {
    pub path: String,
    pub source_token: std::sync::Arc<str>,
}
#[derive(Debug)]
pub struct Normalized {
    pub value: Value,
    pub removed: Vec<Removed>,
}
#[derive(serde::Deserialize)]
struct SourceKey {
    name: String,
    value: i32,
    valid: bool,
}
pub fn normalize(raw: &serde_json::value::RawValue, path: &str) -> Result<Normalized, Error> {
    let mut removed = Vec::new();
    if raw.get() == "null" {
        return Ok(Normalized {
            value: Value::Null,
            removed,
        });
    }
    let definitions: Vec<SourceKey> = serde_json::from_str(include_str!("source_keys.json"))
        .map_err(|e| invalid(e.to_string()))?;
    let by_name = definitions
        .iter()
        .map(|key| (key.name.to_ascii_lowercase(), key.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    let by_value = definitions
        .iter()
        .map(|key| (key.value, key))
        .collect::<std::collections::BTreeMap<_, _>>();
    let alternatives: Vec<&serde_json::value::RawValue> = serde_json::from_str(raw.get())
        .map_err(|e| invalid(format!("Invalid source {path}: {e}")))?;
    let mut normalized = Vec::with_capacity(alternatives.len().min(17));
    if alternatives.is_empty() {
        return Ok(Normalized {
            value: Value::Array(vec![Value::Array(vec![])]),
            removed,
        });
    }
    for (i, alternative) in alternatives.into_iter().enumerate() {
        let keys: Vec<&serde_json::value::RawValue> = serde_json::from_str(alternative.get())
            .map_err(|e| invalid(format!("Invalid source {path}[{i}]: {e}")))?;
        let mut chord = Vec::new();
        for (j, key) in keys.into_iter().enumerate() {
            let token = enum_token(key, path)?;
            let value = token
                .as_str()
                .map(str::trim)
                .and_then(|s| {
                    s.parse::<i32>().ok().or_else(|| {
                        s.split(',').try_fold(0, |value, name| {
                            by_name
                                .get(&name.trim().to_ascii_lowercase())
                                .map(|n| value | n)
                        })
                    })
                })
                .unwrap_or(0);
            // Actual original IsValidKeyForBinding indexes a fixed255 array; these
            // values fail original PostLoad rather than silently becoming unbound.
            if !(0..255).contains(&value) {
                return Err(invalid(format!(
                    "Source key value outside original key table at {path}[{i}][{j}]"
                )));
            }
            if let Some(definition) = by_value.get(&value).filter(|key| key.valid) {
                chord.push(Value::String(definition.name.clone()));
            } else {
                removed.push(Removed {
                    path: format!("{path}[{i}][{j}]"),
                    source_token: std::sync::Arc::from(key.get()),
                });
            }
        }
        normalized.push(Value::Array(chord));
    }
    Ok(Normalized {
        value: Value::Array(normalized),
        removed,
    })
}
fn enum_token(raw: &serde_json::value::RawValue, path: &str) -> Result<Value, Error> {
    if raw.get().starts_with('"') {
        return serde_json::from_str(raw.get()).map_err(|e| invalid(e.to_string()));
    }
    if raw.get().starts_with('{') || raw.get().starts_with('[') {
        return Err(invalid(format!("Invalid structured source enum {path}")));
    }
    Ok(Value::Null)
}
