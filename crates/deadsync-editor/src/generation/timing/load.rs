//! Fumen source import and the editor's subsequent constructor adjustments.
//! Stored source bytes are never rewritten by normalization.
use super::{Event, EventKind, GenerationTiming};
use crate::{
    Error,
    document::{Document, Format},
    timing_edit::{
        self,
        tempo::{MIN_BPM, beat_row},
    },
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy)]
enum Data {
    Number(f64),
    Signature(i32, i32),
    Marker,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Tempo,
    Delay,
    Stop,
    Warp,
    Signature,
    Tick,
    Fake,
    Combo,
    Label,
    Scroll,
    Speed,
}
impl Field {
    fn tag(self) -> &'static str {
        match self {
            Self::Tempo => "BPMS",
            Self::Delay => "DELAYS",
            Self::Stop => "STOPS",
            Self::Warp => "WARPS",
            Self::Signature => "TIMESIGNATURES",
            Self::Tick => "TICKCOUNTS",
            Self::Fake => "FAKES",
            Self::Combo => "COMBOS",
            Self::Label => "LABELS",
            Self::Scroll => "SCROLLS",
            Self::Speed => "SPEEDS",
        }
    }
}
const FIELDS: [Field; 11] = [
    Field::Tempo,
    Field::Delay,
    Field::Stop,
    Field::Warp,
    Field::Signature,
    Field::Tick,
    Field::Fake,
    Field::Combo,
    Field::Label,
    Field::Scroll,
    Field::Speed,
];
pub(super) fn load(document: &Document, chart: usize) -> Result<GenerationTiming, Error> {
    if chart >= document.charts().len() {
        return Err(invalid("Chart does not exist"));
    }
    // The original reader parses the whole song before selecting a chart.
    // Invalid duplicate maps in a different chart must not be overlooked.
    let mut seen = BTreeSet::new();
    for tag in document.tags() {
        let supported = FIELDS.iter().any(|field| {
            field.tag() == tag.name
                && (document.format != Format::Sm
                    || matches!(
                        field,
                        Field::Tempo | Field::Delay | Field::Stop | Field::Signature | Field::Tick
                    ))
        });
        if supported && !seen.insert((tag.chart, tag.name.as_str())) {
            return Err(invalid(
                "Repeated timing properties require explicit repair; the reference reader also rejects them",
            ));
        }
    }
    let own = timing_edit::reference_uses_chart_timing(document, chart);
    let scope = own.then_some(chart);
    let mut file = Vec::new();
    for kind in FIELDS {
        // These properties are not handled by the original SMReader.
        if document.format == Format::Sm
            && matches!(
                kind,
                Field::Warp
                    | Field::Speed
                    | Field::Fake
                    | Field::Combo
                    | Field::Label
                    | Field::Scroll
            )
        {
            continue;
        }
        let rows = records(document, scope, kind)?;
        let replacement = rows
            .values()
            .find_map(|data| match data {
                Data::Number(value) if *value > 0.0 => Some(*value),
                _ => None,
            })
            .unwrap_or(60.0);
        for (index, (row, data)) in rows.into_iter().enumerate() {
            let value = match (kind, data) {
                (Field::Tempo, Data::Number(value)) => {
                    if value > 0.0 {
                        Some(EventKind::Tempo(value))
                    } else if index == 0 {
                        Some(EventKind::Tempo(replacement))
                    } else {
                        None
                    }
                }
                (Field::Delay, Data::Number(value)) if value >= 0.0 => {
                    Some(EventKind::Delay(value))
                }
                (Field::Stop, Data::Number(value))
                    if !crate::number::reference_equal(value, 0.0) =>
                {
                    Some(EventKind::Stop(value))
                }
                (Field::Warp | Field::Fake, Data::Number(value)) if value > 0.0 => {
                    let length = beat_row(value)?;
                    Some(if kind == Field::Warp {
                        EventKind::Warp(length)
                    } else {
                        EventKind::Fake(length)
                    })
                }
                (Field::Signature, Data::Signature(numerator, denominator))
                    if numerator > 0 && denominator > 0 && 192 % denominator == 0 =>
                {
                    Some(EventKind::TimeSignature)
                }
                (Field::Tick, Data::Marker) => Some(EventKind::TickCount),
                (Field::Combo, Data::Marker) => Some(EventKind::Combo),
                (Field::Label, Data::Marker) => Some(EventKind::Label),
                (Field::Scroll, Data::Number(value)) => Some(EventKind::Scroll(value)),
                (Field::Speed, Data::Marker) => Some(EventKind::TrailingMarker),
                _ => None,
            };
            if let Some(kind) = value {
                file.push(Event { row, kind });
            }
        }
    }
    if !file
        .iter()
        .any(|event| event.row == 0 && matches!(event.kind, EventKind::TimeSignature))
    {
        file.push(Event {
            row: 0,
            kind: EventKind::TimeSignature,
        });
    }
    let mut editor = file.clone();
    for event in &mut editor {
        event.kind = match event.kind {
            EventKind::Tempo(value) => EventKind::Tempo(value.max(MIN_BPM)),
            EventKind::Warp(length) => EventKind::Warp(length.max(1)),
            EventKind::Fake(length) => EventKind::Fake(length.max(1)),
            other => other,
        };
    }
    if !editor
        .iter()
        .any(|event| event.row == 0 && matches!(event.kind, EventKind::Tempo(_)))
    {
        editor.push(Event {
            row: 0,
            kind: EventKind::Tempo(starting_tempo(document, chart)?),
        });
    }
    GenerationTiming::with_editor_events(file, editor)
}
/// Parse valid records first, then resolve rounded-row conflicts. Semantic
/// filtering must come later: an invalid length can mask a valid earlier one.
fn records(
    document: &Document,
    scope: Option<usize>,
    kind: Field,
) -> Result<BTreeMap<i64, Data>, Error> {
    let pairs = source_pairs(document, scope, kind)?;
    let mut rows = BTreeMap::new();
    for (beat, data) in pairs {
        let row = beat_row(beat)?;
        if row >= 0 {
            rows.insert(row, data);
        }
    }
    Ok(rows)
}
fn source_pairs(
    document: &Document,
    scope: Option<usize>,
    kind: Field,
) -> Result<Vec<(f64, Data)>, Error> {
    let tag = kind.tag();
    let mut sources = document
        .tags()
        .iter()
        .filter(|entry| {
            entry.chart == scope
                && (entry.name.eq_ignore_ascii_case(tag)
                    || (document.format == Format::Sm
                        && kind == Field::Stop
                        && entry.name.eq_ignore_ascii_case("FREEZES")))
        })
        .map(|entry| rssp::parse::decode_unescape(document.value(entry.value.clone())))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        if let Some(raw) = document.raw_property(scope, tag) {
            sources.push(rssp::parse::decode_unescape(raw));
        }
    }
    if kind == Field::Label {
        sources = document
            .text(scope, tag)
            .map(std::borrow::Cow::Owned)
            .into_iter()
            .collect();
    }
    let mut pairs = Vec::new();
    for source in &sources {
        let cleaned = if kind == Field::Label {
            source.as_ref().to_owned()
        } else {
            source
                .lines()
                .map(|line| line.split_once("//").map_or(line, |(value, _)| value))
                .collect::<Vec<_>>()
                .join("")
        };
        for part in cleaned.split(',').filter(|part| !part.trim().is_empty()) {
            let fields = part.split('=').map(str::trim).collect::<Vec<_>>();
            let Some(beat) = fields.first().and_then(|beat| beat.parse::<f64>().ok()) else {
                continue;
            };
            if !beat.is_finite() {
                return Err(invalid("Timing position must be finite"));
            }
            let data = match kind {
                Field::Tempo
                | Field::Delay
                | Field::Stop
                | Field::Warp
                | Field::Fake
                | Field::Scroll
                    if fields.len() == 2 =>
                {
                    let Ok(value) = fields[1].parse::<f64>() else {
                        continue;
                    };
                    if !value.is_finite() {
                        return Err(invalid("Timing value must be finite"));
                    }
                    Data::Number(value)
                }
                Field::Signature if fields.len() == 3 => {
                    let (Ok(numerator), Ok(denominator)) =
                        (fields[1].parse::<i32>(), fields[2].parse::<i32>())
                    else {
                        continue;
                    };
                    Data::Signature(numerator, denominator)
                }
                Field::Tick if fields.len() == 2 && fields[1].parse::<i32>().is_ok() => {
                    Data::Marker
                }
                Field::Combo
                    if (fields.len() == 2 || fields.len() == 3)
                        && fields[1..].iter().all(|field| field.parse::<i32>().is_ok()) =>
                {
                    Data::Marker
                }
                Field::Label if fields.len() == 2 => Data::Marker,
                Field::Speed if fields.len() == 4 => {
                    let (Ok(speed), Ok(length), Ok(mode)) = (
                        fields[1].parse::<f64>(),
                        fields[2].parse::<f64>(),
                        fields[3].parse::<i32>(),
                    ) else {
                        continue;
                    };
                    if !speed.is_finite() || !length.is_finite() {
                        return Err(invalid("Speed values must be finite"));
                    }
                    if mode == 0 {
                        beat_row(length)?;
                    }
                    Data::Marker
                }
                _ => continue,
            };
            pairs.push((beat, data));
        }
    }
    pairs.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    Ok(pairs)
}

fn initial(document: &Document, chart: usize) -> Result<Option<f64>, Error> {
    let scope = timing_edit::reference_uses_chart_timing(document, chart).then_some(chart);
    let rows = records(document, scope, Field::Tempo)?;
    let replacement = rows
        .values()
        .find_map(|data| match data {
            Data::Number(value) if *value > 0.0 => Some(*value),
            _ => None,
        })
        .unwrap_or(60.0);
    Ok(rows.get(&0).map(|data| match data {
        Data::Number(value) if *value > 0.0 => value.max(MIN_BPM),
        _ => replacement.max(MIN_BPM),
    }))
}
pub(super) fn starting_tempo(document: &Document, chart: usize) -> Result<f64, Error> {
    let mut groups: Vec<(&str, Vec<f64>)> = Vec::new();
    let fallback = || {
        // The original song fallback uses an exact zero-beat dictionary key,
        // before row rounding or invalid first-tempo replacement.
        let direct = source_pairs(document, None, Field::Tempo)?
            .into_iter()
            .rev()
            .find(|(beat, _)| *beat == 0.0)
            .and_then(|(_, data)| match data {
                Data::Number(value) if value >= MIN_BPM => Some(value),
                _ => None,
            });
        Ok::<f64, Error>(
            direct
                .or_else(|| timing_edit::display_tempo(document))
                .unwrap_or(120.0),
        )
    };
    for index in 0..=chart {
        if chart == document.charts().len() && index < chart
            && crate::notes::layout_columns(&document.charts()[index].steps_type).is_none()
        {
            continue;
        }
        let mut histogram: Vec<(f64, usize)> = Vec::new();
        for value in groups.iter().flat_map(|(_, values)| values) {
            if let Some(entry) = histogram.iter_mut().find(|entry| entry.0 == *value) {
                entry.1 += 1;
            } else {
                histogram.push((*value, 1));
            }
        }
        let common = histogram
            .iter()
            .reduce(|left, right| if left.1 > right.1 { left } else { right })
            .map(|entry| entry.0);
        if index == document.charts().len() {
            return match common {
                Some(value) => Ok(value),
                None => fallback(),
            };
        }
        let value = match initial(document, index)? {
            Some(value) => value,
            None => match common {
                Some(value) => value,
                None => fallback()?,
            },
        };
        if index == chart {
            return Ok(value);
        }
        let kind = document.charts()[index].steps_type.as_str();
        if let Some((_, values)) = groups.iter_mut().find(|(name, _)| *name == kind) {
            values.push(value);
        } else {
            groups.push((kind, vec![value]));
        }
    }
    unreachable!("selected chart validated before tempo fallback")
}
fn invalid(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
