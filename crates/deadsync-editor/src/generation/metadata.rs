//! GrooveAuthor chart metadata V1 and explicit generation-profile resolution.
//! Field names and built-in GUIDs follow GrooveAuthor 48e18f71 (MIT).
use super::{
    config::{
        BracketDetermination, BracketMethod, ExpressedConfig, PatternConfig, PerformedConfig,
        PerformedPreset,
    },
    job::ChartOptions,
    pattern_job::{Options, Span},
};
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt, str::FromStr};

pub const DYNAMIC: &str = "a19d532e-b0ce-4759-ad1c-02ecbbdf2efd";
pub const SIXTEENTHS: &str = "03de57bd-4329-4de6-a722-a171c11fdd16";
pub const EIGHTHS: &str = "b3779494-f8c2-4a76-be45-b5f6bfcb8fc0";
pub const PATTERN_BALANCED: &str = "41a7f76e-596a-4fbe-bf6e-6a77a898e4a1";

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Guid([u8; 16]);
impl Guid {
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4().into_bytes())
    }
}
impl FromStr for Guid {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        let text = if (text.starts_with('{') && text.ends_with('}'))
            || (text.starts_with('(') && text.ends_with(')'))
        {
            &text[1..text.len() - 1]
        } else {
            text
        };
        if text.len() != 32
            && !(text.len() == 36 && [8, 13, 18, 23].iter().all(|&i| text.as_bytes()[i] == b'-'))
        {
            return invalid("Invalid generation profile GUID");
        }
        let digits = text.bytes().filter(|&b| b != b'-').collect::<Vec<_>>();
        if digits.len() != 32 {
            return invalid("Invalid generation profile GUID");
        }
        let mut bytes = [0; 16];
        for (i, pair) in digits.chunks_exact(2).enumerate() {
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            bytes[i] = hex(pair[0])
                .zip(hex(pair[1]))
                .map(|(a, b)| a * 16 + b)
                .ok_or_else(|| Error::InvalidFormat("Invalid generation profile GUID".into()))?;
        }
        Ok(Self(bytes))
    }
}
impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if [4, 6, 8, 10].contains(&i) {
                write!(f, "-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
impl Serialize for Guid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Guid {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        // System.Text.Json's Guid converter accepts the canonical D format.
        if text.len() != 36 || ![8, 13, 18, 23].iter().all(|&i| text.as_bytes()[i] == b'-') {
            return Err(serde::de::Error::custom(
                "Expected a canonical generation profile GUID",
            ));
        }
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Definition {
    pub pattern_config_guid: Guid,
    pub performed_chart_config_guid: Guid,
    pub length: u64,
    pub random_seed: i32,
    pub start_position_inclusive: bool,
    pub end_position_inclusive: bool,
    pub ignore_preceding_distribution: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ChartData {
    pub music_offset: f64,
    pub should_use_chart_music_offset: bool,
    pub expressed_chart_config: Guid,
    pub patterns: Option<BTreeMap<u64, Definition>>,
    #[serde(skip_serializing_if = "is_zero")]
    pub max_player_index: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
fn is_zero(value: &u32) -> bool {
    *value == 0
}
impl Default for ChartData {
    fn default() -> Self {
        Self {
            music_offset: 0.0,
            should_use_chart_music_offset: false,
            expressed_chart_config: DYNAMIC.parse().unwrap(),
            patterns: None,
            max_player_index: 0,
            extra: BTreeMap::new(),
        }
    }
}
impl ChartData {
    /// Saved markers are selected independently of note endpoint inclusivity.
    /// Among overlapping raw regions, the latest start takes precedence.
    pub fn marker_at(&self, cursor: u64) -> Option<(u64, &Definition)> {
        self.patterns
            .as_ref()?
            .range(..=cursor)
            .rev()
            .find(|(start, marker)| cursor - **start <= marker.length)
            .map(|(&start, marker)| (start, marker))
    }

    /// Strict next/previous start with wraparound, like the original editor navigation.
    pub fn adjacent_marker(&self, cursor: u64, forward: bool) -> Option<u64> {
        use std::ops::Bound::{Excluded, Unbounded};
        let patterns = self.patterns.as_ref()?;
        let entry = if forward {
            patterns
                .range((Excluded(cursor), Unbounded))
                .next()
                .or_else(|| patterns.first_key_value())
        } else {
            patterns
                .range(..cursor)
                .next_back()
                .or_else(|| patterns.last_key_value())
        };
        entry.map(|(&row, _)| row)
    }

    pub fn from_json(json: &str) -> Result<Self, Error> {
        let data: Self = serde_json::from_slice(&super::jsonc::strip_comments(json.as_bytes())?)
            .map_err(|e| {
                Error::InvalidFormat(format!("Invalid GrooveAuthor chart metadata: {e}"))
            })?;
        data.validate()?;
        Ok(data)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.extra.keys().any(|key| {
            [
                "MusicOffset",
                "ShouldUseChartMusicOffset",
                "ExpressedChartConfig",
                "Patterns",
                "MaxPlayerIndex",
            ]
            .contains(&key.as_str())
        }) {
            return invalid("Unknown chart metadata cannot shadow a defined field");
        }
        if !self.music_offset.is_finite() || self.max_player_index > i32::MAX as u32 - 1 {
            return invalid("Invalid GrooveAuthor chart settings");
        }
        if let Some(patterns) = &self.patterns {
            for (&row, definition) in patterns {
                if definition.extra.keys().any(|key| {
                    [
                        "PatternConfigGuid",
                        "PerformedChartConfigGuid",
                        "Length",
                        "RandomSeed",
                        "StartPositionInclusive",
                        "EndPositionInclusive",
                        "IgnorePrecedingDistribution",
                    ]
                    .contains(&key.as_str())
                }) {
                    return invalid("Unknown pattern metadata cannot shadow a defined field");
                }
                if row > i32::MAX as u64
                    || definition.length > i32::MAX as u64
                    || row
                        .checked_add(definition.length)
                        .is_none_or(|end| end > i32::MAX as u64)
                {
                    return invalid("Pattern marker exceeds the reference row limit");
                }
            }
        }
        Ok(())
    }
    pub fn read(document: &Document, chart: usize) -> Result<Option<Self>, Error> {
        let (scope, version, data) = tags(document, chart)?;
        match (document.text(scope, &version), document.text(scope, &data)) {
            (None, None) => Ok(None),
            (Some(version), Some(json)) if version.trim().parse::<i32>().ok() == Some(1) => {
                Self::from_json(&json).map(Some)
            }
            (Some(_), Some(_)) => Err(Error::Unsupported(
                "Unknown GrooveAuthor chart metadata version".into(),
            )),
            _ => invalid("Incomplete GrooveAuthor chart metadata"),
        }
    }
    pub fn existing_or_initial(document: &Document, chart: usize) -> Result<Self, Error> {
        if let Some(data) = Self::read(document, chart)? {
            return Ok(data);
        }
        let mut data = Self::default();
        if let Some(descriptor) = document.charts().get(chart) {
            if descriptor.steps_type == "pump-routine" {
                // Adding unrelated editor metadata must not turn the Routine
                // default of two players into an explicit one-player choice.
                // Metadata-only operations also accept opaque note streams:
                // use the layout default if this codec cannot decode them,
                // and leave their note bytes untouched when saving metadata.
                data.max_player_index = crate::notes::Grid::parse_layout(
                    document.value(descriptor.notes.clone()), &descriptor.steps_type,
                ).map_or(1, |grid| grid.players().max(2) - 1);
            }
        }
        if document.format == Format::Ssc {
            let number = |scope| -> Result<Option<f64>, Error> {
                document
                    .text(scope, "OFFSET")
                    .map(|text| {
                        text.parse::<f64>()
                            .ok()
                            .filter(|value| value.is_finite())
                            .ok_or_else(|| Error::InvalidFormat("Invalid music offset".into()))
                    })
                    .transpose()
            };
            let song = number(None)?.unwrap_or(0.0);
            if let Some(offset) = number(Some(chart))? {
                if !music_offsets_equal(offset, song) {
                    data.music_offset = offset;
                    data.should_use_chart_music_offset = true;
                }
            }
        }
        Ok(data)
    }

    pub fn save(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        self.save_with_offset(session, chart, None, "Change pattern markers")
    }
    pub(crate) fn save_with_offset(
        &self,
        session: &mut Session,
        chart: usize,
        chart_offset: Option<f64>,
        label: &str,
    ) -> Result<bool, Error> {
        self.validate()?;
        if chart_offset.is_some_and(|offset| !offset.is_finite()) {
            return invalid("Music offset must be finite");
        }
        if chart_offset.is_some() && session.document().format != Format::Ssc {
            return invalid("SM does not support a per-chart OFFSET tag");
        }
        if chart_offset.is_some() && !crate::timing_edit::supports_chart_timing(session.document())
        {
            return invalid("Convert legacy SSC timing in F10 before exporting a chart offset");
        }
        Self::read(session.document(), chart)?;
        let (scope, version, data) = tags(session.document(), chart)?;
        let json = serde_json::to_string(self).map_err(|e| Error::InvalidFormat(e.to_string()))?;
        let offset = chart_offset.map(|value| value.to_string());
        let mut properties: Vec<(Option<usize>, String, std::sync::Arc<[u8]>)> = if chart_offset
            .is_some()
        {
            crate::timing_edit::inherited_fields_for_write(session.document(), chart, &["OFFSET"])?
                .into_iter()
                .map(|(name, bytes)| (Some(chart), name, bytes))
                .collect()
        } else {
            Vec::new()
        };
        for (name, value) in [(version.as_str(), "1"), (data.as_str(), json.as_str())] {
            properties.push((
                scope,
                name.to_owned(),
                crate::document::escape_text(value).into(),
            ));
        }
        if let Some(offset) = &offset {
            properties.push((
                scope,
                "OFFSET".into(),
                crate::document::escape_text(offset).into(),
            ));
        }
        session.set_property_bytes_atomic(&properties, label)
    }
    pub fn options(&self, row: u64, registry: &Registry) -> Result<Options, Error> {
        self.validate()?;
        let options = self.options_allow_empty(row, registry)?;
        options
            .span
            .steps(48 / u64::from(options.pattern.beat_sub_division))?;
        Ok(options)
    }
    /// Caller has validated the shared chart metadata once for this request.
    pub(crate) fn options_allow_empty(
        &self,
        row: u64,
        registry: &Registry,
    ) -> Result<Options, Error> {
        let marker = self
            .patterns
            .as_ref()
            .and_then(|p| p.get(&row))
            .ok_or_else(|| Error::InvalidEdit("Pattern marker does not exist".into()))?;
        let options = Options {
            span: Span {
                start: row,
                end: row + marker.length,
                include_start: marker.start_position_inclusive,
                include_end: marker.end_position_inclusive,
            },
            seed: marker.random_seed,
            ignore_preceding_distribution: marker.ignore_preceding_distribution,
            pattern: resolve(&registry.patterns, &marker.pattern_config_guid, "pattern")?,
            performed: resolve(
                &registry.performed,
                &marker.performed_chart_config_guid,
                "performed",
            )?,
            expressed: resolve(
                &registry.expressed,
                &self.expressed_chart_config,
                "expressed",
            )?,
        };
        options.pattern.validate()?;
        options.performed.validate()?;
        options.expressed.validate()?;
        options
            .span
            .optional_steps(48 / u64::from(options.pattern.beat_sub_division))?;
        Ok(options)
    }
}
pub(crate) fn tags(document: &Document, chart: usize) -> Result<(Option<usize>, String, String), Error> {
    if chart >= document.charts().len() {
        return invalid("Pattern chart does not exist");
    }
    let (scope, suffix) = match document.format {
        Format::Ssc => (Some(chart), String::new()),
        Format::Sm => {
            if chart > 9999 {
                return invalid("SM custom chart metadata supports four-digit indices");
            }
            (None, format!("CHART{chart:04}"))
        }
    };
    Ok((
        scope,
        format!("GAChartDataVersion{suffix}"),
        format!("GAChartData{suffix}"),
    ))
}
fn resolve<T: Clone>(map: &BTreeMap<Guid, T>, guid: &Guid, kind: &str) -> Result<T, Error> {
    map.get(guid)
        .cloned()
        .ok_or_else(|| Error::Unsupported(format!("Missing {kind} generation profile {guid}")))
}
#[derive(Default)]
pub struct Registry {
    pub patterns: BTreeMap<Guid, PatternConfig>,
    pub performed: BTreeMap<Guid, PerformedConfig>,
    pub expressed: BTreeMap<Guid, ExpressedConfig>,
}
impl Registry {
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for (guid, config) in [
            (SIXTEENTHS, PatternConfig::sixteenths()),
            (EIGHTHS, PatternConfig::eighths()),
        ] {
            registry.patterns.insert(guid.parse().unwrap(), config);
        }
        for (guid, preset) in [
            (
                "6276c906-ea8f-43b3-9500-0ddeac7bdc22",
                PerformedPreset::Balanced,
            ),
            (
                "aa41f2c5-4da1-41ad-b645-b18ed7c11d47",
                PerformedPreset::StaminaSingle,
            ),
            (
                "c0334922-6105-4703-add2-3de261b2ff19",
                PerformedPreset::StaminaDouble,
            ),
            (PATTERN_BALANCED, PerformedPreset::PatternBalanced),
            (
                "69d041be-ef12-4609-815e-b591d512e259",
                PerformedPreset::PatternNoCandles,
            ),
            (
                "eba3802f-66ec-42d4-87cf-66e9dadc9d0a",
                PerformedPreset::PatternNoInward,
            ),
            (
                "0dde2261-90c5-4413-82a6-9034dad80aa1",
                PerformedPreset::PatternNoTransitionLimit,
            ),
        ] {
            registry
                .performed
                .insert(guid.parse().unwrap(), PerformedConfig::preset(preset));
        }
        registry.expressed.insert(
            DYNAMIC.parse().unwrap(),
            ChartOptions::balanced("dance-single").expressed,
        );
        for (guid, method) in [
            (
                "da3f6e12-49d1-416b-8db6-0ab413f740b6",
                BracketMethod::Aggressive,
            ),
            (
                "0c0ba200-8f90-4060-8912-e9ea65831ebc",
                BracketMethod::NoBrackets,
            ),
        ] {
            registry.expressed.insert(
                guid.parse().unwrap(),
                ExpressedConfig {
                    bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
                    default_bracket_parsing_method: method,
                    min_level_for_brackets: 0,
                    aggressive_when_uncoverable: false,
                    balanced_bpm_for_aggressive: 0.0,
                    balanced_bpm_for_no_brackets: 0.0,
                    ..ExpressedConfig::default()
                },
            );
        }
        registry
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidFormat(message.into()))
}

/// Original DoubleEquals music-offset comparison (one ULP, signed zero equal).
pub(crate) fn music_offsets_equal(a: f64, b: f64) -> bool {
    crate::number::reference_equal(a, b)
}
