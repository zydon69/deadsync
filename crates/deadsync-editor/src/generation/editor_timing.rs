//! GrooveAuthor editor timing/dependency pass and rate queries, 48e18f71 (MIT).
//! Engine generation keeps Fumen's clock; editor density uses these distinct
//! rules, including time signatures inside warps and retroactive fake flags.
use super::{
    context::ContextEvent,
    pattern_region,
    timing::{Clock, Event, EventKind, GenerationTiming},
};
use crate::{
    Error,
    notes::{Grid, Kind, Position},
};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug)]
enum EntryKind {
    Timing(EventKind),
    Head {
        position: Position,
        kind: Kind,
        end: Option<u64>,
    },
    Region {
        end: u64,
    },
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    row: u64,
    kind: EntryKind,
    time: f64,
}
impl Entry {
    fn order(self) -> u8 {
        match self.kind {
            EntryKind::Timing(kind) => Event {
                row: self.row as i64,
                kind,
            }
            .order(),
            EntryKind::Region { .. } => 9,
            EntryKind::Head {
                kind: Kind::Tap | Kind::Fake | Kind::Lift,
                ..
            } => 8,
            EntryKind::Head {
                kind: Kind::Hold | Kind::Roll,
                ..
            } => 10,
            EntryKind::Head { .. } => 12,
        }
    }
    fn is_rate(self) -> bool {
        matches!(
            self.kind,
            EntryKind::Timing(
                EventKind::Tempo(_)
                    | EventKind::TimeSignature
                    | EventKind::Scroll(_)
                    | EventKind::Delay(_)
                    | EventKind::Stop(_)
                    | EventKind::Warp(_)
            )
        )
    }
}
#[derive(Clone, Copy, Debug)]
pub struct TimedHead {
    pub position: Position,
    pub kind: Kind,
    pub time: f64,
    pub fake: bool,
}
#[derive(Clone, Copy)]
struct Rate {
    row: u64,
    order: u8,
    time: f64,
    warp: i64,
    stop: f64,
    seconds_per_row: f64,
}
impl Rate {
    fn position_time(self, row: u64) -> f64 {
        let relative = (row as f64 - (self.row as f64 + self.warp as f64)).max(0.0);
        self.time + (relative * self.seconds_per_row + self.stop).max(0.0)
    }
}
pub struct EditorTimeline {
    entries: Vec<Entry>,
    rates: Vec<Rate>,
    heads: Vec<TimedHead>,
}
impl EditorTimeline {
    /// `region` represents the original editor's pattern event, which can
    /// extend its end-time estimate even when its last step is earlier.
    pub fn build(
        timing: &GenerationTiming,
        grid: &Grid,
        region: Option<(u64, u64)>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        if grid.players() != 1 {
            return invalid("Pattern timing requires one participant");
        }
        let mut entries = Vec::new();
        for event in timing.events() {
            check(cancelled)?;
            let row = u64::try_from(event.row).map_err(|_| {
                Error::Unsupported(
                    "Negative timing rows need reference reader normalization".into(),
                )
            })?;
            entries.push(Entry {
                row,
                kind: EntryKind::Timing(event.kind),
                time: 0.0,
            });
        }
        for (position, note) in grid.iter() {
            check(cancelled)?;
            valid_row(position.row)?;
            if let Some(end) = note.end {
                valid_row(end)?;
            }
            entries.push(Entry {
                row: position.row,
                kind: EntryKind::Head {
                    position,
                    kind: note.kind,
                    end: note.end,
                },
                time: 0.0,
            });
        }
        if let Some((start, end)) = region {
            valid_row(start)?;
            valid_row(end)?;
            if start > end {
                return invalid("Invalid pattern region");
            }
            entries.push(Entry {
                row: start,
                kind: EntryKind::Region { end },
                time: 0.0,
            });
        }
        entries.sort_by_key(|entry| {
            (
                entry.row,
                entry.order(),
                match entry.kind {
                    EntryKind::Head { position, .. } => position.lane,
                    _ => 0,
                },
            )
        });
        let mut clock = Clock::default();
        let mut heads: Vec<TimedHead> = Vec::new();
        let mut current_row = None;
        let mut heads_at_row = 0;
        let mut delay_at_row = false;
        let mut stop_at_row = false;
        let mut fake_end = 0i64;
        let mut last_negative_stop = -1.0;
        for entry in &mut entries {
            check(cancelled)?;
            if current_row != Some(entry.row) {
                current_row = Some(entry.row);
                heads_at_row = heads.len();
                delay_at_row = false;
                stop_at_row = false;
            }
            let kind = match entry.kind {
                EntryKind::Timing(kind) => Some(kind),
                _ => None,
            };
            let sample = clock.editor_event(entry.row as i64, kind);
            finite(sample.time)?;
            entry.time = sample.time;
            let fake = sample.warping
                || (sample.clamped && sample.time <= last_negative_stop)
                || fake_end > entry.row as i64;
            match entry.kind {
                EntryKind::Head { position, kind, .. } => heads.push(TimedHead {
                    position,
                    kind,
                    time: sample.time,
                    fake: fake || kind == Kind::Fake,
                }),
                EntryKind::Timing(EventKind::Fake(length)) => fake_end = entry.row as i64 + length,
                EntryKind::Timing(EventKind::Delay(_)) => delay_at_row = true,
                EntryKind::Timing(EventKind::Stop(length)) => {
                    stop_at_row = true;
                    if length < 0.0 {
                        if !delay_at_row {
                            for head in &mut heads[heads_at_row..] {
                                head.fake = true;
                            }
                        }
                        last_negative_stop = sample.time;
                    }
                }
                EntryKind::Timing(EventKind::Warp(_)) if !delay_at_row && !stop_at_row => {
                    for head in &mut heads[heads_at_row..] {
                        head.fake = true;
                    }
                }
                _ => {}
            }
        }
        let mut rates: Vec<Rate> = Vec::new();
        let mut warp = 0i64;
        let mut stop = 0.0;
        let mut seconds_per_row = 1.0;
        let mut previous_row = entries
            .iter()
            .find(|entry| entry.is_rate())
            .map_or(0, |entry| entry.row);
        let mut first_tempo = true;
        for entry in entries.iter().copied().filter(|entry| entry.is_rate()) {
            check(cancelled)?;
            let rows = (entry.row - previous_row) as i64;
            warp = (warp - rows).max(0);
            if stop != 0.0 {
                stop = (stop + rows as f64 * seconds_per_row).min(0.0);
            }
            match entry.kind {
                EntryKind::Timing(EventKind::Tempo(bpm)) => {
                    seconds_per_row = 60.0 / bpm / 48.0;
                    if first_tempo {
                        for rate in &mut rates {
                            rate.seconds_per_row = seconds_per_row;
                        }
                    }
                    first_tempo = false;
                }
                EntryKind::Timing(EventKind::Stop(length) | EventKind::Delay(length)) => {
                    stop += length
                }
                EntryKind::Timing(EventKind::Warp(length)) => warp = warp.max(length),
                _ => {}
            }
            finite(stop)?;
            finite(seconds_per_row)?;
            rates.push(Rate {
                row: entry.row,
                order: entry.order(),
                time: entry.time,
                warp,
                stop,
                seconds_per_row,
            });
            previous_row = entry.row;
        }
        Ok(Self {
            entries,
            rates,
            heads,
        })
    }
    pub fn heads(&self) -> &[TimedHead] {
        &self.heads
    }
    pub fn step_count(&self) -> usize {
        self.heads
            .iter()
            .filter(|head| {
                !head.fake && matches!(head.kind, Kind::Tap | Kind::Hold | Kind::Roll | Kind::Lift)
            })
            .count()
    }
    pub fn context_events(&self) -> Vec<ContextEvent> {
        self.entries
            .iter()
            .map(|entry| match entry.kind {
                EntryKind::Head { position, kind, .. } => ContextEvent {
                    row: entry.row,
                    lane: Some(position.lane),
                    is_step: pattern_region::is_step(kind),
                },
                _ => ContextEvent {
                    row: entry.row,
                    lane: None,
                    is_step: false,
                },
            })
            .collect()
    }
    pub fn position_time(&self, row: u64) -> Result<f64, Error> {
        self.query(row, None)
    }
    fn query(&self, row: u64, order: Option<u8>) -> Result<f64, Error> {
        valid_row(row)?;
        let index = self.rates.partition_point(|rate| {
            rate.row < row || (rate.row == row && order.is_some_and(|order| rate.order < order))
        });
        let rate = index
            .checked_sub(1)
            .and_then(|index| self.rates.get(index))
            .or_else(|| self.rates.first());
        let time = rate.map_or(0.0, |rate| rate.position_time(row));
        finite(time)?;
        Ok(time)
    }
    /// Uses the final editor event's end, rather than the maximum hold tail.
    /// This distinction is observable when later timing metadata follows a head.
    pub fn chart_end(&self) -> Result<f64, Error> {
        let Some(entry) = self.entries.last().copied() else {
            return Ok(0.0);
        };
        let time = match entry.kind {
            EntryKind::Head { end: Some(end), .. } => self.query(end, Some(11))?,
            EntryKind::Region { end } => self.position_time(end)?,
            EntryKind::Timing(EventKind::Fake(length)) => {
                self.position_time(entry.row + length as u64)?
            }
            EntryKind::Timing(EventKind::Delay(length)) => entry.time + length,
            EntryKind::Timing(EventKind::Stop(length)) if length > 0.0 => entry.time + length,
            _ => entry.time,
        };
        finite(time)?;
        Ok(time)
    }
    pub fn pattern_nps(&self, num_steps: u64, last_step: u64) -> Result<f64, Error> {
        if num_steps > i32::MAX as u64 {
            return invalid("Pattern step count exceeds the reference range");
        }
        let end = self.chart_end()?.max(self.position_time(last_step)?);
        let nps = if end > 0.0 {
            (self.step_count() as f64 + num_steps as f64) / end
        } else {
            0.0
        };
        finite(nps)?;
        Ok(nps)
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        return invalid("Editor timing preparation cancelled");
    }
    Ok(())
}
fn valid_row(row: u64) -> Result<(), Error> {
    if row > i32::MAX as u64 {
        return invalid("Editor timing row exceeds the reference range");
    }
    Ok(())
}
fn finite(value: f64) -> Result<(), Error> {
    if !value.is_finite() {
        return invalid("Editor timing produced a non-finite value");
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
