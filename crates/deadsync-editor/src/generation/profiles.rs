//! GrooveAuthor AutogenConfigs wrappers. Root member order is significant:
//! editor property setters can change the underlying generation configuration.
use super::{
    config::{ExpressedConfig, PatternConfig, PerformedConfig},
    metadata::{Guid, Registry},
};
use crate::{Error, storage::sync_directory};
use serde::{
    Deserialize, Serialize,
    de::{MapAccess, Visitor},
    ser::SerializeMap,
};
use serde_json::{Value, value::RawValue};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
const SUBDIVISIONS: [u32; 9] = [1, 2, 3, 4, 6, 8, 12, 16, 48];
const SUBDIVISION_NAMES: [&str; 9] = [
    "quarterNotes",
    "eighthNotes",
    "eighthNoteTriplets",
    "sixteenthNotes",
    "sixteenthNoteTriplets",
    "thirtySecondNotes",
    "thirtySecondNoteTriplets",
    "sixtyFourthNotes",
    "oneHundredNinetySecondNotes",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Pattern,
    Performed,
    Expressed,
}
impl Kind {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Pattern => "pc-",
            Self::Performed => "pcc-",
            Self::Expressed => "ecc-",
        }
    }
    pub fn from_path(path: &Path) -> Option<Self> {
        if path.extension()?.to_str()? != "json" {
            return None;
        }
        let name = path.file_name()?.to_str()?;
        [Self::Pattern, Self::Performed, Self::Expressed]
            .into_iter()
            .find(|kind| name.starts_with(kind.prefix()))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Config {
    Pattern(PatternConfig),
    Performed(PerformedConfig),
    Expressed(ExpressedConfig),
}
impl Config {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Pattern(_) => Kind::Pattern,
            Self::Performed(_) => Kind::Performed,
            Self::Expressed(_) => Kind::Expressed,
        }
    }
    /// GrooveAuthor editor defaults, distinct from deserialization/library defaults.
    pub fn editor_defaults(kind: Kind) -> Self {
        match kind {
            Kind::Pattern => Self::Pattern(PatternConfig::sixteenths()),
            Kind::Performed => Self::Performed(PerformedConfig::balanced()),
            Kind::Expressed => Self::Expressed(ExpressedConfig {
                min_level_for_brackets: 7,
                aggressive_when_uncoverable: true,
                balanced_bpm_for_aggressive: 3.0,
                balanced_bpm_for_no_brackets: 1.0,
                ..ExpressedConfig::default()
            }),
        }
    }
    pub fn json(&self) -> Result<Value, Error> {
        match self {
            Self::Pattern(c) => serde_json::to_value(c),
            Self::Performed(c) => serde_json::to_value(c),
            Self::Expressed(c) => serde_json::to_value(c),
        }
        .map_err(json_error)
    }
    pub(crate) fn validate(&mut self) -> Result<(), Error> {
        match self {
            Self::Pattern(c) => c.validate(),
            Self::Performed(c) => {
                c.initialize();
                c.validate()
            }
            Self::Expressed(c) => c.validate(),
        }
    }
}
#[derive(Clone, Debug)]
struct Members(Vec<(String, Box<RawValue>)>);
impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Members;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an editor profile object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Members, M::Error> {
                let mut values = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    values.push(entry);
                }
                Ok(Members(values))
            }
        }
        d.deserialize_map(Entries)
    }
}
impl Serialize for Members {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TravelSpeed {
    pub subdivision_index: usize,
    pub min_bpm: i32,
    pub max_bpm: i32,
}
impl Default for TravelSpeed {
    fn default() -> Self {
        Self {
            subdivision_index: 0,
            min_bpm: 1,
            max_bpm: 1,
        }
    }
}
impl TravelSpeed {
    pub const SUBDIVISIONS: [u32; 9] = SUBDIVISIONS;
    pub fn editor_defaults() -> Self {
        Self {
            subdivision_index: 3,
            min_bpm: 125,
            max_bpm: 170,
        }
    }

    pub fn intervals(self) -> Result<(f64, f64), Error> {
        if self.subdivision_index >= SUBDIVISIONS.len()
            || !(1..=1000).contains(&self.min_bpm)
            || !(1..=1000).contains(&self.max_bpm)
            || self.min_bpm > self.max_bpm
        {
            return invalid(
                "Travel speed needs an ordered BPM range from 1 to 1000 and a valid subdivision",
            );
        }
        Ok(self.original_intervals())
    }
    fn original_intervals(self) -> (f64, f64) {
        let subdivision = SUBDIVISIONS[self.subdivision_index] as i32;
        (
            60.0 / f64::from(subdivision.wrapping_mul(self.max_bpm)) * 2.0,
            60.0 / f64::from(subdivision.wrapping_mul(self.min_bpm)) * 2.0,
        )
    }
}
#[derive(Clone, Debug)]
pub struct Profile {
    guid: Guid,
    name: Option<String>,
    description: Option<String>,
    config: Config,
    members: Members,
    travel_speed: TravelSpeed,
}
impl Profile {
    pub fn parse(kind: Kind, bytes: &[u8]) -> Result<Self, Error> {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let members: Members =
            serde_json::from_slice(&super::jsonc::normalize(bytes)?).map_err(json_error)?;
        Self::from_members(kind, members)
    }
    fn from_members(kind: Kind, mut members: Members) -> Result<Self, Error> {
        let mut guid = None;
        let mut name = None;
        let mut description = None;
        let mut config = match kind {
            Kind::Pattern => Config::Pattern(PatternConfig::default()),
            Kind::Performed => Config::Performed(PerformedConfig::default()),
            Kind::Expressed => Config::Expressed(ExpressedConfig::default()),
        };
        let (mut speed_index, mut min_bpm, mut max_bpm) = (0usize, 1i32, 1i32);
        for (key, value) in &members.0 {
            match key.as_str() {
                "Guid" => guid = Some(read::<Guid>(value)?),
                "Name" => name = read(value)?,
                "Description" => description = read(value)?,
                "Config" => {
                    config = match kind {
                        Kind::Pattern => Config::Pattern(known(value)?),
                        Kind::Performed => Config::Performed(known(value)?),
                        Kind::Expressed => Config::Expressed(known(value)?),
                    }
                }
                "PatternType" if kind == Kind::Pattern => {
                    let index = enum_index(value, &SUBDIVISION_NAMES)?;
                    let subdivision = *SUBDIVISIONS
                        .get(
                            usize::try_from(index)
                                .map_err(|_| invalid_error("Invalid pattern subdivision"))?,
                        )
                        .ok_or_else(|| invalid_error("Invalid pattern subdivision"))?;
                    if let Config::Pattern(c) = &mut config {
                        c.beat_sub_division = subdivision;
                    }
                }
                "StartingFootSpecified" if kind == Kind::Pattern => {
                    let index = enum_index(value, &["left", "right"])?;
                    if let Config::Pattern(c) = &mut config {
                        c.starting_foot_specified = i32::from(index != 0);
                    }
                }
                "TravelSpeedNoteTypeDenominatorIndex"
                | "TravelSpeedMinBPM"
                | "TravelSpeedMaxBPM"
                    if kind == Kind::Performed =>
                {
                    let value: i32 = read(value)?;
                    match key.as_str() {
                        "TravelSpeedNoteTypeDenominatorIndex" => {
                            speed_index = value.clamp(0, 8) as usize
                        }
                        "TravelSpeedMinBPM" => min_bpm = value,
                        _ => max_bpm = value,
                    }
                    if let Config::Performed(c) = &mut config {
                        let (min, max) = TravelSpeed {
                            subdivision_index: speed_index,
                            min_bpm,
                            max_bpm,
                        }
                        .original_intervals();
                        c.step_tightening.speed_min_time_seconds = min;
                        c.step_tightening.speed_max_time_seconds = max;
                    }
                }
                _ => {}
            }
        }
        let guid = guid.unwrap_or_else(Guid::random);
        if guid == Guid::default() {
            return invalid("Editor profiles need a non-empty GUID");
        }
        config.validate()?;
        if !members.0.iter().any(|(key, _)| key == "Guid") {
            members.0.push(("Guid".into(), raw(&guid)?));
        }
        Ok(Self {
            guid,
            name,
            description,
            config,
            members,
            travel_speed: TravelSpeed {
                subdivision_index: speed_index,
                min_bpm,
                max_bpm,
            },
        })
    }
    pub fn new_pattern(name: &str, config: PatternConfig) -> Result<Self, Error> {
        config.validate()?;
        let index = SUBDIVISIONS
            .iter()
            .position(|&s| s == config.beat_sub_division)
            .ok_or_else(|| invalid_error("Invalid pattern subdivision"))?;
        let members = Members(
            vec![
                ("PatternType".into(), Value::from(SUBDIVISION_NAMES[index])),
                (
                    "Guid".into(),
                    serde_json::to_value(Guid::random()).map_err(json_error)?,
                ),
                ("Name".into(), Value::from(name)),
                ("Description".into(), Value::from("")),
                (
                    "Config".into(),
                    serde_json::to_value(config).map_err(json_error)?,
                ),
            ]
            .into_iter()
            .map(|(key, value)| Ok((key, raw(&value)?)))
            .collect::<Result<_, Error>>()?,
        );
        Self::from_members(Kind::Pattern, members)
    }
    pub fn guid(&self) -> &Guid {
        &self.guid
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    pub fn restore_defaults(&mut self) -> Result<bool, Error> {
        let mut candidate = self.clone();
        let mut changed = candidate.set_config(Config::editor_defaults(self.config.kind()))?;
        if self.config.kind() == Kind::Performed {
            changed |= candidate.set_travel_speed(TravelSpeed::editor_defaults())?;
        }
        if self.config.kind() == Kind::Pattern {
            let current = candidate
                .members
                .0
                .iter()
                .rev()
                .find(|(name, _)| name == "PatternType")
                .map(|(_, value)| enum_index(value, &SUBDIVISION_NAMES))
                .transpose()?
                .unwrap_or(0);
            if current != 3 {
                let value = raw(&SUBDIVISION_NAMES[3])?;
                if let Some((_, old)) = candidate
                    .members
                    .0
                    .iter_mut()
                    .rev()
                    .find(|(name, _)| name == "PatternType")
                {
                    *old = value;
                } else {
                    candidate.members.0.push(("PatternType".into(), value));
                }
                let config = candidate.config.clone();
                install_config(&mut candidate.members, &config)?;
                candidate = Self::from_members(Kind::Pattern, candidate.members)?;
                changed = true;
            }
        }
        if changed {
            *self = candidate;
        }
        Ok(changed)
    }
    pub fn travel_speed(&self) -> Option<TravelSpeed> {
        (self.config.kind() == Kind::Performed).then_some(self.travel_speed)
    }
    /// UI-style speed edits retain wrapper values and install the complete
    /// effective Config last, so subsequent reloads cannot undo the edit.
    pub fn set_travel_speed(&mut self, speed: TravelSpeed) -> Result<bool, Error> {
        let Config::Performed(mut config) = self.config.clone() else {
            return invalid("Only movement profiles have travel-speed settings");
        };
        let (min, max) = speed.intervals()?;
        config.step_tightening.speed_min_time_seconds = min;
        config.step_tightening.speed_max_time_seconds = max;
        let mut config = Config::Performed(config);
        config.validate()?;
        if self.travel_speed == speed && self.config == config {
            return Ok(false);
        }
        let mut members = self.members.clone();
        for (key, value) in [
            (
                "TravelSpeedNoteTypeDenominatorIndex",
                raw(&speed.subdivision_index)?,
            ),
            ("TravelSpeedMinBPM", raw(&speed.min_bpm)?),
            ("TravelSpeedMaxBPM", raw(&speed.max_bpm)?),
        ] {
            if let Some((_, old)) = members.0.iter_mut().rev().find(|(name, _)| name == key) {
                *old = value;
            } else {
                members.0.push((key.into(), value));
            }
        }
        install_config(&mut members, &config)?;
        let candidate = Self::from_members(Kind::Performed, members)?;
        if candidate.config != config || candidate.travel_speed != speed {
            return invalid("Travel-speed settings changed during serialization");
        }
        *self = candidate;
        Ok(true)
    }
    pub(crate) fn preset(
        id: &Guid,
        config: Config,
        speed: Option<TravelSpeed>,
    ) -> Result<Self, Error> {
        let mut members = Members(vec![("Guid".into(), raw(id)?)]);
        if let Some(speed) = speed {
            members.0.extend([
                (
                    "TravelSpeedNoteTypeDenominatorIndex".into(),
                    raw(&speed.subdivision_index)?,
                ),
                ("TravelSpeedMinBPM".into(), raw(&speed.min_bpm)?),
                ("TravelSpeedMaxBPM".into(), raw(&speed.max_bpm)?),
            ]);
        }
        members.0.push(("Config".into(), raw(&config.json()?)?));
        Self::from_members(config.kind(), members)
    }
    pub fn filename(&self) -> String {
        format!("{}{}.json", self.config.kind().prefix(), self.guid)
    }
    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        serde_json::to_vec_pretty(&self.members).map_err(json_error)
    }
    /// Unknown wrapper and generation fields stay in the ordered source body.
    /// A rename changes only the last effective Name property.
    pub fn rename(&mut self, name: &str) -> Result<(), Error> {
        self.set_text_member("Name", Some(name))
    }
    /// A semantic no-op preserves missing/null descriptions and all source data.
    pub fn set_description(&mut self, description: Option<&str>) -> Result<bool, Error> {
        if self.description() == description {
            return Ok(false);
        }
        self.set_text_member("Description", description)?;
        Ok(true)
    }
    fn set_text_member(&mut self, key: &str, text: Option<&str>) -> Result<(), Error> {
        let mut members = self.members.clone();
        if let Some((_, value)) = members.0.iter_mut().rev().find(|(name, _)| name == key) {
            *value = raw(&text)?;
        } else {
            members.0.push((key.into(), raw(&text)?));
        }
        *self = Self::from_members(self.config.kind(), members)?;
        Ok(())
    }
    /// Replace the effective typed configuration without discarding vendor data.
    /// The final Config is written after wrapper setters, whose source order can
    /// otherwise silently override the user's edit when the profile is reopened.
    pub fn set_config(&mut self, mut config: Config) -> Result<bool, Error> {
        if config.kind() != self.config.kind() {
            return invalid("Profile configuration kind cannot change");
        }
        config.validate()?;
        if config == self.config {
            return Ok(false);
        }
        let mut members = self.members.clone();
        if let Config::Pattern(pattern) = &config {
            let index = SUBDIVISIONS
                .iter()
                .position(|value| *value == pattern.beat_sub_division)
                .ok_or_else(|| invalid_error("Invalid pattern subdivision"))?;
            for (name, value) in [
                ("PatternType", raw(&SUBDIVISION_NAMES[index])?),
                (
                    "StartingFootSpecified",
                    raw(&pattern.starting_foot_specified)?,
                ),
            ] {
                if let Some((_, existing)) = members.0.iter_mut().rev().find(|(key, _)| key == name)
                {
                    *existing = value;
                } else {
                    members.0.push((name.into(), value));
                }
            }
        }
        install_config(&mut members, &config)?;
        let candidate = Self::from_members(self.config.kind(), members)?;
        if candidate.config != config {
            return invalid("Profile configuration changed during serialization");
        }
        *self = candidate;
        Ok(true)
    }

    /// Create a custom copy, including when the source is a protected preset.
    pub fn duplicate(&self, name: &str) -> Result<Self, Error> {
        let mut copy = self.clone();
        let guid = Guid::random();
        if let Some((_, value)) = copy
            .members
            .0
            .iter_mut()
            .rev()
            .find(|(key, _)| key == "Guid")
        {
            *value = raw(&guid)?;
        }
        copy = Self::from_members(self.config.kind(), copy.members)?;
        copy.rename(name)?;
        Ok(copy)
    }
    pub fn register(&self, registry: &mut Registry) -> Result<(), Error> {
        if builtin(self.config.kind(), &self.guid) {
            return invalid("Built-in editor profiles cannot be overwritten");
        }
        match &self.config {
            Config::Pattern(c) => {
                registry.patterns.insert(self.guid.clone(), c.clone());
            }
            Config::Performed(c) => {
                registry.performed.insert(self.guid.clone(), c.clone());
            }
            Config::Expressed(c) => {
                registry.expressed.insert(self.guid.clone(), c.clone());
            }
        }
        Ok(())
    }
    pub fn export_new(&self, directory: &Path) -> Result<ProfileFile, Error> {
        if builtin(self.config.kind(), &self.guid) {
            return invalid("Built-in editor profiles cannot be overwritten");
        }
        fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        let path = directory.join(self.filename());
        let bytes = self.serialize()?;
        let mut pending = tempfile::NamedTempFile::new_in(&directory)?;
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        pending
            .persist_noclobber(&path)
            .map_err(|error| Error::Io(error.error))?;
        sync_directory(&directory)?;
        Ok(ProfileFile {
            path,
            expected: bytes.into(),
            profile: self.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
}
fn install_config(members: &mut Members, config: &Config) -> Result<(), Error> {
    let schema = match config.kind() {
        Kind::Pattern => Config::Pattern(PatternConfig::default()),
        Kind::Performed => Config::Performed(PerformedConfig::default()),
        Kind::Expressed => Config::Expressed(ExpressedConfig::default()),
    }
    .json()?;
    let original = members
        .0
        .iter()
        .rev()
        .find(|(key, _)| key == "Config")
        .map(|(_, value)| value.clone())
        .unwrap_or(raw(&serde_json::json!({}))?);
    let merged = merge_config(&original, &config.json()?, &schema)?;
    if let Some((_, value)) = members.0.last_mut().filter(|(key, _)| key == "Config") {
        *value = merged;
    } else {
        members.0.push(("Config".into(), merged));
    }
    Ok(())
}
// Only typed object fields are merged. Empty schema objects represent dynamic
// dictionaries: their keys are user data, so removal must remain possible.
fn merge_config(
    original: &RawValue,
    edited: &Value,
    schema: &Value,
) -> Result<Box<RawValue>, Error> {
    let (Some(fields), Some(schema)) = (edited.as_object(), schema.as_object()) else {
        return raw(edited);
    };
    if schema.is_empty() {
        return raw(edited);
    }
    let mut members: Members = read(original)?;
    for (key, value) in fields {
        let child_schema = schema.get(key).unwrap_or(&Value::Null);
        if let Some((_, original)) = members.0.iter_mut().rev().find(|(name, _)| name == key) {
            *original = merge_config(original, value, child_schema)?;
        } else {
            members.0.push((key.clone(), raw(value)?));
        }
    }
    raw(&members)
}
fn enum_index(raw: &RawValue, names: &[&str]) -> Result<i32, Error> {
    let value: Value = read(raw)?;
    if let Some(text) = value.as_str() {
        let text = text.trim();
        if let Ok(index) = text.parse::<i32>() {
            return Ok(index);
        }
        let mut combined = 0;
        for part in text.split(',') {
            let index = names
                .iter()
                .position(|s| s.eq_ignore_ascii_case(part.trim()))
                .ok_or_else(|| invalid_error("Unknown profile enum value"))?;
            combined |= index as i32;
        }
        return Ok(combined);
    }
    serde_json::from_value(value.clone()).map_err(json_error)
}
// System.Text.Json ignores unknown object members. Keep them in Members for
// export while feeding only the current library fields to its typed port.
fn read<T: serde::de::DeserializeOwned>(value: &RawValue) -> Result<T, Error> {
    serde_json::from_str(value.get()).map_err(json_error)
}
fn raw<T: Serialize>(value: &T) -> Result<Box<RawValue>, Error> {
    serde_json::value::to_raw_value(value).map_err(json_error)
}
fn known<T: Default + Serialize + serde::de::DeserializeOwned>(
    value: &RawValue,
) -> Result<T, Error> {
    fn filter(value: &RawValue, schema: &Value) -> Result<Value, Error> {
        if let Value::Object(fields) = schema {
            if !fields.is_empty() {
                let members: Members = read(value)?;
                let mut known = serde_json::Map::new();
                for (key, raw) in members.0 {
                    if let Some(schema) = fields.get(&key) {
                        known.insert(key, filter(&raw, schema)?);
                    }
                }
                return Ok(Value::Object(known));
            }
        }
        read(value)
    }
    serde_json::from_value(filter(
        value,
        &serde_json::to_value(T::default()).map_err(json_error)?,
    )?)
    .map_err(json_error)
}
fn builtin(kind: Kind, guid: &Guid) -> bool {
    static BUILTINS: OnceLock<Registry> = OnceLock::new();
    let registry = BUILTINS.get_or_init(Registry::builtin);
    match kind {
        Kind::Pattern => registry.patterns.contains_key(guid),
        Kind::Performed => registry.performed.contains_key(guid),
        Kind::Expressed => registry.expressed.contains_key(guid),
    }
}
pub struct ProfileFile {
    path: PathBuf,
    expected: Arc<[u8]>,
    profile: Profile,
    undo: Vec<(Arc<[u8]>, Profile)>,
    redo: Vec<(Arc<[u8]>, Profile)>,
}
impl ProfileFile {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let kind = Kind::from_path(path)
            .ok_or_else(|| invalid_error("Unknown editor profile filename"))?;
        let path = path.canonicalize()?;
        let expected: Arc<[u8]> = fs::read(&path)?.into();
        let profile = Profile::parse(kind, &expected)?;
        Ok(Self {
            path,
            expected,
            profile,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
    /// Retain the catalog's parsed identity (including a generated GUID for an
    /// anonymous import) while validating the same source bytes a second time.
    pub(crate) fn capture_snapshot(
        path: &Path,
        expected: Arc<[u8]>,
        profile: Profile,
    ) -> Result<Self, Error> {
        if Kind::from_path(path) != Some(profile.config.kind()) {
            return invalid("Profile filename does not match its configuration kind");
        }
        let path = path.canonicalize()?;
        if fs::read(&path)?.as_slice() != expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        Ok(Self {
            path,
            expected,
            profile,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
    pub(crate) fn snapshot(&self) -> (PathBuf, Arc<[u8]>) {
        (self.path.clone(), self.expected.clone())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
    pub fn save(&mut self, profile: &Profile) -> Result<(), Error> {
        if profile.guid != self.profile.guid || profile.config.kind() != self.profile.config.kind()
        {
            return invalid("Profile identity cannot change during save");
        }
        if builtin(profile.config.kind(), &profile.guid) {
            return invalid("Built-in editor profiles cannot be overwritten");
        }
        let bytes: Arc<[u8]> = profile.serialize()?.into();
        if bytes == self.expected {
            if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
                return Err(Error::ExternalChange);
            }
            return Ok(());
        }
        let previous = (self.expected.clone(), self.profile.clone());
        self.replace_bytes(bytes, profile.clone())?;
        self.undo.push(previous);
        self.redo.clear();
        Ok(())
    }
    /// Remove a custom profile while retaining an exact, private recovery file.
    /// The returned deletion owns that backup until its history is discarded.
    pub fn delete(&self) -> Result<DeletedProfile, Error> {
        if builtin(self.profile.config.kind(), &self.profile.guid) {
            return invalid("Built-in editor profiles cannot be deleted");
        }
        let _lock = profile_lock(&self.path)?;
        if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        let permissions = fs::metadata(&self.path)?.permissions();
        let parent = self.path.parent().unwrap();
        let backup = tempfile::Builder::new()
            .prefix(".deadsync-deleted-")
            .suffix(".profile")
            .tempfile_in(parent)?
            .into_temp_path();
        if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        fs::rename(&self.path, &backup)?;
        Ok(DeletedProfile {
            path: self.path.clone(),
            bytes: self.expected.clone(),
            permissions,
            backup,
            deleted: true,
        })
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Restore exact source bytes, including unknown fields and formatting.
    /// External edits are checked before changing either history stack.
    pub fn undo(&mut self) -> Result<bool, Error> {
        let Some((bytes, profile)) = self.undo.last().cloned() else {
            return Ok(false);
        };
        let current = (self.expected.clone(), self.profile.clone());
        self.replace_bytes(bytes, profile)?;
        self.undo.pop();
        self.redo.push(current);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, Error> {
        let Some((bytes, profile)) = self.redo.last().cloned() else {
            return Ok(false);
        };
        let current = (self.expected.clone(), self.profile.clone());
        self.replace_bytes(bytes, profile)?;
        self.redo.pop();
        self.undo.push(current);
        Ok(true)
    }
    fn replace_bytes(&mut self, bytes: Arc<[u8]>, profile: Profile) -> Result<(), Error> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| invalid_error("Profile has no parent directory"))?;
        let _lock = profile_lock(&self.path)?;
        if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending
            .as_file()
            .set_permissions(fs::metadata(&self.path)?.permissions())?;
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        if fs::read(&self.path)?.as_slice() != self.expected.as_ref() {
            return Err(Error::ExternalChange);
        }
        pending
            .persist(&self.path)
            .map_err(|e| Error::Io(e.error))?;
        self.expected = bytes;
        self.profile = profile;
        sync_directory(parent)?;
        Ok(())
    }
}
/// An undoable disk deletion. Backups are excluded from the profile catalog.
/// Dropping this value discards its private backup; a restored original remains.
pub struct DeletedProfile {
    path: PathBuf,
    bytes: Arc<[u8]>,
    permissions: fs::Permissions,
    backup: tempfile::TempPath,
    deleted: bool,
}
impl DeletedProfile {
    pub(crate) fn preserve_backup(self) -> Result<PathBuf, Error> {
        self.backup.keep().map_err(|error| Error::Io(error.error))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
    pub fn restore(&mut self) -> Result<bool, Error> {
        if !self.deleted {
            return Ok(false);
        }
        let _lock = profile_lock(&self.path)?;
        if fs::read(&self.backup)?.as_slice() != self.bytes.as_ref() {
            return Err(Error::ExternalChange);
        }
        let mut pending = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?;
        pending
            .as_file()
            .set_permissions(self.permissions.clone())?;
        pending.write_all(&self.bytes)?;
        pending.as_file().sync_all()?;
        pending
            .persist_noclobber(&self.path)
            .map_err(|e| Error::Io(e.error))?;
        self.deleted = false;
        Ok(true)
    }
    pub fn delete_again(&mut self) -> Result<bool, Error> {
        if self.deleted {
            return Ok(false);
        }
        let _lock = profile_lock(&self.path)?;
        if fs::read(&self.path)?.as_slice() != self.bytes.as_ref()
            || fs::read(&self.backup)?.as_slice() != self.bytes.as_ref()
        {
            return Err(Error::ExternalChange);
        }
        fs::remove_file(&self.path)?;
        self.deleted = true;
        Ok(true)
    }
}
fn profile_lock(path: &Path) -> Result<fs::File, Error> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("Profile has no parent directory"))?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(format!(
            ".{}.deadsync-save-lock",
            path.file_name().unwrap().to_string_lossy()
        )))?;
    lock.try_lock().map_err(std::io::Error::other)?;
    Ok(lock)
}
#[derive(Clone, Debug)]
pub struct Issue {
    pub path: PathBuf,
    pub message: String,
}
pub struct Loaded {
    pub registry: Registry,
    pub profiles: BTreeMap<(Kind, Guid), ProfileFile>,
    pub issues: Vec<Issue>,
}
impl Loaded {
    pub fn directory(path: &Path) -> Result<Self, Error> {
        Self::directory_checked(path, &AtomicBool::new(false))
    }
    pub fn directory_checked(path: &Path, cancelled: &AtomicBool) -> Result<Self, Error> {
        let check = || {
            if cancelled.load(Ordering::Relaxed) {
                Err(Error::InvalidEdit("Profile loading cancelled".into()))
            } else {
                Ok(())
            }
        };
        check()?;
        let mut loaded = Self {
            registry: Registry::builtin(),
            profiles: BTreeMap::new(),
            issues: Vec::new(),
        };
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(loaded),
            Err(error) => return Err(error.into()),
        };
        let mut paths = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for path in paths {
            check()?;
            if Kind::from_path(&path).is_none() || !path.is_file() {
                continue;
            }
            let file = match ProfileFile::open(&path) {
                Ok(file) => file,
                Err(error) => {
                    loaded.issues.push(Issue {
                        path,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            if let Err(error) = file.profile.register(&mut loaded.registry) {
                loaded.issues.push(Issue {
                    path,
                    message: error.to_string(),
                });
                continue;
            }
            let key = (file.profile.config.kind(), file.profile.guid.clone());
            if let Some(old) = loaded.profiles.insert(key, file) {
                loaded.issues.push(Issue {
                    path: old.path,
                    message: "Duplicate profile GUID; later filename takes precedence".into(),
                });
            }
        }
        check()?;
        Ok(loaded)
    }
}
fn json_error(error: serde_json::Error) -> Error {
    Error::InvalidFormat(format!("Invalid editor profile: {error}"))
}
fn invalid_error(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(invalid_error(message))
}
