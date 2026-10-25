//! Generation decision clock from Fumen SMCommon.SetEventTimeFromRows (MIT,
//! Fumen 0fe292f9). Gameplay/analysis keep RSSP timing. The reference generator
//! uses row-based f64 arithmetic; replacing it with beat-based queries can
//! change strict NPS threshold decisions through floating-point rounding.
use crate::{Error, document::Format, notes::ROWS_PER_BEAT};

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
        Ok(Self { events })
    }

    /// Reuse RSSP's file parser and chart inheritance rules, retaining raw f64
    /// values for the generation clock instead of narrowing through f32.
    pub fn from_simfile(bytes: &[u8], format: Format, chart: usize) -> Result<Self, Error> {
        let parsed = rssp::parse::extract_sections(bytes, format.extension())?;
        let entry = parsed
            .notes_list
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
        let timing_format = if format == Format::Ssc {
            rssp::timing::TimingFormat::Ssc
        } else {
            rssp::timing::TimingFormat::Sm
        };
        let allow = rssp::timing::steps_timing_allowed(
            rssp::parse::parse_version(parsed.version, timing_format),
            timing_format,
        );
        let source = rssp::timing::resolve_chart_timing(
            allow,
            0.0,
            entry.chart_offset.as_deref(),
            entry.chart_bpms.as_deref(),
            entry.chart_stops.as_deref(),
            entry.chart_delays.as_deref(),
            entry.chart_warps.as_deref(),
            entry.chart_speeds.as_deref(),
            entry.chart_scrolls.as_deref(),
            entry.chart_fakes.as_deref(),
            entry.chart_time_signatures.as_deref(),
            entry.chart_labels.as_deref(),
            entry.chart_tickcounts.as_deref(),
            entry.chart_combos.as_deref(),
            "",
            "",
            "",
            "",
            "",
            "",
            "",
        );
        let own = source.chart_has_own_timing;
        let fields = [
            (
                if own {
                    entry.chart_bpms.as_deref()
                } else {
                    parsed.bpms
                },
                0,
            ),
            (
                if own {
                    entry.chart_delays.as_deref()
                } else {
                    parsed.delays
                },
                1,
            ),
            (
                if own {
                    entry.chart_stops.as_deref()
                } else {
                    parsed.stops
                },
                2,
            ),
            (
                if own {
                    entry.chart_warps.as_deref()
                } else {
                    parsed.warps
                },
                3,
            ),
            (
                if own {
                    entry.chart_time_signatures.as_deref()
                } else {
                    parsed.time_signatures
                },
                6,
            ),
            (
                if own {
                    entry.chart_tickcounts.as_deref()
                } else {
                    parsed.tickcounts
                },
                7,
            ),
            (
                if own {
                    entry.chart_fakes.as_deref()
                } else {
                    parsed.fakes
                },
                8,
            ),
            (
                if own {
                    entry.chart_combos.as_deref()
                } else {
                    parsed.combos
                },
                9,
            ),
            (
                if own {
                    entry.chart_labels.as_deref()
                } else {
                    parsed.labels
                },
                10,
            ),
            (
                if own {
                    entry.chart_scrolls.as_deref()
                } else {
                    parsed.scrolls
                },
                11,
            ),
            (
                if own {
                    entry.chart_speeds.as_deref()
                } else {
                    parsed.speeds
                },
                5,
            ),
        ];
        let mut events = vec![Event {
            row: 0,
            kind: EventKind::TimeSignature,
        }];
        for (field, kind) in fields {
            let Some(field) = field else {
                continue;
            };
            let decoded = rssp::parse::decode_unescape(field);
            let cleaned = decoded
                .lines()
                .map(|line| line.split_once("//").map_or(line, |(before, _)| before))
                .collect::<Vec<_>>()
                .join("");
            for segment in cleaned
                .split(',')
                .filter(|segment| !segment.trim().is_empty())
            {
                let Some((beat, value)) = segment.trim().split_once('=') else {
                    return invalid("Invalid timing segment");
                };
                let beat = beat
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Error::InvalidFormat("Invalid timing beat".into()))?;
                if matches!(kind, 4 | 5 | 6 | 7 | 9 | 10) {
                    events.push(Event {
                        row: beat_to_row(beat)?,
                        kind: match kind {
                            4 => EventKind::LeadingMarker,
                            5 => EventKind::TrailingMarker,
                            6 => EventKind::TimeSignature,
                            7 => EventKind::TickCount,
                            9 => EventKind::Combo,
                            _ => EventKind::Label,
                        },
                    });
                    continue;
                }
                let value = value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Error::InvalidFormat("Invalid timing value".into()))?;
                let row = beat_to_row(beat)?;
                let kind = match kind {
                    0 => EventKind::Tempo(value),
                    1 => EventKind::Delay(value),
                    2 => EventKind::Stop(value),
                    3 => EventKind::Warp(beat_to_row(value)?),
                    8 => EventKind::Fake(beat_to_row(value)?),
                    _ => EventKind::Scroll(value),
                };
                events.push(Event { row, kind });
            }
        }
        if events
            .iter()
            .skip(1)
            .any(|event| event.row == 0 && matches!(event.kind, EventKind::TimeSignature))
        {
            events.remove(0);
        }
        Self::new(events)
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.events
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

fn beat_to_row(beat: f64) -> Result<i64, Error> {
    let row = (beat * ROWS_PER_BEAT as f64).round_ties_even();
    if !row.is_finite() || row < f64::from(i32::MIN) || row > f64::from(i32::MAX) {
        return invalid("Timing position exceeds the reference row range");
    }
    Ok(row as i64)
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.to_owned()))
}
