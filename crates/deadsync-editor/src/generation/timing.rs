//! Generation decision clock from Fumen SMCommon.SetEventTimeFromRows (MIT,
//! Fumen 0fe292f9). Gameplay/analysis keep RSSP timing. The reference generator
//! uses row-based f64 arithmetic; replacing it with beat-based queries can
//! change strict NPS threshold decisions through floating-point rounding.
mod load;
use crate::{
    Error,
    document::{Document, Format},
    notes::ROWS_PER_BEAT,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub enum EventKind {
    Tempo(f64),
    Delay(f64),
    Stop(f64),
    Warp(i64),
    Fake(i64),
    TimeSignature,
    TickCount,
    Combo,
    Label,
    Scroll(f64),
    /// Time signatures, ticks, fakes, combos and labels precede delays.
    LeadingMarker,
    /// Scroll and speed events follow notes but precede ordinary stops.
    TrailingMarker,
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub row: i64,
    pub kind: EventKind,
}

impl Event {
    pub(crate) fn order(self) -> u8 {
        match self.kind {
            EventKind::TimeSignature => 0,
            EventKind::Tempo(_) => 1,
            EventKind::LeadingMarker | EventKind::TickCount => 2,
            EventKind::Fake(_) => 3,
            EventKind::Combo => 4,
            EventKind::Label => 5,
            EventKind::Delay(_) => 7,
            EventKind::Scroll(_) => 13,
            EventKind::TrailingMarker => 14,
            EventKind::Stop(length) if length >= 0.0 => 15,
            EventKind::Stop(_) => 16,
            EventKind::Warp(_) => 17,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GenerationTiming {
    events: Vec<Event>,
    editor_events: Option<Vec<Event>>,
}

impl GenerationTiming {
    pub fn new(mut events: Vec<Event>) -> Result<Self, Error> {
        for event in &events {
            if event.row < i64::from(i32::MIN) || event.row > i64::from(i32::MAX) {
                return invalid("Timing row exceeds the reference range");
            }
            match event.kind {
                EventKind::Tempo(bpm)
                    if !bpm.is_finite()
                        || bpm <= 0.0
                        || !(60.0 / bpm / ROWS_PER_BEAT as f64).is_finite() =>
                {
                    return invalid(
                        "Generation requires finite positive BPM values with representable row times",
                    );
                }
                EventKind::Stop(value) | EventKind::Delay(value) if !value.is_finite() => {
                    return invalid("Stop duration must be finite");
                }
                EventKind::Warp(rows) | EventKind::Fake(rows)
                    if rows < 0
                        || event
                            .row
                            .checked_add(rows)
                            .is_none_or(|end| end > i64::from(i32::MAX)) =>
                {
                    return invalid("Invalid warp row range");
                }
                EventKind::Scroll(value) if !value.is_finite() => {
                    return invalid("Scroll rate must be finite");
                }
                _ => {}
            }
        }
        events.sort_by_key(|event| (event.row, event.order()));
        Ok(Self {
            events,
            editor_events: None,
        })
    }

    /// Source normalization retains separate file and editor event sets.
    pub fn from_simfile(bytes: &[u8], format: Format, chart: usize) -> Result<Self, Error> {
        Self::from_document(&Document::parse(Arc::<[u8]>::from(bytes), format)?, chart)
    }
    pub(crate) fn from_document(document: &Document, chart: usize) -> Result<Self, Error> {
        load::load(document, chart)
    }
    fn with_editor_events(file: Vec<Event>, editor: Vec<Event>) -> Result<Self, Error> {
        let mut result = Self::new(file)?;
        result.editor_events = Some(Self::new(editor)?.events);
        Ok(result)
    }
    pub(crate) fn editor_events(&self) -> &[Event] {
        self.editor_events.as_deref().unwrap_or(&self.events)
    }

    pub fn times(&self, rows: impl IntoIterator<Item = u64>) -> Result<Vec<f64>, Error> {
        self.times_checked(rows, || Ok(()))
    }

    /// Original first/last event times, including timing-only rows and note
    /// rows that produce no movement (for example keysounds and mines).
    pub fn event_span_checked(
        &self,
        note_rows: impl IntoIterator<Item = u64>,
        mut checkpoint: impl FnMut() -> Result<(), Error>,
    ) -> Result<[f64; 2], Error> {
        let mut events = self.events.clone();
        for row in note_rows {
            checkpoint()?;
            let row = i64::try_from(row)
                .map_err(|_| Error::InvalidEdit("Generation row overflow".into()))?;
            if row > i64::from(i32::MAX) {
                return invalid("Generation row exceeds the reference range");
            }
            events.push(Event {
                row,
                kind: EventKind::TrailingMarker,
            });
        }
        events.sort_by_key(|event| (event.row, event.order()));
        let mut clock = Clock::default();
        let mut first = None;
        let mut last = 0.0;
        for event in events {
            checkpoint()?;
            last = clock.event(event.row, Some(event.kind));
            if !last.is_finite() {
                return invalid("Generation timing produced a non-finite timestamp");
            }
            first.get_or_insert(last);
        }
        Ok([first.unwrap_or(0.0), last])
    }

    /// ExpressedChart.ParseNextEvents uses the time of the first event at each
    /// row, before same-row tempo/delay events alter later note timestamps.
    /// Mine events keep their own note timestamps and must use `times` instead.
    pub fn grouped_times(&self, rows: impl IntoIterator<Item = u64>) -> Result<Vec<f64>, Error> {
        self.query_times(rows, true, || Ok(()))
    }

    pub fn grouped_times_checked(
        &self,
        rows: impl IntoIterator<Item = u64>,
        checkpoint: impl FnMut() -> Result<(), Error>,
    ) -> Result<Vec<f64>, Error> {
        self.query_times(rows, true, checkpoint)
    }

    /// Check cancellation while preparing long ranges, including dense timing events.
    pub fn times_checked(
        &self,
        rows: impl IntoIterator<Item = u64>,
        checkpoint: impl FnMut() -> Result<(), Error>,
    ) -> Result<Vec<f64>, Error> {
        self.query_times(rows, false, checkpoint)
    }

    fn query_times(
        &self,
        rows: impl IntoIterator<Item = u64>,
        grouped: bool,
        mut checkpoint: impl FnMut() -> Result<(), Error>,
    ) -> Result<Vec<f64>, Error> {
        let mut clock = Clock::default();
        let mut cursor = 0;
        let mut previous_row = None;
        let mut output = Vec::new();
        for row in rows {
            checkpoint()?;
            let row = i64::try_from(row)
                .map_err(|_| Error::InvalidEdit("Generation row overflow".into()))?;
            if row > i64::from(i32::MAX) || previous_row.is_some_and(|previous| previous > row) {
                return invalid("Generation rows must be ordered and within the reference range");
            }
            while let Some(event) = self.events.get(cursor).filter(|event| {
                event.row < row || (!grouped && event.row == row && event.order() < 8)
            }) {
                checkpoint()?;
                clock.event(event.row, Some(event.kind));
                cursor += 1;
            }
            let time = clock.event(row, None);
            if !time.is_finite() {
                return invalid("Generation timing produced a non-finite timestamp");
            }
            output.push(time);
            previous_row = Some(row);
        }
        Ok(output)
    }
}

#[derive(Default)]
pub(crate) struct Clock {
    tempo_row: i64,
    tempo_time: f64,
    seconds_per_row: Option<f64>,
    stops: f64,
    previous_time: f64,
    warp_end: Option<i64>,
    warp_start: Option<i64>,
    warp_time: f64,
}

pub(crate) struct ClockSample {
    pub time: f64,
    pub clamped: bool,
    pub warping: bool,
}
impl Clock {
    fn event(&mut self, row: i64, kind: Option<EventKind>) -> f64 {
        self.sample(row, kind, false).time
    }

    pub(crate) fn editor_event(&mut self, row: i64, kind: Option<EventKind>) -> ClockSample {
        self.sample(row, kind, true)
    }

    fn sample(&mut self, row: i64, kind: Option<EventKind>, editor: bool) -> ClockSample {
        let absolute = self.tempo_time
            + self
                .seconds_per_row
                .map_or(0.0, |seconds| (row - self.tempo_row) as f64 * seconds);
        let mut current_warp = 0.0;
        if let Some(end) = self.warp_end {
            current_warp = self.seconds_per_row.map_or(0.0, |seconds| {
                (row.min(end) - self.warp_start.unwrap()) as f64 * seconds
            });
            if row >= end {
                self.warp_end = None;
                self.warp_start = None;
                self.warp_time += current_warp;
                current_warp = 0.0;
            }
        }
        let raw = absolute - current_warp - self.warp_time + self.stops;
        let clamped = raw < self.previous_time;
        let time = raw.max(self.previous_time);
        let warping = self.warp_end.is_some();
        self.previous_time = time;
        match kind {
            Some(EventKind::TimeSignature) if editor && self.warp_end.is_some() => {
                self.warp_time += current_warp;
                self.warp_start = Some(row);
            }
            Some(EventKind::Tempo(bpm)) => {
                self.seconds_per_row = Some(60.0 / bpm / ROWS_PER_BEAT as f64);
                self.tempo_row = row;
                self.tempo_time = absolute;
                if self.warp_end.is_some() {
                    self.warp_time += current_warp;
                    self.warp_start = Some(row);
                }
            }
            Some(EventKind::Stop(seconds) | EventKind::Delay(seconds)) => self.stops += seconds,
            Some(EventKind::Warp(rows)) => {
                self.warp_end = Some(self.warp_end.map_or(row + rows, |end| end.max(row + rows)));
                if self.warp_start.is_none() {
                    self.warp_start = Some(row);
                }
            }
            None
            | Some(
                EventKind::LeadingMarker
                | EventKind::TrailingMarker
                | EventKind::TimeSignature
                | EventKind::TickCount
                | EventKind::Combo
                | EventKind::Label
                | EventKind::Fake(_)
                | EventKind::Scroll(_),
            ) => {}
        }
        ClockSample {
            time,
            clamped,
            warping,
        }
    }
}

fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.to_owned()))
}
