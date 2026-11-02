//! Explicit source envelope adapter. Native File::open never accepts JSONC or
//! source root aliases. Raw input is archived alongside the reviewed mapping.
use super::File;
use crate::{
    Error,
    history::{ApplicationReviewToken, Session},
    key_binding_preferences::source_profile::{self, Platform},
};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, sync::Arc};
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const GROUPS: [(&str, &str); 13] = [
    ("PreferencesAudio", "Audio"),
    ("PreferencesWaveForm", "Waveform"),
    ("PreferencesDark", "Dark"),
    ("PreferencesScroll", "Scroll"),
    ("PreferencesSelection", "Selection"),
    ("PreferencesMiniMap", "MiniMap"),
    ("PreferencesOptions", "Options"),
    ("PreferencesReceptors", "Receptors"),
    ("PreferencesStream", "Stream"),
    ("PreferencesDensityGraph", "DensityGraph"),
    ("PreferencesKeyBinds", "KeyBinds"),
    ("PreferencesMultiplayer", "NoteColor"),
    ("PreferencesPerformance", "Performance"),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Applied,
    Defaulted,
    Retained,
    Normalized,
    EnumFallback,
    Adapted,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub source_token: Option<std::sync::Arc<str>>,
    pub path: String,
    pub disposition: Disposition,
    pub raw: Option<Value>,
    pub effective: Option<Value>,
    pub reason: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub omitted_items: usize,
    pub items: Vec<Item>,
    pub key_bindings: Option<source_profile::Report>,
}
impl Report {
    fn push(&mut self, item: Item) {
        if self.items.len() < 4096 {
            self.items.push(item)
        } else {
            self.omitted_items += 1;
        }
    }
    pub fn requires_acknowledgement(&self) -> bool {
        self.omitted_items != 0
            || self
                .items
                .iter()
                .any(|i| matches!(i.disposition, Disposition::Retained | Disposition::Adapted))
            || self.key_bindings.as_ref().is_some_and(|r| {
                !r.rejected.is_empty()
                    || !r.unavailable_commands.is_empty()
                    || !r.neutralized_native_extensions.is_empty()
                    || !r.retained_modifiers.is_empty()
                    || !r.retained_unknown.is_empty()
            })
    }
}
#[derive(Clone)]
pub struct Snapshot {
    file: Arc<File>,
}
impl Snapshot {
    pub fn capture(file: &File) -> Self {
        Self {
            file: Arc::new(file.clone()),
        }
    }
    pub fn file(&self) -> &File {
        &self.file
    }
    pub fn same_values(&self, other: &Self) -> bool {
        self.file.settings == other.file.settings
    }
}
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationPreferencesSnapshot")
            .field("path", &self.file.path)
            .finish_non_exhaustive()
    }
}
pub struct Draft {
    focus: Option<crate::chart_order::Binding>,
    token: ApplicationReviewToken,
    before: Snapshot,
    target: Snapshot,
    source: Arc<[u8]>,
    report: Box<Report>,
}
impl Draft {
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn raw_source(&self) -> &[u8] {
        &self.source
    }
    pub fn target(&self) -> &File {
        self.target.file()
    }
    pub(crate) fn into_change(
        self,
        accept_adaptations: bool,
    ) -> Result<transaction::Change, Error> {
        if !accept_adaptations {
            return Err(invalid(
                "Explicit acceptance of the source import report is required",
            ));
        }
        transaction::Change::prepare(&self.before, &self.target)
    }
    pub fn is_current(&self, session: &Session, current: &File, focus: Option<usize>) -> bool {
        if !session.accepts_application_review(&self.token) || current.path != self.target.file.path
        {
            return false;
        }
        if !match (&self.focus, focus) {
            (None, None) => true,
            (Some(binding), Some(chart)) => binding.is_current(session, chart),
            _ => false,
        } {
            return false;
        }
        let roots = (
            self.before.file.projected_root(),
            self.target.file.projected_root(),
            current.projected_root(),
        );
        match roots {
            (Ok(before), Ok(after), Ok(current)) => transaction::Patch::between(&before, &after)
                .apply(&current, true)
                .is_ok(),
            _ => false,
        }
    }
}
fn invalid(s: impl Into<String>) -> Error {
    Error::InvalidFormat(s.into())
}
fn source_root(raw: &[u8]) -> Result<Vec<u8>, Error> {
    if raw.len() > MAX_SOURCE_BYTES {
        return Err(invalid("Source preferences exceed the 8 MiB input limit"));
    }
    let input = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(raw);
    let normalized = crate::generation::jsonc::normalize(input)?;
    // Original System.Text.Json defaults to maximum depth64, including Skip of
    // unknown values. Count structural tokens only outside strings.
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for &b in &normalized {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > 64 {
                        return Err(invalid("Source preferences exceed original JSON depth64"));
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    let root: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
        serde_json::from_slice(&normalized).map_err(|e| invalid(e.to_string()))?;
    let native_groups = GROUPS
        .iter()
        .filter(|(_, native)| root.get(*native).is_some_and(|v| v.get().starts_with('{')))
        .count();
    let source_groups = GROUPS.iter().any(|(source, _)| root.contains_key(*source));
    if root
        .get("Version")
        .and_then(|v| serde_json::from_str::<u64>(v.get()).ok())
        == Some(1)
        && native_groups >= 3
        && !source_groups
    {
        return Err(invalid(
            "Native Version1 preferences require the native preferences loader",
        ));
    }
    Ok(normalized)
}
#[derive(Clone, Copy)]
struct EnumSpec {
    field: &'static str,
    names: &'static [&'static str],
    invalid_string: i32,
}
fn enum_value(raw: &Value, spec: EnumSpec) -> (Value, bool, bool) {
    // Actual Fumen converter returns zero for non-string tokens, not the
    // registered invalid-string default. Enum.TryParse supports numeric strings
    // and comma-combined names, case-insensitively.
    let parsed = raw.as_str().map(|s| s.trim()).and_then(|s| {
        s.parse::<i32>().ok().or_else(|| {
            s.split(',').try_fold(0, |value, part| {
                spec.names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(part.trim()))
                    .map(|i| value | i as i32)
            })
        })
    });
    let fallback = raw.is_string() && parsed.is_none();
    let value = parsed.unwrap_or(if raw.is_string() {
        spec.invalid_string
    } else {
        0
    });
    match usize::try_from(value).ok().and_then(|i| spec.names.get(i)) {
        Some(name) => (Value::String((*name).into()), fallback, false),
        None => (Value::String(value.to_string()), fallback, true),
    }
}
fn normalize_enums(
    fields: &mut Map<String, Value>,
    group: &str,
    specs: &[EnumSpec],
    report: &mut Report,
) {
    for &spec in specs {
        if let Some(raw) = fields.get(spec.field).cloned() {
            let (value, fallback, unknown) = enum_value(&raw, spec);
            let accumulation_unknown = unknown && matches!(group, "PreferencesDensityGraph" | "PreferencesStream") && spec.field == "AccumulationType";
            let supported_unknown = accumulation_unknown || (group=="PreferencesDark"&&matches!(spec.field,"Size"|"DarkBgDrawOrder")) || (group == "PreferencesScroll" && spec.field == "SpacingMode") || (group=="PreferencesWaveForm" && matches!(spec.field,"WaveFormSparseColorOption"|"WaveFormDrawLocation")) || (group=="PreferencesMiniMap" && spec.field=="MiniMapSpacingModeForVariable");
            if fallback || unknown || raw != value {
                report.push(Item{source_token:None,path:if group.is_empty(){spec.field.into()}else{format!("{group}.{}",spec.field)},disposition:if accumulation_unknown && group == "PreferencesStream" {Disposition::Adapted}else if unknown&&!supported_unknown{Disposition::Retained}else if fallback{Disposition::EnumFallback}else{Disposition::Normalized},raw:Some(raw),effective:if unknown&&!supported_unknown{None}else{Some(value.clone())},reason:if accumulation_unknown && group == "PreferencesStream" {"Unknown accumulation uses Row on reference rebuild; native recomputes Row consistently after edits instead of retaining reference stale incremental streams"}else if unknown&&!supported_unknown{"Representable source enum has no implemented native consumer; raw value retained"}else{"Original permissive enum normalization"}.into()});
            }
            if unknown && !supported_unknown {
                fields.remove(spec.field);
            } else {
                fields.insert(spec.field.into(), value);
            }
        }
    }
}
fn decode_source(
    raw: &std::collections::BTreeMap<String, &serde_json::value::RawValue>,
    schema: &Map<String, Value>,
    report: &mut Report,
) -> Result<Map<String, Value>, Error> {
    let mut source = Map::new();
    for (source_name, native_name) in GROUPS {
        let Some(raw_group) = raw.get(source_name) else {
            continue;
        };
        let fields: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
            serde_json::from_str(raw_group.get())
                .map_err(|e| invalid(format!("Invalid source {source_name}: {e}")))?;
        let mut shape = schema
            .get(native_name)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if source_name=="PreferencesMultiplayer"{
            if let Some(value)=shape.remove("ColorMultiplayerHoldsAndRolls"){shape.insert("ColorHoldsAndRolls".into(),value);}
        }
        if source_name == "PreferencesPerformance" {
            crate::performance::preferences::Preferences::default().write(&mut shape)?;
        }
        if source_name == "PreferencesWaveForm" {shape.remove("RowBasedWaveFormScrollMode");}
        if source_name == "PreferencesScroll" {
            shape.extend(
                schema["Zoom"]
                    .as_object()
                    .expect("typed Zoom schema")
                    .clone(),
            );
            shape.insert("SpacingMode".into(), Value::String("ConstantTime".into()));
            shape.insert(
                "RowBasedWaveFormScrollMode".into(),
                Value::String("MostCommonTempo".into()),
            );
        }
        let mut result = Map::new();
        if source_name == "PreferencesKeyBinds" {
            // Existing adapter handles its own model/limits; decode only recognized
            // source IDs/modifier descriptors, so arbitrary unknown JSON remains opaque.
            let defaults: Value = serde_json::from_str(include_str!(
                "../key_binding_preferences/source_defaults.json"
            ))
            .map_err(|e| invalid(e.to_string()))?;
            let names = defaults
                .as_object()
                .expect("source platforms")
                .values()
                .filter_map(Value::as_object)
                .flat_map(|m| m.keys().cloned())
                .collect::<BTreeSet<_>>();
            for (key, value) in fields {
                if names.contains(&key) {
                    result.insert(
                        key.clone(),
                        decode_key_bindings(value, &format!("{source_name}.{key}"), report)?,
                    );
                } else {
                    report_retained(
                        report,
                        format!("{source_name}.{key}"),
                        Value::String(value.get().into()),
                        "Unknown key binding JSON archived without decoding",
                    );
                }
            }
        } else {
            for (key, value) in fields {
                let path = format!("{source_name}.{key}");
                if let Some(template) = shape.get(&key) {
                    let enum_spec = specs(source_name)
                        .into_iter()
                        .find(|spec| spec.field == key);
                    let decoded = if source_name == "PreferencesPerformance"
                        && key == "FrameMaxTime"
                    {
                        let parsed = enum_token(value, &path)?;
                        Value::String(
                            crate::performance::preferences::Scale::read_source(Some(&parsed))?
                                .source_name(),
                        )
                    } else if (enum_spec.is_some()
                        || matches!(
                            key.as_str(),
                            "DefaultStepsType" | "DefaultDifficultyType" | "StepColorMethodValue"
                        ))
                        && !value.get().starts_with('"')
                    {
                        if value.get().starts_with('{') || value.get().starts_with('[') {
                            return Err(invalid(format!("Invalid structured source enum {path}")));
                        }
                        Value::Null
                    } else if matches!(
                        key.as_str(),
                        "PositionOffsets"
                            | "DensityGraphPositionOffsets"
                            | "DensityGraphWidthOffsets"
                    ) {
                        Value::Object(Map::new())
                    } else {
                        decode_known(value, template, &path, report)?
                    };
                    let native_key=if source_name=="PreferencesMultiplayer"&&key=="ColorHoldsAndRolls"{"ColorMultiplayerHoldsAndRolls".into()}else{key};
                    result.insert(native_key, decoded);
                } else {
                    report_retained(
                        report,
                        path,
                        Value::String(value.get().into()),
                        "Unconsumed source field retained as raw JSON",
                    );
                }
            }
        }
        source.insert(source_name.into(), Value::Object(result));
    }
    for key in [
        "NoteEntryMode",
        "SnapIndex",
        "SnapLockIndex",
        "Player",
        "RecentFiles",
        "OpenFileDialogInitialDirectory",
        "LastSelectedAutogenChartType",
        "LastSelectedAutogenPerformedChartConfig",
        "RequireIdenticalTimingInSmFiles",
        "OmitChartTimingData",
        "OmitCustomSaveData",
        "AnonymizeSaveData",
        "UseStepF2ForPumpRoutine",
        "UseOutfoxFormatForSmx",
        "ShowLogWindow",
        "LogWindowLevel",
        "LogWindowDateDisplay",
        "LogWindowLineWrap",
        "LastCompletedFtueVersion",
        "FtueIndex",
    ] {
        if let Some(value) = raw.get(key) {
            let parsed = if key == "LogWindowLevel" {
                enum_token(value, key)?
            } else if matches!(key, "NoteEntryMode" | "LastSelectedAutogenChartType")
                && !value.get().starts_with('"')
            {
                if value.get().starts_with('{') || value.get().starts_with('[') {
                    return Err(invalid("Invalid structured source NoteEntryMode"));
                }
                Value::Null
            } else if key == "RecentFiles" {
                decode_known(
                    value,
                    &serde_json::json!([{"FileName":null,"SongName":null,"PackName":null,"SpacingZoom":1.0,"ChartPosition":0.0,"FocusedChartIndex":-1,"ActiveCharts":[{"ChartType":"dance_single","ChartDifficultyType":"Beginner","Rating":0,"Name":null,"Description":null}],"LastChartType":"dance_single","LastChartDifficultyType":"Beginner"}]),
                    key,
                    report,
                )?
            } else {
                serde_json::from_str(value.get())
                    .map_err(|e| invalid(format!("Invalid source {key}: {e}")))?
            };
            source.insert(key.into(), parsed);
        }
    }
    Ok(source)
}
fn decode_key_bindings(
    raw: &serde_json::value::RawValue,
    path: &str,
    report: &mut Report,
) -> Result<Value, Error> {
    let normalized = crate::key_binding_preferences::source_post_load::normalize(raw, path)?;
    for removed in normalized.removed {
        report.push(Item {
            source_token: Some(removed.source_token),
            path: removed.path,
            disposition: Disposition::Normalized,
            raw: None,
            effective: None,
            reason: "Original PreferencesKeyBinds.PostLoad removes invalid key".into(),
        });
    }
    let mut value = normalized.value;
    // Existing callback adapter has a separate explicit16-alternative policy.
    // Preserve one over-limit alternative for its diagnostic; original rawbytes
    // and unrestricted held-query normalization remain intact.
    if !crate::key_binding_preferences::scroll_modifiers::Kind::ALL
        .iter()
        .any(|kind| path.rsplit('.').next() == Some(kind.source_name()))
    {
        if let Value::Array(alternatives) = &mut value {
            alternatives.truncate(17);
        }
    }
    Ok(value)
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
fn decode_known(
    raw: &serde_json::value::RawValue,
    template: &Value,
    path: &str,
    report: &mut Report,
) -> Result<Value, Error> {
    if matches!(
        path.rsplit('.').next().unwrap_or(path),
        "ChartType" | "ChartDifficultyType" | "LastChartType" | "LastChartDifficultyType"
    ) {
        return enum_token(raw, path);
    }
    if path.ends_with(".StartupStepGraphs") {
        let values: Vec<&serde_json::value::RawValue> =
            serde_json::from_str(raw.get()).map_err(|e| invalid(e.to_string()))?;
        return values
            .into_iter()
            .map(|value| enum_token(value, path))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    match template {
        Value::Object(shape) => {
            let fields: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
                serde_json::from_str(raw.get())
                    .map_err(|e| invalid(format!("Invalid source {path}: {e}")))?;
            let mut result = Map::new();
            for (key, value) in fields {
                if let Some(child) = shape.get(&key) {
                    result.insert(
                        key.clone(),
                        decode_known(value, child, &format!("{path}.{key}"), report)?,
                    );
                } else {
                    report_retained(
                        report,
                        format!("{path}.{key}"),
                        Value::String(value.get().into()),
                        "Unknown nested source JSON archived without decoding",
                    );
                }
            }
            Ok(Value::Object(result))
        }
        Value::Array(shape) if !shape.is_empty() => {
            let values: Vec<&serde_json::value::RawValue> = serde_json::from_str(raw.get())
                .map_err(|e| invalid(format!("Invalid source {path}: {e}")))?;
            let mut result = Vec::with_capacity(values.len());
            for (i, value) in values.into_iter().enumerate() {
                result.push(decode_known(
                    value,
                    &shape[0],
                    &format!("{path}[{i}]"),
                    report,
                )?);
            }
            Ok(Value::Array(result))
        }
        _ => serde_json::from_str(raw.get())
            .map_err(|e| invalid(format!("Invalid source {path}: {e}"))),
    }
}
const SPACING: &[&str] = &["ConstantTime", "ConstantRow", "Variable"];
const WAVE_SCROLL: &[&str] = &["CurrentTempo", "CurrentTempoAndRate", "MostCommonTempo"];
const POSITIONS: &[&str] = &[
    "RightSideOfWindow",
    "LeftSideOfWindow",
    "FocusedChartWithoutScaling",
    "FocusedChartWithScaling",
];
const DENSITY_POSITIONS: &[&str] = &[
    "RightSideOfWindow",
    "LeftSideOfWindow",
    "FocusedChartWithoutScaling",
    "FocusedChartWithScaling",
    "TopOfFocusedChart",
    "BottomOfFocusedChart",
];
fn specs(group: &str) -> Vec<EnumSpec> {
    let make = |field, names, invalid_string| EnumSpec {
        field,
        names,
        invalid_string,
    };
    match group {
        "PreferencesScroll" => vec![
            make("SpacingMode", SPACING, 0),
            make("RowBasedWaveFormScrollMode", WAVE_SCROLL, 0),
        ],
        "PreferencesDark" => vec![
            make("Size", &["Charts", "Window"], 1),
            make("DarkBgDrawOrder", &["AfterBackground", "AfterWaveForm"], 0),
        ],
        "PreferencesSelection" => vec![
            make("Mode", &["OverlapAny", "OverlapCenter", "OverlapAll"], 1),
            make(
                "RegionMode",
                &["TimeOrPosition", "TimeOrPositionAndLane"],
                0,
            ),
        ],
        "PreferencesMiniMap" => vec![
            make(
                "MiniMapSelectMode",
                &["MoveToCursor", "MoveToSelectedArea"],
                0,
            ),
            make("MiniMapPosition", POSITIONS, 1),
            make("MiniMapSpacingModeForVariable", SPACING, 0),
        ],
        "PreferencesDensityGraph" => vec![
            make("AccumulationType", &["Step", "Row"], 0),
            make(
                "DensityGraphColorModeValue",
                &["ColorByDensity", "ColorByHeight"],
                1,
            ),
            make("DensityGraphPositionValue", DENSITY_POSITIONS, 1),
        ],
        "PreferencesStream" => vec![
            make("AccumulationType", &["Step", "Row"], 0),
            make(
                "NoteType",
                &[
                    "QuarterNotes",
                    "EighthNotes",
                    "EighthNoteTriplets",
                    "SixteenthNotes",
                    "SixteenthNoteTriplets",
                    "ThirtySecondNotes",
                    "ThirtySecondNoteTriplets",
                    "SixtyFourthNotes",
                    "OneHundredNinetySecondNotes",
                ],
                0,
            ),
        ],
        "PreferencesWaveForm" => vec![
            make(
                "WaveFormSparseColorOption",
                &["DarkerDenseColor", "SameAsDenseColor", "UniqueColor"],
                0,
            ),
            make(
                "WaveFormDrawLocation",
                &["FocusedChart", "AllCharts", "AllChartsWithSameMusic"],
                0,
            ),
        ],
        // ChartType, difficulty and StepColorMethod already implement the original
        // permissive conversion in their typed native readers, including Unknown.
        "PreferencesOptions" => vec![make("BackgroundImageSize", &["ChartArea", "Window"], 0)],
        _ => vec![],
    }
}
fn group(root: &Map<String, Value>, name: &str) -> Result<Map<String, Value>, Error> {
    match root.get(name) {
        None => Ok(Map::new()),
        Some(Value::Object(fields)) => Ok(fields.clone()),
        _ => Err(invalid(format!("Source {name} must be an object"))),
    }
}
fn vector_defaults(value: &mut Value, template: &Value) {
    if let (Value::Object(value), Value::Object(template)) = (value, template) {
        let components = ["X", "Y", "Z", "W"];
        if template.contains_key("X")
            && template
                .keys()
                .all(|key| components.contains(&key.as_str()))
        {
            for component in template.keys() {
                value.entry(component.clone()).or_insert(Value::from(0.0));
            }
        } else {
            for (key, child) in value {
                if let Some(template) = template.get(key) {
                    vector_defaults(child, template);
                }
            }
        }
    }
}
fn ordered_object(raw: &str) -> Result<Vec<(String, Box<serde_json::value::RawValue>)>, Error> {
    struct Pairs;
    impl<'de> serde::de::Visitor<'de> for Pairs {
        type Value = Vec<(String, Box<serde_json::value::RawValue>)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an enum-keyed object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut pairs = Vec::new();
            while let Some(pair) = map.next_entry()? {
                pairs.push(pair);
            }
            Ok(pairs)
        }
    }
    let mut parser = serde_json::Deserializer::from_str(raw);
    serde::de::Deserializer::deserialize_map(&mut parser, Pairs).map_err(|e| invalid(e.to_string()))
}
fn normalize_dictionary(
    fields: &mut Map<String, Value>,
    raw_group: Option<&serde_json::value::RawValue>,
    group: &str,
    key: &str,
    names: &'static [&'static str],
    report: &mut Report,
) -> Result<(), Error> {
    let Some(value) = fields.get(key) else {
        return Ok(());
    };
    if !value.is_object() {
        return Err(invalid(format!(
            "Source {group}.{key} must be an enum dictionary"
        )));
    }
    let raw_fields: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
        serde_json::from_str(
            raw_group
                .ok_or_else(|| invalid("Missing original source group"))?
                .get(),
        )
        .map_err(|e| invalid(e.to_string()))?;
    let raw_dictionary = raw_fields
        .get(key)
        .ok_or_else(|| invalid("Missing original enum dictionary"))?;
    let input = ordered_object(raw_dictionary.get())?;
    let mut result = Map::new();
    for (name, value) in &input {
        let spec = EnumSpec {
            field: "",
            names,
            invalid_string: 0,
        };
        let raw = Value::String(name.clone());
        let (normalized, fallback, unknown) = enum_value(&raw, spec);
        if fallback || unknown {
            report_retained(
                report,
                format!("{group}.{key}.{name}"),
                Value::String(value.get().into()),
                if fallback {
                    "Original enum dictionary converter ignores invalid named keys"
                } else {
                    "Source enum dictionary entry has no native position consumer"
                },
            );
            if fallback {
                continue;
            }
        }
        let offset: i32 = serde_json::from_str(value.get())
            .map_err(|e| invalid(format!("Invalid source {group}.{key}.{name}: {e}")))?;
        result.insert(
            normalized.as_str().expect("normalized enum name").into(),
            Value::from(offset),
        );
    }
    fields.insert(key.into(), Value::Object(result));
    Ok(())
}
fn report_retained(report: &mut Report, path: String, raw: Value, reason: &str) {
    let field = path.rsplit('.').next().unwrap_or(&path);
    let on_demand_window =
        field.starts_with("Show") && field.ends_with("Window") && field != "ShowLogWindow";
    report.push(Item{source_token:None,path,disposition:if on_demand_window{Disposition::Adapted}else{Disposition::Retained},raw:Some(raw),effective:None,reason:if on_demand_window{"Source window visibility retained; native advanced panels open on demand rather than recreating dock windows".into()}else{reason.into()}});
}
impl Draft {
    pub fn prepare(
        session: &mut Session,
        focus: Option<usize>,
        current: &File,
        raw: &[u8],
        platform: Platform,
    ) -> Result<Self, Error> {
        let normalized_json = source_root(raw)?;
        let raw_root: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
            serde_json::from_slice(&normalized_json).map_err(|e| invalid(e.to_string()))?;
        let mut report = Report::default();
        let defaults = File::from_root(
            current.path(),
            None,
            Map::from_iter([("Version".into(), Value::from(1))]),
        )?;
        let mut schema = defaults.projected_root()?;
        // Change-aware serialization can omit a constructor-default field.
        // The source input schema must still recognize that field on import.
        schema.get_mut("Options").and_then(Value::as_object_mut)
            .expect("typed Options schema")
            .insert("SuppressExternalSongModificationNotification".into(), Value::Bool(false));
        let source = decode_source(&raw_root, &schema, &mut report)?;
        let mut target = current.projected_root()?;
        let mut normalized = Map::new();
        let mut consumed = BTreeSet::new();
        for (source_name, native_name) in GROUPS {
            consumed.insert(source_name.to_owned());
            if native_name == "KeyBinds" {
                continue;
            }
            let mut fields = group(&source, source_name)?;
            // Loading accepts unknown ordinals, but the original stream consumer indexes
            // ValidDenominators with this value and throws. Refuse before any publication.
            if source_name == "PreferencesStream" {
                if let Some(raw) = fields.get("NoteType") {
                    let spec = specs(source_name).into_iter().find(|s| s.field == "NoteType").expect("stream NoteType schema");
                    let (value, _, unknown) = enum_value(raw, spec);
                    if unknown {
                        return Err(Error::Unsupported(format!("Unsafe source stream NoteType: {}", value.as_str().unwrap_or("?"))));
                    }
                }
            }
            normalize_enums(&mut fields, source_name, &specs(source_name), &mut report);
            if source_name=="PreferencesWaveForm" && fields.get("WaveFormLoadingMaxParallelism").and_then(Value::as_i64)==Some(0){
                let value=schema["Waveform"]["WaveFormLoadingMaxParallelism"].clone();
                fields.insert("WaveFormLoadingMaxParallelism".into(),value.clone());
                report.push(Item{source_token:None,path:"PreferencesWaveForm.WaveFormLoadingMaxParallelism".into(),disposition:Disposition::Normalized,raw:Some(0.into()),effective:Some(value),reason:"Source PostLoad replaces zero with the initialized runtime default".into()});
            }
            if source_name == "PreferencesReceptors" {
                for key in ["PositionX","PositionY"] {
                    if let Some(value)=fields.get(key).and_then(Value::as_i64) {
                        // Int32 JSON conversion precedes the source setter.
                        // Never turn an overflowing source integer into valid0.
                        if i32::try_from(value).is_ok() && value<0 {
                            fields.insert(key.into(),Value::from(0));
                            report.push(Item{source_token:None,path:format!("{source_name}.{key}"),disposition:Disposition::Normalized,raw:Some(value.into()),effective:Some(0.into()),reason:"Original receptor constructor clamps negative coordinates to zero before Editor binding".into()});
                        }
                    }
                }
            }
            if source_name == "PreferencesMiniMap" {
                normalize_dictionary(
                    &mut fields,
                    raw_root.get(source_name).copied(),
                    source_name,
                    "PositionOffsets",
                    POSITIONS,
                    &mut report,
                )?;
            }
            if source_name == "PreferencesDensityGraph" {
                for field in ["DensityGraphPositionOffsets", "DensityGraphWidthOffsets"] {
                    normalize_dictionary(
                        &mut fields,
                        raw_root.get(source_name).copied(),
                        source_name,
                        field,
                        DENSITY_POSITIONS,
                        &mut report,
                    )?;
                }
            }

            let mut native_defaults = schema
                .get(native_name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if source_name == "PreferencesPerformance" {
                crate::performance::preferences::Preferences::default()
                    .write(&mut native_defaults)?;
            }
            let mut mapped = target
                .get(native_name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, default) in &native_defaults {
                if source_name=="PreferencesWaveForm"&&key=="RowBasedWaveFormScrollMode"{continue;}
                let mut value = fields.get(key).cloned().unwrap_or_else(|| default.clone());
                vector_defaults(&mut value, default);
                let source_key=if source_name=="PreferencesMultiplayer"&&key=="ColorMultiplayerHoldsAndRolls"{"ColorHoldsAndRolls"}else{key.as_str()};
                report.push(Item {
                    source_token: None,
                    path: format!("{source_name}.{source_key}"),
                    disposition: if source_name=="PreferencesReceptors"&&matches!(key.as_str(),"PositionX"|"PositionY"|"AllowChartMoveWhenPositionLocked"){Disposition::Adapted}
                    else if fields.contains_key(key) && key.starts_with("Show") && key.ends_with("Window") {
                        Disposition::Adapted
                    } else if fields.contains_key(key) {
                        Disposition::Applied
                    } else if source.get(source_name).and_then(|g| g.get(key)).is_some() {
                        Disposition::Adapted
                    } else {
                        Disposition::Defaulted
                    },
                    // decode_source has already renamed aliases to native keys;
                    // source_key is only the public source spelling in path.
                    raw: source.get(source_name).and_then(|g| g.get(key)).cloned(),
                    effective: Some(value.clone()),
                    reason: if source_name=="PreferencesReceptors"&&matches!(key.as_str(),"PositionX"|"PositionY"){
                        "Constructor-normalized source chart-space integer preserved; projected relative to source default (842,110) onto centered native canvas and receptor inset, with reverse-Y reflection and viewport-only clamping"
                    }else if source_name=="PreferencesReceptors"&&key=="AllowChartMoveWhenPositionLocked"{
                        "Source header-drag lock exception consumed by explicit keyboard header-move control; horizontal centering still forbids movement"
                    }else if fields.contains_key(key) && key.starts_with("Show") && key.ends_with("Window") {
                        "Source window visibility retained; native advanced panels open on demand rather than recreating dock windows"
                    } else if fields.contains_key(key) {
                        "Mapped to implemented typed consumer"
                    } else if source.get(source_name).and_then(|g| g.get(key)).is_some() {
                        "Native default for unconsumed source value; review required"
                    } else {
                        "Original group constructor default"
                    }
                    .into(),
                });
                mapped.insert(key.clone(), value);
            }
            for (key, value) in &fields {
                if !native_defaults.contains_key(key)
                    && !(source_name == "PreferencesScroll"
                        && (schema["Zoom"]
                            .as_object()
                            .is_some_and(|m| m.contains_key(key))
                            || key == "SpacingMode"
                            || key == "RowBasedWaveFormScrollMode"))
                {
                    report_retained(
                        &mut report,
                        format!("{source_name}.{key}"),
                        value.clone(),
                        "Source field has no consumer in this mapped native group",
                    );
                }
            }
            normalized.insert(source_name.into(), Value::Object(fields));
            target.insert(native_name.into(), Value::Object(mapped));
        }
        // Zoom properties are owned by source Scroll, not a fictitious source Zoom.
        let scroll = normalized["PreferencesScroll"]
            .as_object()
            .expect("normalized source group");
        let mut zoom = target
            .get("Zoom")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, default) in schema["Zoom"].as_object().expect("typed Zoom schema") {
            let value = scroll.get(key).cloned().unwrap_or_else(|| default.clone());
            report.push(Item {
                source_token: None,
                path: format!("PreferencesScroll.{key}"),
                disposition: if scroll.contains_key(key) {
                    Disposition::Applied
                } else {
                    Disposition::Defaulted
                },
                raw: source
                    .get("PreferencesScroll")
                    .and_then(|g| g.get(key))
                    .cloned(),
                effective: Some(value.clone()),
                reason: "Source Scroll property consumed by native Zoom owner".into(),
            });
            zoom.insert(key.clone(), value);
        }
        target.insert("Zoom".into(), Value::Object(zoom));
        target
            .get_mut("Scroll")
            .and_then(Value::as_object_mut)
            .expect("typed Scroll schema")
            .insert(
                "SpacingMode".into(),
                scroll
                    .get("SpacingMode")
                    .cloned()
                    .unwrap_or(Value::String("ConstantTime".into())),
            );

        // Source RowBasedWaveFormScrollMode belongs to Scroll but drives waveform too.
        let wave_mode = scroll
            .get("RowBasedWaveFormScrollMode")
            .cloned()
            .unwrap_or(Value::String("MostCommonTempo".into()));
        report.push(Item{source_token:None,path:"PreferencesScroll.RowBasedWaveFormScrollMode".into(),disposition:if scroll.contains_key("RowBasedWaveFormScrollMode"){Disposition::Applied}else if source.get("PreferencesScroll").and_then(|g|g.get("RowBasedWaveFormScrollMode")).is_some(){Disposition::Adapted}else{Disposition::Defaulted},raw:source.get("PreferencesScroll").and_then(|g|g.get("RowBasedWaveFormScrollMode")).cloned(),effective:Some(wave_mode.clone()),reason:"Source Scroll property consumed by native Waveform owner".into()});
        target
            .get_mut("Waveform")
            .and_then(Value::as_object_mut)
            .expect("typed Waveform schema")
            .insert("RowBasedWaveFormScrollMode".into(), wave_mode);
        let key_source=serde_json::to_string(&serde_json::json!({"PreferencesKeyBinds":source.get("PreferencesKeyBinds").cloned().unwrap_or(Value::Object(Map::new()))})).map_err(|e|invalid(e.to_string()))?;
        let keys = source_profile::import(&current.settings.key_bindings, &key_source, platform)?;
        let mut key_fields = target
            .get("KeyBinds")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        keys.preferences.write(&mut key_fields)?;
        target.insert("KeyBinds".into(), Value::Object(key_fields));
        report.key_bindings = Some(keys.report);
        // Root constructor defaults, explicit even when the native envelope previously
        // had no corresponding key or used different legacy snap indexing.
        for (key, default) in [
            ("NoteEntryMode", Value::String("Normal".into())),
            ("SnapIndex", Value::from(0)),
            ("SnapLockIndex", Value::from(0)),
            ("Player", Value::from(0)),
            ("RecentFiles", Value::Array(vec![])),
            ("OpenFileDialogInitialDirectory", Value::Null),
            (
                "LastSelectedAutogenChartType",
                Value::String("dance_single".into()),
            ),
            (
                "LastSelectedAutogenPerformedChartConfig",
                Value::String(crate::autogen_preferences::BALANCED.into()),
            ),
        ] {
            consumed.insert(key.into());
            let value = source.get(key).cloned().unwrap_or(default);
            let value=if key=="RecentFiles"{
                // Source SavedSongInformation.PostLoad runs once at import. A
                // later native File::open must preserve intentional chartless
                // ActiveCharts=[] rather than synthesize a phantom chart.
                let mut root=Map::from_iter([("RecentFiles".into(),value)]);
                crate::recent_song_preferences::Preferences::read_source(&root)?.write(&mut root);
                root.remove("RecentFiles").expect("serialized recent owner")
            }else{value};
            report.push(Item {
                source_token: None,
                path: key.into(),
                disposition: if source.contains_key(key) {
                    Disposition::Applied
                } else {
                    Disposition::Defaulted
                },
                raw: source.get(key).cloned(),
                effective: Some(value.clone()),
                reason: "Source root preference mapped to its native consumer".into(),
            });
            target.insert(key.into(), value);
        }
        let mut root_mode =
            Map::from_iter([("NoteEntryMode".into(), target["NoteEntryMode"].clone())]);
        normalize_enums(
            &mut root_mode,
            "",
            &[EnumSpec {
                field: "NoteEntryMode",
                names: &["Normal", "AdvanceBySnap"],
                invalid_string: 0,
            }],
            &mut report,
        );
        target.insert(
            "NoteEntryMode".into(),
            root_mode
                .remove("NoteEntryMode")
                .unwrap_or(Value::String("Normal".into())),
        );
        defaults.settings.advanced_save.write(&mut target);
        for key in [
            "RequireIdenticalTimingInSmFiles",
            "OmitChartTimingData",
            "OmitCustomSaveData",
            "AnonymizeSaveData",
            "UseStepF2ForPumpRoutine",
            "UseOutfoxFormatForSmx",
        ] {
            consumed.insert(key.into());
            if let Some(value) = source.get(key) {
                target.insert(key.into(), value.clone());
            }
            report.push(Item {
                source_token: None,
                path: key.into(),
                disposition: if source.contains_key(key) {
                    Disposition::Applied
                } else {
                    Disposition::Defaulted
                },
                raw: source.get(key).cloned(),
                effective: target.get(key).cloned(),
                reason: "Source advanced save preference mapped to export owner".into(),
            });
        }

        let log = crate::log_preferences::Preferences::read_source(&source)?;
        for (key, effective) in [
            ("ShowLogWindow", Value::Bool(log.show)),
            ("LogWindowLevel", Value::String(log.level.source_name())),
            ("LogWindowDateDisplay", Value::from(log.date)),
            ("LogWindowLineWrap", Value::Bool(log.wrap)),
        ] {
            consumed.insert(key.into());
            let unsupported = key == "LogWindowDateDisplay" && log.date_format().is_err();
            let on_demand_window = key == "ShowLogWindow" && source.contains_key(key);
            report.push(Item{source_token:None,path:key.into(),disposition:if unsupported || on_demand_window{Disposition::Adapted}else if source.contains_key(key){Disposition::Applied}else{Disposition::Defaulted},raw:source.get(key).cloned(),effective:Some(effective.clone()),reason:if on_demand_window{"Source window visibility retained; native advanced panels open on demand rather than recreating dock windows"}else if unsupported{"Unknown source date ordinal preserved; native log reports unsupported date display"}else{"Mapped to the existing application log owner"}.into()});
            target.insert(key.into(), effective);
        }
        crate::ftue::Preferences::read(&source)?;
        for (key, default) in [
            ("LastCompletedFtueVersion", Value::Null),
            ("FtueIndex", Value::from(0)),
        ] {
            let effective = source.get(key).cloned().unwrap_or(default);
            consumed.insert(key.into());
            report.push(Item {
                source_token: None,
                path: key.into(),
                disposition: if source.contains_key(key) {
                    Disposition::Applied
                } else {
                    Disposition::Defaulted
                },
                raw: source.get(key).cloned(),
                effective: Some(effective.clone()),
                reason: "Mapped to native FTUE progression owner".into(),
            });
            target.insert(key.into(), effective);
        }
        for (key, value) in &raw_root {
            if !consumed.contains(key) {
                report_retained(
                    &mut report,
                    key.clone(),
                    Value::String(value.get().into()),
                    "Raw source JSON is archived without a native consumer (not decoded)",
                );
            }
        }
        // Capture original token text for exact top-level/group report rows. Parse
        // each raw group once; never reparse a large object once per report line.
        let mut raw_groups = std::collections::BTreeMap::new();
        for (source_name, _) in GROUPS {
            if let Some(value) = raw_root.get(source_name) {
                if let Ok(fields) = serde_json::from_str::<
                    std::collections::BTreeMap<String, &serde_json::value::RawValue>,
                >(value.get())
                {
                    raw_groups.insert(source_name, fields);
                }
            }
        }
        for item in &mut report.items {
            if item.source_token.is_none() {
                let token = raw_root.get(&item.path).copied().or_else(|| {
                    let (group, field) = item.path.split_once('.')?;
                    raw_groups.get(group)?.get(field).copied()
                });
                if let Some(token) = token {
                    item.source_token = Some(std::sync::Arc::from(token.get()));
                    if item.reason.contains("raw JSON") {
                        item.raw = None;
                    }
                }
            }
        }
        let utf8 = std::str::from_utf8(raw).map_err(|e| invalid(e.to_string()))?;
        let mut archive = target
            .get("GrooveAuthorSourceArchive")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        archive.insert(
            "SourceCommit".into(),
            Value::String("48e18f71655d0f0d86144c63239331833501fda2".into()),
        );
        archive.insert("RawUtf8".into(), Value::String(utf8.into()));
        target.insert("GrooveAuthorSourceArchive".into(), Value::Object(archive));
        let focus_binding = focus
            .map(|chart| crate::chart_order::Binding::capture(session, chart))
            .transpose()?;
        // Source RefreshPlayerLimitFromChart clamps the persisted global player to
        // the focused chart. Chartless raw values remain available for a later open.
        if let Some(chart) = focus {
            let players = session.grid(chart)?.players();
            let raw_player = target
                .get("Player")
                .and_then(Value::as_i64)
                .ok_or_else(|| invalid("Invalid source Player"))?;
            let effective = raw_player.clamp(0, i64::from(players.saturating_sub(1)));
            if effective != raw_player {
                report.push(Item {
                    source_token: None,
                    path: "Player".into(),
                    disposition: Disposition::Normalized,
                    raw: Some(Value::from(raw_player)),
                    effective: Some(Value::from(effective)),
                    reason: "Original focused-chart player limit clamps the persisted player"
                        .into(),
                });
            }
            target.insert("Player".into(), Value::from(effective));
        }
        let mut candidate = File::from_root(current.path(), current.expected.clone(), target)?;
        candidate.original = current.original.clone();
        candidate.written = current.written.clone();
        Ok(Self {
            focus: focus_binding,
            token: session.application_review_token(),
            before: Snapshot::capture(current),
            target: Snapshot::capture(&candidate),
            source: Arc::from(raw),
            report: Box::new(report),
        })
    }
}

pub(crate) mod transaction;
pub use transaction::Update;
