//! Exact editor positions when the standard time-based attack property is ambiguous.
//! Game readers consume ATTACKS; DeadSync restores the row data only while its
//! exact source-property binding and modifier set still match.
use super::{Attacks, Event, Modifier, placement::Timeline};
use crate::{
    Error,
    document::{Document, Format, escape_text},
    history::Session,
    playback::Clock,
    timing_edit::Scope,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowEvent {
    pub row: u64,
    pub modifiers: Vec<Modifier>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    local: bool,
    source: Vec<u8>,
    timing: Vec<crate::generation::timing::Event>,
    offset: f64,
    events: Vec<RowEvent>,
}
pub(crate) fn tag(
    document: &Document,
    chart: usize,
    local: bool,
) -> Result<(Option<usize>, String), Error> {
    if chart >= document.charts().len() {
        return Err(invalid("Resident attack chart does not exist"));
    }
    match document.format {
        Format::Ssc => Ok((
            Some(chart),
            if local {
                "DSAttackRows"
            } else {
                "DSAttackRowsSong"
            }
            .into(),
        )),
        Format::Sm if chart <= 9999 => Ok((None, format!("DSAttackRowsCHART{chart:04}"))),
        _ => Err(invalid(
            "Resident attack chart index exceeds SM metadata limits",
        )),
    }
}
fn stored(session: &Session, chart: usize, local: bool) -> Result<Option<Stored>, Error> {
    let (scope, name) = tag(session.document(), chart, local)?;
    let Some(text) = session
        .document()
        .text(scope, &name)
        .filter(|text| !text.trim().is_empty())
    else {
        return Ok(None);
    };
    let value: Stored = serde_json::from_str(&text)
        .map_err(|_| invalid("Malformed or unsupported resident attack data"))?;
    if value.version != 1 {
        return Err(invalid("Unsupported resident attack data version"));
    }
    validate(&value.events)?;
    Ok(Some(value))
}
pub(crate) fn read(
    session: &Session,
    chart: usize,
    local: bool,
    source: &[u8],
) -> Result<Option<Vec<RowEvent>>, Error> {
    let Some(value) = stored(session, chart, local)? else {
        return Ok(None);
    };
    let timing =
        crate::generation::timing::GenerationTiming::from_document(session.document(), chart)?;
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    if value.local != local
        || value.source != source
        || value.timing != timing.editor_events()
        || value.offset != offset
    {
        return Ok(None);
    }
    let actual = Attacks::parse(source)?;
    if modifier_keys(value.events.iter().flat_map(|e| &e.modifiers))
        != modifier_keys(actual.events().iter().flat_map(|e| &e.modifiers))
    {
        return Err(invalid(
            "Resident attack modifiers do not match their game property",
        ));
    }
    Ok(Some(value.events))
}
fn modifier_keys<'a>(mods: impl Iterator<Item = &'a Modifier>) -> Vec<(String, u64, u64, u64)> {
    fn bits(value: f64) -> u64 {
        if value == 0.0 { 0 } else { value.to_bits() }
    }
    let mut keys: Vec<_> = mods
        .map(|m| (m.name.clone(), bits(m.level), bits(m.speed), bits(m.length)))
        .collect();
    keys.sort();
    keys
}
fn validate(events: &[RowEvent]) -> Result<(), Error> {
    if events.iter().any(|e| e.row > i32::MAX as u64)
        || events.windows(2).any(|pair| pair[0].row >= pair[1].row)
    {
        return Err(invalid("Resident attack rows must be unique and ordered"));
    }
    for event in events {
        for modifier in &event.modifiers {
            modifier.validate()?;
        }
    }
    Ok(())
}
pub(crate) fn events(timeline: &Timeline) -> Vec<RowEvent> {
    timeline
        .markers()
        .iter()
        .map(|m| RowEvent {
            row: m.row,
            modifiers: m.modifiers.clone(),
        })
        .collect()
}
pub(crate) fn same(timeline: &Timeline, events: &[RowEvent]) -> bool {
    timeline.markers().len() == events.len()
        && timeline
            .markers()
            .iter()
            .zip(events)
            .all(|(m, e)| m.row == e.row && m.modifiers == e.modifiers)
}
pub(crate) fn encode(
    session: &Session,
    chart: usize,
    events: &[RowEvent],
    hints: &std::collections::BTreeMap<u64, Vec<f64>>,
) -> Result<Vec<u8>, Error> {
    validate(events)?;
    let rows: Vec<_> = events.iter().map(|e| e.row).collect();
    let clock = Clock::capture_with_attacks(session, chart, &rows)?;
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    let desired: Vec<_> = clock
        .attack_times()
        .map(|(row, time)| {
            (
                row,
                time - offset,
                hints.get(&row).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let times = super::placement::encode_times(session, chart, &desired)
        .unwrap_or_else(|_| desired.iter().map(|(_, time, _)| *time).collect());
    let attacks = Attacks {
        events: events
            .iter()
            .zip(times)
            .map(|(e, time)| Event {
                time,
                modifiers: e.modifiers.clone(),
            })
            .collect(),
    };
    attacks.serialize()
}
/// Called only on a staged transaction, after writing its ordinary ATTACKS tag.
pub(crate) fn reconcile(
    session: &mut Session,
    chart: usize,
    scope: Scope,
    events: &[RowEvent],
) -> Result<(), Error> {
    validate(events)?;
    // Never overwrite a future version or malformed metadata even if its source binding is stale.
    let local = scope == Scope::Chart;
    let existing = stored(session, chart, local)?;
    let file = Timeline::capture_file_scope(session, chart, scope)?;
    let (target, name) = tag(session.document(), chart, local)?;
    if same(&file, events) {
        if existing.is_some() {
            session.set_property_bytes(target, &name, std::sync::Arc::from(&b""[..]))?;
        }
        return Ok(());
    }
    let source = session
        .document()
        .raw_property(local.then_some(chart), "ATTACKS")
        .unwrap_or_default()
        .to_vec();
    let value = Stored {
        version: 1,
        local,
        source,
        timing: crate::generation::timing::GenerationTiming::from_document(
            session.document(),
            chart,
        )?
        .editor_events()
        .to_vec(),
        offset: crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset(),
        events: events.to_vec(),
    };
    let bytes = escape_text(&serde_json::to_string(&value).map_err(|e| invalid(&e.to_string()))?);
    session.set_property_bytes(target, &name, bytes.into())?;
    // Validate the effect/source binding before publishing the surrounding history group.
    if !same(&Timeline::capture_scope(session, chart, scope)?, events) {
        return Err(invalid(
            "Resident attack transaction did not preserve its rows",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
