//! Explicit source preset/import, never implicit when reading native preferences.
use super::{Action, Preferences};
use crate::{
    Error,
    key_commands::{self, Key},
};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, sync::Arc};
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const ARCHIVE_KEY: &str = "_DeadSyncSourceImportRaw";
pub const BLOCKERS_KEY: &str = "_DeadSyncSourceUnavailableChords";
pub const BLOCKER_PREFIX: &str = "__source_unavailable_";
pub const ORDER_KEY: &str = "_DeadSyncSourceOrder";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Other,
}
impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Other
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Mac => "mac",
            Self::Other => "other",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub id: String,
    pub alternative: Option<usize>,
    pub reason: &'static str,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub platform: Platform,
    pub applied: Vec<String>,
    pub applied_modifiers: Vec<String>,
    pub unavailable_commands: Vec<String>,
    pub retained_modifiers: Vec<String>,
    pub retained_unknown: Vec<String>,
    pub neutralized_native_extensions: Vec<String>,
    pub rescue_commands: Vec<String>,
    pub rejected: Vec<Rejected>,
    pub blocked_unavailable: Vec<String>,
    pub retained_native_contexts: Vec<String>,
}
pub struct Prepared {
    pub preferences: Preferences,
    pub report: Report,
}
fn defaults(platform: Platform) -> Result<Map<String, Value>, Error> {
    let all: Value = serde_json::from_str(include_str!("source_defaults.json"))
        .map_err(|e| Error::InvalidFormat(e.to_string()))?;
    all.get(platform.key())
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| Error::InvalidFormat("Pinned source defaults are missing".into()))
}
fn source_key(name: &str) -> Option<String> {
    // Keys enum parsing in the source is case insensitive. Native Ctrl is not
    // a MonoGame enum value and must not broaden an imported physical chord.
    let fixed = [
        "LeftControl",
        "RightControl",
        "LeftWindows",
        "RightWindows",
        "LeftShift",
        "RightShift",
        "LeftAlt",
        "RightAlt",
        "Space",
        "Enter",
        "NumPad0",
        "Escape",
        "Tab",
        "Back",
        "Delete",
        "Insert",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Up",
        "Down",
        "Left",
        "Right",
        "OemQuestion",
    ];
    let names = fixed
        .into_iter()
        .map(str::to_owned)
        .chain(('A'..='Z').map(|c| c.to_string()))
        .chain((0..=9).map(|i| format!("D{i}")))
        .chain((1..=24).map(|i| format!("F{i}")));
    names
        .into_iter()
        .find(|key| key.eq_ignore_ascii_case(name))
        .map(|key| Key::named(&key).name().to_owned())
}
fn alternatives(id: &str, value: &Value, report: &mut Report) -> Vec<Vec<String>> {
    let reject = |report: &mut Report, alternative, reason| {
        report.rejected.push(Rejected {
            id: id.into(),
            alternative,
            reason,
        })
    };
    let Some(raw) = value.as_array() else {
        reject(report, None, "Expected an alternatives array");
        return vec![vec![]];
    };
    if raw.len() > 16 {
        reject(report, None, "Too many alternatives");
        return vec![vec![]];
    }
    if raw.is_empty() {
        return vec![vec![]];
    }
    let mut result = Vec::new();
    for (index, chord) in raw.iter().enumerate() {
        let Some(chord) = chord.as_array() else {
            reject(report, Some(index), "Expected an ordered key array");
            continue;
        };
        if chord.len() > super::reservations::MAX_CHORD_KEYS {
            reject(report, Some(index), "Chord exceeds native key limit");
            continue;
        }
        let mut keys = Vec::new();
        let mut seen = BTreeSet::new();
        let mut valid = true;
        for key in chord {
            let Some(key) = key.as_str().and_then(source_key) else {
                reject(
                    report,
                    Some(index),
                    "Source key has no supported native mapping",
                );
                valid = false;
                break;
            };
            if !seen.insert(key.clone()) {
                reject(
                    report,
                    Some(index),
                    "Duplicate modifier/key after physical alias normalization",
                );
                valid = false;
                break;
            }
            keys.push(key);
        }
        if valid
            && !Action::from_id(id).map_or_else(
                || super::reservations::supported_chord(&keys),
                |action| action.accepts_chord(&keys),
            )
        {
            reject(report, Some(index), "Unsupported native key chord");
            valid = false;
        }
        if valid && keys.iter().any(|key| key == "F1") {
            reject(report, Some(index), "F1 is reserved for native menu rescue");
            valid = false;
        }
        if valid && super::reservations::conflicts(&keys) {
            reject(
                report,
                Some(index),
                "Chord is owned by an unmigrated native context",
            );
            valid = false;
        }
        // Never remove only an unsupported modifier: that could turn an import
        // into an unsafe bare shortcut. Reject the whole affected alternative.
        if valid {
            result.push(keys);
        }
    }
    if result.is_empty() {
        result.push(vec![]);
    }
    result
}
pub fn native_defaults(current: &Preferences) -> Preferences {
    let mut native = Preferences::default();
    native.source_import_raw = current.source_import_raw.clone();
    native
}
pub fn preset(current: &Preferences, platform: Platform) -> Result<Prepared, Error> {
    let defaults = defaults(platform)?;
    let raw =
        serde_json::to_string_pretty(&defaults).map_err(|e| Error::InvalidFormat(e.to_string()))?;
    import(current, &raw, platform)
}
pub fn import(current: &Preferences, raw: &str, platform: Platform) -> Result<Prepared, Error> {
    current.validate()?;
    if raw.len() > MAX_SOURCE_BYTES {
        return Err(Error::InvalidFormat(
            "Source key bindings exceed the 1 MiB input limit".into(),
        ));
    }
    let input = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let normalized = crate::generation::jsonc::normalize(input.as_bytes())?;
    let root: Value =
        serde_json::from_slice(&normalized).map_err(|e| Error::InvalidFormat(e.to_string()))?;
    let root = root
        .as_object()
        .ok_or_else(|| Error::InvalidFormat("Source preferences must be an object".into()))?;
    let defaults = defaults(platform)?;
    let fields = if let Some(group) = root.get("PreferencesKeyBinds") {
        group
            .as_object()
            .ok_or_else(|| Error::InvalidFormat("PreferencesKeyBinds must be an object".into()))?
    } else if root.keys().any(|key| defaults.contains_key(key)) {
        root
    } else {
        return Err(Error::InvalidFormat(
            "No source PreferencesKeyBinds section found".into(),
        ));
    };
    let source_ids: BTreeSet<_> = key_commands::source::COMMANDS
        .iter()
        .map(|command| command.id)
        .collect();
    let mut report = Report {
        platform,
        applied: vec![],
        applied_modifiers: vec![],
        unavailable_commands: vec![],
        retained_modifiers: vec![],
        retained_unknown: vec![],
        neutralized_native_extensions: vec![],
        rescue_commands: vec![],
        rejected: vec![],
        blocked_unavailable: vec![],
        retained_native_contexts: vec![],
    };
    let mut preferences = Preferences::default();
    preferences.source_import_raw = Some(Arc::from(raw));
    preferences.source_order = true;
    for kind in super::scroll_modifiers::Kind::ALL {
        let id = kind.source_name();
        let value = fields.get(id).or_else(|| defaults.get(id)).ok_or_else(|| {
            Error::InvalidFormat(format!("No original default for source modifier {id}"))
        })?;
        let raw = serde_json::value::RawValue::from_string(
            serde_json::to_string(value).map_err(|e| Error::InvalidFormat(e.to_string()))?,
        )
        .map_err(|e| Error::InvalidFormat(e.to_string()))?;
        let normalized = super::source_post_load::normalize(&raw, id)?;
        preferences.scroll_modifiers.set(
            kind,
            super::scroll_modifiers::Query::from_post_load(&normalized.value)?,
        );
        report.applied_modifiers.push(id.into());
    }
    for action in Action::ALL {
        let id = action.id();
        if source_ids.contains(id) {
            let value = fields.get(id).or_else(|| defaults.get(id)).ok_or_else(|| {
                Error::InvalidFormat(format!("No original default for source command {id}"))
            })?;
            preferences
                .commands
                .insert(id.into(), alternatives(id, value, &mut report));
            report.applied.push(id.into());
        } else if matches!(id, "OpenEditorMenu" | "OpenKeyBindingSettings") {
            // Explicit access rescue for the simplified native UI. Report it;
            // never call these native menu operations original GA callbacks.
            report.rescue_commands.push(id.into());
        } else {
            if current.commands[id].iter().any(|keys| !keys.is_empty()) {
                report.neutralized_native_extensions.push(id.into());
            }
            preferences.commands.insert(id.into(), vec![vec![]]);
        }
    }
    for id in source_ids {
        if Action::from_id(id).is_none() {
            report.unavailable_commands.push(id.into());
            if let Some(value) = fields.get(id).or_else(|| defaults.get(id)) {
                // Reuse strict parsing but keep unsupported callback diagnostics
                // separate: raw data remains archived, no callback is invented.
                let rejected_start = report.rejected.len();
                let chords = alternatives(id, value, &mut report);
                if report.rejected[rejected_start..].iter().any(|entry| {
                    entry.reason == "Chord is owned by an unmigrated native context"
                        || entry.reason == "F1 is reserved for native menu rescue"
                }) {
                    report.retained_native_contexts.push(id.into());
                }
                report.rejected.truncate(rejected_start);
                let chords: Vec<_> = chords.into_iter().filter(|keys| !keys.is_empty()).collect();
                if !chords.is_empty() {
                    report.blocked_unavailable.push(id.into());
                    preferences.source_blockers.insert(id.into(), chords);
                }
            }
        }
    }
    for id in defaults.keys().filter(|id| {
        !key_commands::source::COMMANDS
            .iter()
            .any(|c| c.id == id.as_str())
    }) {
        if !super::scroll_modifiers::Kind::ALL
            .iter()
            .any(|kind| kind.source_name() == id)
        {
            report.retained_modifiers.push(id.clone());
        }
    }
    for id in fields.keys().filter(|id| !defaults.contains_key(*id)) {
        report.retained_unknown.push(id.clone());
    }
    preferences.validate()?;
    Ok(Prepared {
        preferences,
        report,
    })
}

/// Marker IDs are per alternative so unrelated final keys never get consumed.
pub fn blocker_source(id: &str) -> Option<&str> {
    id.strip_prefix(BLOCKER_PREFIX)?
        .rsplit_once('/')
        .map(|(source, _)| source)
}
pub fn blockers(preferences: &Preferences) -> Vec<key_commands::Binding> {
    preferences
        .source_blockers
        .iter()
        .filter(|(id, _)| {
            Action::from_id(id).is_none()
                && key_commands::source::COMMANDS
                    .iter()
                    .any(|command| command.id == id.as_str())
        })
        .flat_map(|(id, alternatives)| {
            alternatives
                .iter()
                .enumerate()
                .map(move |(index, chord)| key_commands::Binding {
                    id: format!("{BLOCKER_PREFIX}{id}/{index}"),
                    alternatives: super::physical::expand(chord),
                    repeat: false,
                    blocks: true,
                })
        })
        .collect()
}
pub fn validate_blockers(preferences: &Preferences) -> Result<(), Error> {
    if preferences.source_blockers.len() > 164
        || preferences
            .source_blockers
            .values()
            .map(Vec::len)
            .sum::<usize>()
            > 256
    {
        return Err(Error::InvalidEdit(
            "Too many unavailable source chords".into(),
        ));
    }
    for (id, alternatives) in &preferences.source_blockers {
        if id.len() > 128 || id.contains(['\0', '/']) || alternatives.len() > 16 {
            return Err(Error::InvalidEdit(
                "Invalid unavailable source command".into(),
            ));
        }
        for chord in alternatives {
            let mut seen = BTreeSet::new();
            if chord.is_empty()
                || chord.len() > super::reservations::MAX_CHORD_KEYS
                || chord.iter().any(|key| {
                    source_key(key).as_deref() != Some(key.as_str())
                        || !seen.insert(Key::named(key))
                })
                || !super::reservations::supported_chord(chord)
                || super::reservations::conflicts(chord)
                || chord.iter().any(|key| key == "F1")
            {
                return Err(Error::InvalidEdit(
                    "Invalid unavailable source chord".into(),
                ));
            }
        }
    }
    Ok(())
}
