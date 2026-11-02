//! Immutable visual speed snapshot using real editor event-phase times.
//! This is separate from audio Clock capture: unsupported visual speed metadata
//! must not introduce new failures into ordinary audio playback.
use crate::{
    Error,
    document::Format,
    history::Session,
    playback::Clock,
    timing_edit::speed::{Period, Speeds},
};
use std::collections::BTreeMap;

pub struct Timeline {
    speeds: Speeds,
    origins: BTreeMap<u64, f64>,
}
impl Timeline {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let document = session.document();
        if chart >= document.charts().len() {
            return Err(Error::InvalidEdit("Invalid speed chart".into()));
        }
        let own=if document.format==Format::Sm {crate::timing_edit::uses_chart_timing(document,chart)} else {crate::timing_edit::reference_uses_chart_timing(document,chart)};
        let text=crate::timing_edit::resident::text(document,own.then_some(chart),"SPEEDS")?;
        let speeds = Speeds::parse(&text)?;
        let mut origins = BTreeMap::new();
        if speeds
            .values()
            .values()
            .any(|value| matches!(value.period, Period::Seconds(_)))
        {
            let clock = Clock::capture(session, chart)?;
            for (&row, value) in speeds.values() {
                if matches!(value.period, Period::Seconds(_)) {
                    origins.insert(row, clock.speed_event_time(row)?);
                }
            }
        }
        Ok(Self { speeds, origins })
    }
    /// Chart-time seconds, without audio-file offset or song sync offset.
    /// Selection is by row even when the active event interpolates by time.
    pub fn rate_at(&self, row: f64, chart_time: f64) -> Result<f64, Error> {
        self.speeds.rate_at(row, chart_time, |start| {
            self.origins
                .get(&start)
                .copied()
                .ok_or_else(|| Error::InvalidEdit("Missing captured speed origin".into()))
        })
    }
    pub fn time_origins(&self) -> &BTreeMap<u64, f64> {
        &self.origins
    }
    pub fn speeds(&self) -> &Speeds {
        &self.speeds
    }
}
