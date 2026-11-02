//! Keep resident attack rows stable across timing/property transactions.
use super::{placement::Timeline, resident};
use crate::{Error, history::Session, timing_edit::Scope};

/// Bind copied non-performance attacks to their source editor rows. Generated
/// notes can change the file reader's inverse clock, even with identical timing.
pub(crate) fn preserve_generated(
    before: &Session,
    after: &mut Session,
    copies: &[(usize, usize)],
) -> Result<(), Error> {
    for &(source, target) in copies {
        let events = resident::events(&Timeline::capture(before, source)?);
        if events.is_empty() {
            continue;
        }
        let scope = if super::supports_resident_chart_attacks(after.document()) {
            let bytes = resident::encode(after, target, &events, &Default::default())?;
            super::property::write(after,Some(target),bytes.into())?;
            Scope::Chart
        } else {
            Scope::Song
        };
        resident::reconcile(after, target, scope, &events)?;
        if !resident::same(&Timeline::capture(after, target)?, &events) {
            return Err(Error::InvalidEdit(
                "Generated chart changed attack positions".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn preserve(before: &Session, after: &mut Session) -> Result<(), Error> {
    let scope = if super::supports_resident_chart_attacks(after.document()) {
        Scope::Chart
    } else {
        Scope::Song
    };
    let mut charts = Vec::new();
    for (chart, descriptor) in before.document().charts().iter().enumerate() {
        let mut has_attacks=false;
        for origin in [None,Some(chart)] {
            has_attacks |= super::property::read(before.document(),origin)?.is_some_and(|raw|!raw.is_empty());
        }
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
        let current = Timeline::capture_file_scope(after, chart, scope)?;
        let events = resident::events(&original);
        let needs_write = !resident::same(&current, &events) || original.has_resident_rows();
        let bytes = if needs_write {
            Some(resident::encode(
                after,
                chart,
                &events,
                &Default::default(),
            )?)
        } else {
            None
        };
        charts.push((chart, events, bytes));
    }
    if scope == Scope::Chart {
        for (chart, events, bytes) in &charts {
            if let Some(bytes) = bytes {
                super::property::write(after,Some(*chart),bytes.clone().into())?;
                resident::reconcile(after, *chart, scope, events)?;
            }
        }
    } else if charts.iter().any(|(_, _, bytes)| bytes.is_some()) {
        // Choose a shared encoding valid for the most charts. Any remaining row
        // ambiguity is retained per chart in editor metadata, without losing effects.
        let mut best = None;
        for (_, _, bytes) in &charts {
            let Some(bytes) = bytes else {
                continue;
            };
            let mut candidate = Session::new(after.snapshot_document()?);
            candidate.set_property_bytes(None, "ATTACKS", bytes.clone().into())?;
            let score = charts
                .iter()
                .filter(|(chart, events, _)| {
                    Timeline::capture_file_scope(&candidate, *chart, scope)
                        .is_ok_and(|actual| resident::same(&actual, events))
                })
                .count();
            if best.as_ref().is_none_or(|(_, old)| score > *old) {
                best = Some((bytes.clone(), score));
            }
        }
        if let Some((bytes, _)) = best {
            after.set_property_bytes(None, "ATTACKS", bytes.into())?;
        }
        for (chart, events, _) in &charts {
            resident::reconcile(after, *chart, scope, events)?;
        }
    }
    for (chart, events, _) in &charts {
        if !resident::same(&Timeline::capture(after, *chart)?, events) {
            return Err(Error::InvalidEdit(
                "Timing change did not preserve resident attack positions".into(),
            ));
        }
    }
    Ok(())
}
