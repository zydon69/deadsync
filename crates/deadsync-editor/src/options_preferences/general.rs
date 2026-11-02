//! Source Options fields that have no other typed preference owner.
//! Existing Display, Recent, note-color and minimap owners stay independent.
use crate::{Error, document::chart_type};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq)]
pub struct General {
    pub undo_history_size: i32,
    pub default_steps_type: i32,
    pub default_difficulty: i32,
    pub startup_graphs: BTreeSet<i32>,
    pub open_last: bool,
    pub new_sync: f64,
    pub open_sync: f64,
    pub custom_dpi: bool,
    pub dpi_scale: f64,
    pub suppress_update: bool,
    pub suppress_pack: bool,
    pub suppress_opus: bool,
}
impl Default for General {
    fn default() -> Self {
        Self {
            undo_history_size: 1024,
            default_steps_type: 0,
            default_difficulty: 4,
            startup_graphs: [0, 1].into_iter().collect(),
            open_last: true,
            new_sync: 0.009,
            open_sync: 0.009,
            custom_dpi: false,
            dpi_scale: 1.,
            suppress_update: false,
            suppress_pack: false,
            suppress_opus: false,
        }
    }
}
impl General {
    /// Direct FTUE controls share the same changed-leaf CAS without adding Undo.
    pub fn merge_changes(&self, expected: &Self, target: &Self) -> Result<Self, Error> {
        let (mut current, mut before, mut after) = (Map::new(), Map::new(), Map::new());
        self.write(&mut current)?;
        expected.write(&mut before)?;
        target.write(&mut after)?;
        crate::audio_preferences::publication::merge_fields(&mut current, &before, &after)?;
        Self::read(&current)
    }

    pub const NAMES: [&'static str; 12] = [
        "UndoHistorySize",
        "DefaultStepsType",
        "DefaultDifficultyType",
        "StartupStepGraphs",
        "OpenLastOpenedFileOnLaunch",
        "NewSongSyncOffset",
        "OpenSongSyncOffset",
        "UseCustomDpiScale",
        "DpiScale",
        "SuppressUpdateNotification",
        "SuppressExternalPackModificationNotification",
        "SuppressOpusCodecUsageNotification",
    ];
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let defaults = Self::default();
        let opening = crate::open_preferences::Preferences::read(fields)?;
        let startup_graphs = match fields.get("StartupStepGraphs") {
            None => defaults.startup_graphs.clone(),
            Some(Value::Array(values)) => values
                .iter()
                .map(|value| {
                    let mut one = Map::new();
                    one.insert("DefaultStepsType".into(), value.clone());
                    crate::open_preferences::Preferences::read(&one).map(|p| p.steps_type_value())
                })
                .collect::<Result<BTreeSet<_>, _>>()?,
            Some(_) => return Err(invalid("StartupStepGraphs must be an array")),
        };
        Ok(Self {
            undo_history_size: integer(fields, "UndoHistorySize", defaults.undo_history_size)?,
            default_steps_type: opening.steps_type_value(),
            default_difficulty: opening.difficulty_value(),
            startup_graphs,
            open_last: boolean(fields, "OpenLastOpenedFileOnLaunch", defaults.open_last)?,
            new_sync: number(fields, "NewSongSyncOffset", defaults.new_sync)?,
            open_sync: number(fields, "OpenSongSyncOffset", defaults.open_sync)?,
            custom_dpi: boolean(fields, "UseCustomDpiScale", defaults.custom_dpi)?,
            dpi_scale: number(fields, "DpiScale", defaults.dpi_scale)?,
            suppress_update: boolean(
                fields,
                "SuppressUpdateNotification",
                defaults.suppress_update,
            )?,
            suppress_pack: boolean(
                fields,
                "SuppressExternalPackModificationNotification",
                defaults.suppress_pack,
            )?,
            suppress_opus: boolean(
                fields,
                "SuppressOpusCodecUsageNotification",
                defaults.suppress_opus,
            )?,
        })
    }
    pub fn write(&self, fields: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        fields.insert("UndoHistorySize".into(), self.undo_history_size.into());
        fields.insert(
            "DefaultStepsType".into(),
            step_name(self.default_steps_type).into(),
        );
        fields.insert(
            "DefaultDifficultyType".into(),
            difficulty_name(self.default_difficulty).into(),
        );
        fields.insert(
            "StartupStepGraphs".into(),
            self.startup_graphs
                .iter()
                .map(|value| Value::String(step_name(*value)))
                .collect::<Vec<_>>()
                .into(),
        );
        for (name, value) in [
            ("OpenLastOpenedFileOnLaunch", self.open_last),
            ("UseCustomDpiScale", self.custom_dpi),
            ("SuppressUpdateNotification", self.suppress_update),
            (
                "SuppressExternalPackModificationNotification",
                self.suppress_pack,
            ),
            ("SuppressOpusCodecUsageNotification", self.suppress_opus),
        ] {
            fields.insert(name.into(), value.into());
        }
        for (name, value) in [
            ("NewSongSyncOffset", self.new_sync),
            ("OpenSongSyncOffset", self.open_sync),
            ("DpiScale", self.dpi_scale),
        ] {
            fields.insert(name.into(), value.into());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), Error> {
        if [self.new_sync, self.open_sync, self.dpi_scale]
            .into_iter()
            .all(f64::is_finite)
        {
            Ok(())
        } else {
            Err(invalid("Nonfinite General option"))
        }
    }
    /// Source UI offers1..32768; retain raw imported values independently.
    pub fn effective_history_size(&self) -> usize {
        self.undo_history_size.clamp(1, 32768) as usize
    }
    pub fn set_history_size(&mut self, value: i32) {
        self.undo_history_size = value.clamp(1, 32768);
    }
    pub fn set_new_sync(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(invalid("Nonfinite NewSongSyncOffset"));
        }
        self.new_sync = value.max(0.);
        Ok(())
    }
    pub fn set_open_sync(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(invalid("Nonfinite OpenSongSyncOffset"));
        }
        self.open_sync = value.max(0.);
        Ok(())
    }
    pub fn step_default_type(&mut self, forward: bool) {
        // Same14source choices, plus the two user-approved Couple extensions.
        let supported = &crate::chart_order::TYPE_ORDER;
        let current = usize::try_from(self.default_steps_type)
            .ok()
            .and_then(|i| chart_type::ALL.get(i))
            .and_then(|kind| supported.iter().position(|candidate| candidate == kind))
            .unwrap_or(0);
        let selected =
            supported[(current + if forward { 1 } else { supported.len() - 1 }) % supported.len()];
        self.default_steps_type = chart_type::ALL
            .iter()
            .position(|kind| *kind == selected)
            .expect("canonical supported type") as i32;
    }
    pub fn step_default_difficulty(&mut self, forward: bool) {
        let current = self.default_difficulty.clamp(0, 5);
        self.default_difficulty = (current + if forward { 1 } else { 5 }) % 6;
    }
}
fn step_name(value: i32) -> String {
    usize::try_from(value)
        .ok()
        .and_then(|i| chart_type::ALL.get(i))
        .map(|name| name.replace('-', "_"))
        .unwrap_or_else(|| value.to_string())
}
fn difficulty_name(value: i32) -> String {
    usize::try_from(value)
        .ok()
        .and_then(|i| crate::chart_identity::Difficulty::ALL.get(i))
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| value.to_string())
}
fn integer(fields: &Map<String, Value>, name: &str, default: i32) -> Result<i32, Error> {
    match fields.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| invalid(name)),
    }
}
fn boolean(fields: &Map<String, Value>, name: &str, default: bool) -> Result<bool, Error> {
    match fields.get(name) {
        None => Ok(default),
        Some(value) => value.as_bool().ok_or_else(|| invalid(name)),
    }
}
fn number(fields: &Map<String, Value>, name: &str, default: f64) -> Result<f64, Error> {
    match fields.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| invalid(name)),
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidFormat(format!("Invalid General Options: {message}"))
}

pub(crate) mod history;
pub use history::Update;
