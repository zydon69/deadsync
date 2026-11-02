//! One-shot source profile resolution before publishing a loaded document.
use super::{
    metadata::{ChartData, DYNAMIC, Guid, PATTERN_BALANCED, SIXTEENTHS},
    profiles::{Kind, Loaded},
};
use crate::{
    Error,
    document::Document,
    history::{ContentToken, RevisionToken, Session},
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::InvalidEdit("Profile resolution cancelled".into()))
    } else {
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    exists: bool,
    files: BTreeMap<PathBuf, Arc<[u8]>>,
}
fn snapshot(directory: &Path, cancelled: &AtomicBool) -> Result<Snapshot, Error> {
    check(cancelled)?;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Snapshot {
                exists: false,
                files: Default::default(),
            });
        }
        Err(error) => return Err(error.into()),
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut files = BTreeMap::new();
    for path in paths {
        check(cancelled)?;
        if Kind::from_path(&path).is_none() || !path.is_file() {
            continue;
        }
        let mut file = fs::File::open(&path)?;
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16384];
        loop {
            check(cancelled)?;
            let count = file.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        files.insert(path, bytes.into());
    }
    check(cancelled)?;
    Ok(Snapshot {
        exists: true,
        files,
    })
}
#[derive(Clone, Debug)]
pub struct Repair {
    pub chart: usize,
    pub row: u64,
    pub kind: Kind,
    pub previous: Guid,
    pub replacement: Guid,
}
#[derive(Clone)]
pub struct Guard {
    directory: Option<PathBuf>,
    snapshot: Option<Snapshot>,
    baseline: Option<ContentToken>,
    marker_repairs: bool,
    repairs: Vec<Repair>,
}

impl Guard {
    pub(crate) fn repairs(&self) -> &[Repair] {
        &self.repairs
    }
    pub(crate) fn permits_dirty_recovery(&self, session: &Session) -> bool {
        self.marker_repairs
            && self
                .baseline
                .as_ref()
                .is_some_and(|token| session.accepts_content_token(token))
    }
    pub(crate) fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }
    pub(crate) fn after_recovery(&mut self) {
        self.baseline = None;
    }
    pub fn validate_for(
        &self,
        directory: Option<&Path>,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        if directory != self.directory {
            return Err(Error::InvalidEdit(
                "Configured profile directory changed during song loading".into(),
            ));
        }
        self.validate(cancelled)
    }
    pub fn validate(&self, cancelled: &AtomicBool) -> Result<(), Error> {
        check(cancelled)?;
        if let (Some(directory), Some(expected)) = (&self.directory, &self.snapshot) {
            if snapshot(directory, cancelled)? != *expected {
                return Err(Error::ExternalChange);
            }
        }
        Ok(())
    }
}
pub struct Plan {
    token: RevisionToken,
    source: Arc<[u8]>,
    document: Option<Document>,
    marker_repairs: bool,
    guard: Guard,
}
impl Plan {
    pub fn capture(
        session: &Session,
        directory: Option<&Path>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        if session.revision() != 0
            || session.undo_label().is_some()
            || session.redo_label().is_some()
        {
            return Err(Error::InvalidEdit(
                "Profile resolution requires a freshly loaded song".into(),
            ));
        }
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        let before = directory
            .as_deref()
            .map(|path| snapshot(path, cancelled))
            .transpose()?;
        let loaded = if let Some(path) = directory.as_deref() {
            let (loaded, sources) = Loaded::directory_snapshot_checked(path, cancelled)?;
            if before
                .as_ref()
                .is_some_and(|snapshot| snapshot.files != sources)
            {
                return Err(Error::ExternalChange);
            }
            loaded
        } else {
            Loaded {
                registry: super::metadata::Registry::builtin(),
                profiles: Default::default(),
                issues: Vec::new(),
            }
        };
        let mut guard = Guard {
            directory,
            snapshot: before,
            baseline: None,
            marker_repairs: false,
            repairs: Vec::new(),
        };
        guard.validate(cancelled)?;
        let mut temporary = Session::new(Document::parse_with_context(
            Arc::<[u8]>::from(session.serialize()?),
            session.document().format,
            session.document().load_context(),
        )?);
        let mut changed = false;
        let mut marker_repairs = false;
        for chart in 0..temporary.document().charts().len() {
            check(cancelled)?;
            // Unknown-version/malformed custom data retains its existing loader
            // behavior. This contract resolves missing profiles in valid data.
            let Ok(Some(mut data)) = ChartData::read(temporary.document(), chart) else {
                continue;
            };
            let mut repaired = false;
            let mut expression_repaired = false;
            let first_repair = guard.repairs.len();
            if !loaded
                .registry
                .expressed
                .contains_key(&data.expressed_chart_config)
            {
                data.expressed_chart_config = DYNAMIC.parse()?;
                repaired = true;
                expression_repaired = true;
            }
            for (row, definition) in data
                .patterns
                .as_mut()
                .into_iter()
                .flat_map(|patterns| patterns.iter_mut())
            {
                if !loaded
                    .registry
                    .patterns
                    .contains_key(&definition.pattern_config_guid)
                {
                    let replacement: Guid = SIXTEENTHS.parse()?;
                    guard.repairs.push(Repair {
                        chart,
                        row: *row,
                        kind: Kind::Pattern,
                        previous: definition.pattern_config_guid.clone(),
                        replacement: replacement.clone(),
                    });
                    definition.pattern_config_guid = replacement;
                    repaired = true;
                    marker_repairs = true;
                }
                if !loaded
                    .registry
                    .performed
                    .contains_key(&definition.performed_chart_config_guid)
                {
                    let replacement: Guid = PATTERN_BALANCED.parse()?;
                    guard.repairs.push(Repair {
                        chart,
                        row: *row,
                        kind: Kind::Performed,
                        previous: definition.performed_chart_config_guid.clone(),
                        replacement: replacement.clone(),
                    });
                    definition.performed_chart_config_guid = replacement;
                    repaired = true;
                    marker_repairs = true;
                }
            }
            if repaired {
                let (scope, _, name) = super::metadata::tags(temporary.document(), chart)?;
                let json = rewrite_metadata(
                    temporary.document(),
                    chart,
                    expression_repaired,
                    &guard.repairs[first_repair..],
                )?;
                temporary.set_property_bytes_atomic(
                    &[(scope, name, crate::document::escape_text(&json).into())],
                    "Resolve loaded generation profiles",
                )?;
                changed = true;
            }
        }
        check(cancelled)?;
        guard.validate(cancelled)?;
        let document = if changed {
            Some(Document::parse_with_context(
                Arc::<[u8]>::from(temporary.serialize()?),
                session.document().format,
                session.document().load_context(),
            )?)
        } else {
            None
        };
        Ok(Self {
            token: session.revision_token(),
            source: Arc::clone(session.document().source()),
            document,
            marker_repairs,
            guard,
        })
    }
    pub fn apply(mut self, session: &mut Session, cancelled: &AtomicBool) -> Result<Guard, Error> {
        check(cancelled)?;
        self.guard.validate(cancelled)?;
        if !session.accepts_token(&self.token)
            || !Arc::ptr_eq(&self.source, session.document().source())
        {
            return Err(Error::InvalidEdit(
                "Profile resolution belongs to another song or revision".into(),
            ));
        }
        if let Some(document) = self.document {
            session.adopt_profile_load(&self.token, document, self.marker_repairs)?;
        }
        self.guard.marker_repairs = self.marker_repairs;
        self.guard.baseline = Some(session.content_token());
        Ok(self.guard)
    }
}
impl crate::storage::FileSession {
    pub fn resolve_generation_profiles(
        &mut self,
        directory: Option<&Path>,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        check(cancelled)?;
        if self.has_generation_profile_resolution() {
            return Err(Error::InvalidEdit(
                "Generation profiles were already resolved for this load".into(),
            ));
        }
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let plan = Plan::capture(&self.session, directory, cancelled)?;
        if self.has_external_change()? {
            return Err(Error::ExternalChange);
        }
        let guard = plan.apply(&mut self.session, cancelled)?;
        self.install_generation_profile_guard(guard);
        Ok(())
    }
}

// Ordered raw members retain every unknown value and duplicate member. This is
// a surgical edit of the shared on-document owner, not a parallel metadata map.
struct RawObject(Vec<(String, Box<serde_json::value::RawValue>)>);
impl<'de> serde::Deserialize<'de> for RawObject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = RawObject;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a GA metadata object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<RawObject, M::Error> {
                let mut members = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    members.push(entry);
                }
                Ok(RawObject(members))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
impl serde::Serialize for RawObject {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}
fn json_error(error: serde_json::Error) -> Error {
    Error::InvalidFormat(format!("Invalid GA profile reference data: {error}"))
}
impl RawObject {
    fn value(&self, key: &str) -> Option<&serde_json::value::RawValue> {
        self.0
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_ref())
    }
    fn set<T: serde::Serialize>(&mut self, key: &str, value: &T) -> Result<(), Error> {
        let raw = serde_json::value::to_raw_value(value).map_err(json_error)?;
        if let Some((_, old)) = self.0.iter_mut().rev().find(|(name, _)| name == key) {
            *old = raw;
        } else {
            self.0.push((key.to_owned(), raw));
        }
        Ok(())
    }
}
fn rewrite_metadata(
    document: &Document,
    chart: usize,
    expression: bool,
    repairs: &[Repair],
) -> Result<String, Error> {
    let (scope, _, name) = super::metadata::tags(document, chart)?;
    let text = document
        .text(scope, &name)
        .ok_or_else(|| Error::InvalidFormat("Missing GA metadata owner".into()))?;
    let mut object: RawObject =
        serde_json::from_slice(&super::jsonc::strip_comments(text.as_bytes())?)
            .map_err(json_error)?;
    if expression {
        object.set("ExpressedChartConfig", &DYNAMIC.parse::<Guid>()?)?;
    }
    if !repairs.is_empty() {
        let raw = object
            .value("Patterns")
            .ok_or_else(|| Error::InvalidFormat("Missing GA pattern owner".into()))?;
        let mut patterns: RawObject = serde_json::from_str(raw.get()).map_err(json_error)?;
        for repair in repairs {
            let index = patterns
                .0
                .iter()
                .rposition(|(row, _)| row.parse::<u64>().ok() == Some(repair.row))
                .ok_or_else(|| Error::InvalidFormat("Missing GA pattern row".into()))?;
            let mut definition: RawObject =
                serde_json::from_str(patterns.0[index].1.get()).map_err(json_error)?;
            let field = match repair.kind {
                Kind::Pattern => "PatternConfigGuid",
                Kind::Performed => "PerformedChartConfigGuid",
                Kind::Expressed => {
                    return Err(Error::InvalidEdit(
                        "Expression is not a marker profile".into(),
                    ));
                }
            };
            definition.set(field, &repair.replacement)?;
            patterns.0[index].1 =
                serde_json::value::to_raw_value(&definition).map_err(json_error)?;
        }
        object.set("Patterns", &patterns)?;
    }
    let json = serde_json::to_string(&object).map_err(json_error)?;
    ChartData::from_json(&json)?;
    Ok(json)
}
