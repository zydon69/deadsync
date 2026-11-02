//! Source fixed-row measure density and stream notation. This is not RSSP NPS.
pub mod colors;
pub mod layout;
pub mod mesh;
pub mod task;
use crate::{Error, density_preferences::Accumulation};
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamSettings {
    pub threshold: u32,
    pub accumulation: Accumulation,
    pub show_break_lengths: bool,
    pub minimum_length: usize,
    pub short_break_cutoff: usize,
    pub short_break: char,
    pub long_break: char,
}
impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            threshold: 16,
            accumulation: Accumulation::Step,
            show_break_lengths: false,
            minimum_length: 1,
            short_break_cutoff: 4,
            short_break: '-',
            long_break: '|',
        }
    }
}
impl StreamSettings {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        Ok(crate::stream_preferences::Preferences::read(fields)?.density_settings())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub row: u64,
    pub step: bool,
    pub fake: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    pub start_time: f64,
    pub steps: u32,
    pub rows: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stream {
    pub start: usize,
    pub last: usize,
}
#[derive(Clone, Debug)]
pub struct Series {
    pub measures: Vec<Measure>,
    pub final_time: f64,
    pub streams: Vec<Stream>,
    pub breakdown: Option<String>,
    /// Source stores byte counts; dense native charts retain true counts instead of wrapping.
    pub source_byte_overflows: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub has_steps: bool,
    pub time: f64,
    pub rate: f64,
    pub percent: f64,
}
#[derive(Clone, Debug)]
pub struct Plot {
    pub samples: Vec<Sample>,
    pub peak: f64,
    pub aggregated: bool,
}
impl Series {
    pub fn build(
        events: impl IntoIterator<Item = Event>,
        times: Vec<f64>,
        final_time: f64,
        settings: StreamSettings,
        max_events: usize,
    ) -> Result<Self, Error> {
        Self::build_checked(events, times, final_time, settings, max_events, || Ok(()))
    }
    pub(crate) fn build_checked(
        events: impl IntoIterator<Item = Event>,
        times: Vec<f64>,
        final_time: f64,
        settings: StreamSettings,
        max_events: usize,
        mut checkpoint: impl FnMut() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        if times.is_empty()
            || times.len() > 262144
            || times.iter().any(|t| !t.is_finite())
            || !final_time.is_finite()
        {
            return Err(invalid("Invalid or oversized density time series"));
        }
        let mut measures: Vec<_> = times
            .into_iter()
            .map(|start_time| Measure {
                start_time,
                ..Default::default()
            })
            .collect();
        let mut streams = Vec::new();
        let mut current = None;
        let mut steps = 0u32;
        let mut rows = 0u32;
        let mut last_row = None;
        let mut start = None;
        let mut last = 0usize;
        let mut previous_row = 0;
        let commit = |streams: &mut Vec<Stream>, start: &mut Option<usize>, last: usize| {
            if let Some(start) = start.take() {
                streams.push(Stream { start, last });
            }
        };
        for (i, event) in events.into_iter().enumerate() {
            if i % 512 == 0 {
                checkpoint()?;
            }
            if i >= max_events {
                return Err(invalid("Density event budget exceeded"));
            }
            if i > 0 && event.row < previous_row {
                return Err(invalid("Density events must retain source row order"));
            }
            previous_row = event.row;
            let m =
                usize::try_from(event.row / 192).map_err(|_| invalid("Density row overflow"))?;
            if m >= measures.len() {
                return Err(invalid("Density event exceeds the declared end measure"));
            }
            if current != Some(m) {
                if let Some(old) = current {
                    measures[old].steps = steps;
                    measures[old].rows = rows;
                }
                let count = if settings.accumulation == Accumulation::Step {
                    steps
                } else {
                    rows
                };
                if count < settings.threshold {
                    commit(&mut streams, &mut start, last);
                }
                current = Some(m);
                steps = 0;
                rows = 0;
            }
            if event.step && !event.fake {
                steps = steps
                    .checked_add(1)
                    .ok_or_else(|| invalid("Density step overflow"))?;
                if last_row != Some(event.row) {
                    rows += 1;
                }
                let count = if settings.accumulation == Accumulation::Step {
                    steps
                } else {
                    rows
                };
                if count >= settings.threshold {
                    if start.is_none() {
                        start = Some(m);
                    }
                    last = m;
                }
                last_row = Some(event.row);
            }
        }
        if let Some(m) = current {
            measures[m].steps = steps;
            measures[m].rows = rows;
        }
        commit(&mut streams, &mut start, last);
        let mut breakdown = String::new();
        let mut previous = None;
        for stream in &streams {
            let length = stream.last - stream.start + 1;
            if length < settings.minimum_length {
                continue;
            }
            if let Some(last) = previous {
                let gap = stream.start - (last + 1);
                if gap <= settings.short_break_cutoff {
                    breakdown.push(settings.short_break);
                } else if settings.show_break_lengths {
                    breakdown.push_str(&format!(" ({gap}) "));
                } else {
                    breakdown.push(settings.long_break);
                }
            }
            breakdown.push_str(&length.to_string());
            previous = Some(stream.last);
        }
        for (i, m) in measures.iter().enumerate() {
            let duration = measures.get(i + 1).map_or(final_time, |m| m.start_time) - m.start_time;
            if duration > 0.0
                && (!(m.steps as f64 / duration).is_finite()
                    || !(m.rows as f64 / duration).is_finite())
            {
                return Err(invalid("Density rates exceed the finite numeric range"));
            }
        }
        let source_byte_overflows = measures
            .iter()
            .filter(|m| m.steps > u8::MAX as u32 || m.rows > u8::MAX as u32)
            .count();
        Ok(Self {
            measures,
            final_time,
            streams,
            breakdown: (!breakdown.is_empty()).then_some(breakdown),
            source_byte_overflows,
        })
    }
    pub fn plot(&self, mode: Accumulation, limit: usize) -> Result<Plot, Error> {
        if limit == 0 {
            return Err(invalid("Density plot needs a nonzero sample limit"));
        }
        let all = self.samples(mode);
        let stride = all.len().div_ceil(limit).max(1);
        let samples = all
            .chunks(stride)
            .filter_map(|chunk| chunk.iter().max_by(|a, b| a.percent.total_cmp(&b.percent)))
            .copied()
            .collect();
        Ok(Plot {
            samples,
            peak: self.peak(mode),
            aggregated: stride > 1,
        })
    }
    pub fn peak(&self, mode: Accumulation) -> f64 {
        self.measures
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let duration = self
                    .measures
                    .get(i + 1)
                    .map_or(self.final_time, |m| m.start_time)
                    - m.start_time;
                (duration > 0.0).then(|| {
                    if mode == Accumulation::Step {
                        m.steps as f64 / duration
                    } else {
                        m.rows as f64 / duration
                    }
                })
            })
            .fold(0.0, f64::max)
    }
    pub fn samples(&self, mode: Accumulation) -> Vec<Sample> {
        let peak = self.peak(mode);
        self.measures
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let duration = self
                    .measures
                    .get(i + 1)
                    .map_or(self.final_time, |m| m.start_time)
                    - m.start_time;
                let count = if mode == Accumulation::Step {
                    m.steps
                } else {
                    m.rows
                };
                let rate = if m.steps > 0 && !crate::number::reference_equal(duration, 0.0) {
                    count as f64 / duration
                } else {
                    0.0
                };
                Sample {
                    has_steps: m.steps > 0,
                    time: m.start_time,
                    rate,
                    percent: if peak > 0.0 { rate / peak } else { 0.0 },
                }
            })
            .collect()
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
