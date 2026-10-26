//! Keep resident attack rows stable across timing/property transactions.
use super::{Attacks, Event, placement::Timeline};
use crate::{Error, document::Format, history::Session, playback::Clock};

pub(crate) fn preserve(before: &Session, after: &mut Session) -> Result<(), Error> {
    let mut charts = Vec::new();
    for (chart, descriptor) in before.document().charts().iter().enumerate() {
        let has_attacks = [None, Some(chart)].into_iter().any(|scope| {
            before
                .document()
                .raw_property(scope, "ATTACKS")
                .is_some_and(|raw| !raw.is_empty())
        });
        if !has_attacks {
            continue;
        }
        if crate::notes::layout_columns(&descriptor.steps_type).is_none() {
            return Err(Error::Unsupported(
                "Cannot preserve attack rows for an unknown chart layout".into(),
            ));
        }
        let original = Timeline::capture(before, chart)?;
        if original.markers().is_empty() {
            continue;
        }
        let current = Timeline::capture(after, chart)?;
        if same(&original, &current) {
            charts.push((chart, original, None));
            continue;
        }
        let rows: Vec<_> = original.markers().iter().map(|marker| marker.row).collect();
        let clock = Clock::capture_with_attacks(after, chart, &rows)?;
        let offset =
            crate::chart_properties::Draft::capture(after, chart)?.effective_music_offset();
        let desired: Vec<_> = clock
            .attack_times()
            .map(|(row, time)| (row, time - offset, Vec::new()))
            .collect();
        let times = super::placement::encode_times(after, chart, &desired)?;
        let attacks = Attacks {
            events: original
                .markers()
                .iter()
                .zip(times)
                .map(|(marker, time)| Event {
                    time,
                    modifiers: marker.modifiers.clone(),
                })
                .collect(),
        };
        charts.push((chart, original, Some(attacks.serialize_preserving_empty()?)));
    }
    if after.document().format == Format::Ssc {
        for (chart, _, bytes) in &charts {
            if let Some(bytes) = bytes {
                after.set_property_bytes(Some(*chart), "ATTACKS", bytes.clone().into())?;
            }
        }
    } else if charts.iter().any(|(_, _, bytes)| bytes.is_some()) {
        // SM has a single shared attack list. Search the per-chart encodings for
        // a representation valid for every resident chart, including note tails.
        let mut found = None;
        for (_, _, bytes) in &charts {
            let Some(bytes) = bytes else {
                continue;
            };
            let mut candidate = Session::new(crate::document::Document::parse(
                after.serialize()?,
                Format::Sm,
            )?);
            candidate.set_property_bytes(None, "ATTACKS", bytes.clone().into())?;
            if charts.iter().all(|(chart, expected, _)| {
                Timeline::capture(&candidate, *chart).is_ok_and(|actual| same(expected, &actual))
            }) {
                found = Some(bytes.clone());
                break;
            }
        }
        let bytes = found.ok_or_else(|| {
            Error::Unsupported(
                "Shared SM attacks cannot preserve every chart position; convert to SSC".into(),
            )
        })?;
        after.set_property_bytes(None, "ATTACKS", bytes.into())?;
    }
    for (chart, expected, _) in &charts {
        let actual = Timeline::capture(after, *chart)?;
        if !same(expected, &actual) {
            return Err(Error::InvalidEdit(
                "Timing change cannot preserve attack positions through save/reload".into(),
            ));
        }
    }
    Ok(())
}
fn same(expected: &Timeline, actual: &Timeline) -> bool {
    expected.markers().len() == actual.markers().len()
        && expected
            .markers()
            .iter()
            .zip(actual.markers())
            .all(|(left, right)| left.row == right.row && left.modifiers == right.modifiers)
}
