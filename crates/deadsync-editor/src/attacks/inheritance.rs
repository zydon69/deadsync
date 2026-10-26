//! SSC empty chart attacks require eliminating the shared fallback. Preserve
//! peers by materializing their inherited attacks and complete timing first.
use super::{Draft, placement::Timeline, resident};
use crate::{
    Error, chart_properties,
    generation::timing::GenerationTiming,
    history::Session,
    timing_edit::{self, Scope, TempoDraft},
};
use std::sync::Arc;

struct Before {
    inherited: bool,
    events: Vec<resident::RowEvent>,
    clock: GenerationTiming,
    tempo: String,
    offset: f64,
}
fn same_clock(session: &Session, chart: usize, before: &Before) -> Result<bool, Error> {
    Ok(
        GenerationTiming::from_document(session.document(), chart)?.editor_events()
            == before.clock.editor_events()
            && chart_properties::Draft::capture(session, chart)?.effective_music_offset()
                == before.offset,
    )
}
fn materialize_timing(session: &mut Session, chart: usize) -> Result<(), Error> {
    let fields = timing_edit::inherited_fields_for_write(session.document(), chart, &[])?;
    for (name, bytes) in fields {
        session.set_property_bytes(Some(chart), &name, bytes)?;
    }
    Ok(())
}
pub(super) fn clear_chart(session: &mut Session, chart: usize) -> Result<bool, Error> {
    // Capture before staging so every fallback uses the original preceding-chart
    // tempo histogram, and every peer sees its pending note grid.
    let mut before = Vec::new();
    for index in 0..session.document().charts().len() {
        let file = Draft::capture(session, index, Scope::Chart)?;
        before.push(Before {
            inherited: file.inherited(),
            events: resident::events(&Timeline::capture_scope(session, index, Scope::Chart)?),
            clock: GenerationTiming::from_document(session.document(), index)?,
            tempo: TempoDraft::capture(session, index, Scope::Chart)?
                .tempos()
                .serialize(),
            offset: chart_properties::Draft::capture(session, index)?.effective_music_offset(),
        });
    }
    let shared: Arc<[u8]> = Arc::from(
        session
            .document()
            .raw_property(None, "ATTACKS")
            .unwrap_or_default(),
    );
    session.edit_chart_atomic(chart, "Clear chart attacks", |staged| {
        for (index, original) in before.iter().enumerate() {
            if index == chart {
                continue;
            }
            if !original.inherited && original.events.is_empty() {
                continue;
            }
            // Independent local attacks also need split timing once the song
            // fallback disappears. Keep their ATTACKS bytes and row metadata.
            materialize_timing(staged, index)?;
            // Force split timing even when the source has no explicit BPMS tag.
            if staged
                .document()
                .raw_property(Some(index), "BPMS")
                .is_none()
            {
                staged.set_property_bytes(
                    Some(index),
                    "BPMS",
                    Arc::from(original.tempo.as_bytes()),
                )?;
            }
            if original.inherited {
                staged.set_property_bytes(Some(index), "ATTACKS", shared.clone())?;
            }
        }
        staged.set_property_bytes(Some(chart), "ATTACKS", Arc::from(&b""[..]))?;
        staged.set_property_bytes(None, "ATTACKS", Arc::from(&b""[..]))?;

        // Adding explicit peer tempos can change the reader's fallback for a
        // later chart which lacked an initial BPM. Freeze only clocks that change.
        for (index, original) in before.iter().enumerate() {
            if !same_clock(staged, index, original)? {
                materialize_timing(staged, index)?;
                staged.set_property_bytes(
                    Some(index),
                    "BPMS",
                    Arc::from(original.tempo.as_bytes()),
                )?;
            }
        }
        for (index, original) in before.iter().enumerate() {
            if !same_clock(staged, index, original)? {
                return Err(Error::InvalidEdit(
                    "Clearing chart attacks changed a chart clock".into(),
                ));
            }
            let expected = if index == chart {
                &[][..]
            } else {
                &original.events
            };
            if index == chart || original.inherited {
                resident::reconcile(staged, index, Scope::Chart, expected)?;
            }
            if !resident::same(
                &Timeline::capture_scope(staged, index, Scope::Chart)?,
                expected,
            ) {
                return Err(Error::InvalidEdit(
                    "Clearing chart attacks changed another chart's attacks".into(),
                ));
            }
        }
        Ok(())
    })
}
