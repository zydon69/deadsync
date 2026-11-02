//! Song-level widgets have fractional chart positions and immutable placement
//! for Delete/Shift, but source Copy/Paste repositions their target song fields.
use crate::{Error, history::Session, playback::Clock, song_properties::Draft, song_text::Field};
use std::cmp::Ordering;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WidgetKind {
    Preview,
    EndHint,
}
/// Finite position captured through the chart clock, never an arbitrary public
/// `(row,time)` tuple. Session-bound Selection validates the identity's lifetime.
#[derive(Clone, Copy, Debug)]
pub struct SongWidget {
    kind: WidgetKind,
    row: f64,
    chart_time: f64,
}
impl SongWidget {
    pub fn kind(self) -> WidgetKind {
        self.kind
    }
    pub fn position(self) -> f64 {
        self.row
    }
    pub fn chart_time(self) -> f64 {
        self.chart_time
    }
    pub fn source_row(self) -> i64 {
        self.row.trunc() as i64
    }
}
impl PartialEq for SongWidget {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.row == other.row && self.chart_time == other.chart_time
    }
}
impl Eq for SongWidget {}
impl PartialOrd for SongWidget {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for SongWidget {
    fn cmp(&self, other: &Self) -> Ordering {
        self.row
            .total_cmp(&other.row)
            .then(self.kind.cmp(&other.kind))
            .then(self.chart_time.total_cmp(&other.chart_time))
    }
}
pub(super) fn capture(session: &Session, chart: usize) -> Result<Vec<SongWidget>, Error> {
    let draft = Draft::capture(session)?;
    capture_draft(&draft, session, chart)
}
pub(crate) fn capture_draft(
    draft: &Draft,
    session: &Session,
    chart: usize,
) -> Result<Vec<SongWidget>, Error> {
    let clock = draft.clock(session, chart)?;
    let mut widgets = Vec::new();
    for (kind, present, time) in [
        (
            WidgetKind::Preview,
            draft.text(Field::PreviewMusic).is_empty(),
            draft.sample_start(),
        ),
        (
            WidgetKind::EndHint,
            draft.last_second_hint() > 0.0,
            draft.last_second_hint(),
        ),
    ] {
        if !present {
            continue;
        }
        let row = clock.position(time)?;
        let chart_time = clock.music_to_chart_time(time)?;
        if !row.is_finite() || row < i32::MIN as f64 || row > i32::MAX as f64 {
            return Err(Error::InvalidEdit(
                "Song widget exceeds editor positions".into(),
            ));
        }
        widgets.push(SongWidget {
            kind,
            row: if row == 0.0 { 0.0 } else { row },
            chart_time: if chart_time == 0.0 { 0.0 } else { chart_time },
        });
    }
    Ok(widgets)
}
#[derive(Clone)]
pub(super) struct Copied {
    pub kind: WidgetKind,
    pub row: i64,
}
impl Copied {
    pub fn apply(
        &self,
        session: &mut Session,
        chart: usize,
        origin: i64,
        destination: u64,
    ) -> Result<(), Error> {
        let row = i128::from(destination) + i128::from(self.row) - i128::from(origin);
        if !(0..=i128::from(i32::MAX)).contains(&row) {
            return Err(Error::InvalidEdit(
                "Song widget paste exceeds editor rows".into(),
            ));
        }
        // Use the target clock after pasted rate events; this is the source's
        // before-same-row position time, not note time after a delay.
        let time = Clock::capture(session, chart)?.music_time(row as u64)?;
        let mut draft = Draft::capture(session)?;
        match self.kind {
            WidgetKind::Preview => draft.set_sample_start(time)?,
            WidgetKind::EndHint => draft.set_last_second_hint(time)?,
        }
        draft.apply(session)?;
        Ok(())
    }
}
