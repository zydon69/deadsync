//! GrooveAuthor editor timing/dependency pass and rate queries, 48e18f71 (MIT).
//! Engine generation keeps Fumen's clock; editor density uses these distinct
//! rules, including time signatures inside warps and retroactive fake flags.
mod insertion;

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
    Attack,
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
            EntryKind::Attack => 6,
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
    fn key(self) -> (u64, u8, u16) {
        (
            self.row,
            self.order(),
            match self.kind {
                EntryKind::Head { position, .. } => position.lane,
                _ => 0,
            },
        )
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
    tempo: f64,
    scroll: f64,
}
impl Rate {
    fn position_time(self, row: u64) -> f64 {
        crate::rate_mapping::rate_time(
            self.row as f64, self.time, self.warp as f64,
            self.stop, self.seconds_per_row, row as f64,
        )
    }
}
/// The initialized state of one original rate-altering event. Speed interpolation
/// is supplied separately; it is not equivalent to this constant scroll value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingSection {
    pub order: u8,
    pub mapping: crate::rate_mapping::RateMapping,
    pub tempo: f64,
    pub scroll: f64,
}
impl SpacingSection {
    pub fn spacing_rate(self) -> crate::spacing::Rate {
        crate::spacing::Rate {
            row: self.mapping.row(),
            time: self.mapping.time(),
            tempo: self.tempo,
            seconds_per_row: self.mapping.seconds_per_row(),
            rows_per_second: self.mapping.rows_per_second(),
            scroll: self.scroll,
        }
    }
}
pub struct EditorTimeline {
    columns: u16,
    entries: Vec<Entry>,
    rates: Vec<Rate>,
    rates_by_time: Vec<usize>,
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
        Self::build_with_attacks(timing, grid, region, &[], cancelled)
    }
    /// Build the generation clock with scheduled, non-performance events.
    /// Attack rows use their editor-assigned positions and precede delays at
    /// the same row, just as they do in the original event ordering.
    pub fn build_with_attacks(
        timing: &GenerationTiming,
        grid: &Grid,
        region: Option<(u64, u64)>,
        attacks: &[u64],
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        Self::build_internal(timing, grid, region, attacks, false, cancelled)
    }
    /// Resident editor clock includes actual note heads and attack phase six.
    /// Multiple participants are valid here; generation retains its own guard.
    pub(crate) fn build_clock(
        timing: &GenerationTiming,
        grid: &Grid,
        attacks: &[u64],
    ) -> Result<Self, Error> {
        Self::build_internal(timing, grid, None, attacks, true, &AtomicBool::new(false))
    }
    fn build_internal(
        timing: &GenerationTiming,
        grid: &Grid,
        region: Option<(u64, u64)>,
        attacks: &[u64],
        allow_players: bool,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        if !allow_players && grid.players() != 1 {
            return invalid("Pattern timing requires one participant");
        }
        let mut entries = Vec::new();
        for event in timing.editor_events() {
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
        for &row in attacks {
            check(cancelled)?;
            valid_row(row)?;
            entries.push(Entry {
                row,
                kind: EntryKind::Attack,
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
        entries.sort_by(|a, b| event_cmp(a.key(), b.key()));
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
        let mut tempo = 1.0;
        let mut scroll = 1.0;
        let mut first_scroll = true;
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
                    tempo = bpm;
                    seconds_per_row = 60.0 / bpm / 48.0;
                    if first_tempo {
                        for rate in &mut rates {
                            rate.seconds_per_row = seconds_per_row;
                            rate.tempo = tempo;
                        }
                    }
                    first_tempo = false;
                }
                EntryKind::Timing(EventKind::Scroll(value)) => {
                    scroll = value;
                    if first_scroll {
                        for rate in &mut rates {
                            rate.scroll = scroll;
                        }
                    }
                    first_scroll = false;
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
                tempo,
                scroll,
            });
            previous_row = entry.row;
        }
        let mut rates_by_time: Vec<_> = (0..rates.len()).collect();
        rates_by_time.sort_by(|&a, &b| rates[a].time.total_cmp(&rates[b].time));
        Ok(Self {
            columns: grid.columns(),
            entries,
            rates,
            rates_by_time,
            heads,
        })
    }
    /// The original loader adds custom pattern events after refreshing the
    /// chart, using typed rate queries without changing assigned note times.
    pub fn with_markers(
        mut self,
        regions: &[(u64, u64)],
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        let mut known = std::collections::BTreeMap::new();
        for entry in &self.entries {
            check(cancelled)?;
            if let EntryKind::Region { end } = entry.kind {
                known.insert(entry.row, end);
            }
        }
        for &(start, end) in regions {
            check(cancelled)?;
            valid_row(start)?;
            valid_row(end)?;
            if start > end {
                return invalid("Invalid loaded pattern region");
            }
            if let Some(old) = known.insert(start, end) {
                if old != end {
                    return invalid("Conflicting pattern markers at the same row");
                }
                continue;
            }
            let time = self.query(start, Some(9))?;
            self.entries.push(Entry {
                row: start,
                kind: EntryKind::Region { end },
                time,
            });
        }
        self.entries.sort_by(|a, b| event_cmp(a.key(), b.key()));
        check(cancelled)?;
        Ok(self)
    }

    /// Apply lane-only deletions/hold shortening without recalculating the
    /// assigned times, fake flags or rate states of surviving events.
    /// A new pattern marker obtains its time from the existing typed rate query.
    pub fn project(
        &self,
        retained: &Grid,
        region: Option<(u64, u64)>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        if retained.columns() != self.columns || retained.players() != 1 {
            return invalid("Projected grid does not match its editor timeline");
        }
        let mut entries = Vec::new();
        let mut found = 0;
        for entry in self.entries.iter().copied() {
            check(cancelled)?;
            match entry.kind {
                EntryKind::Head {
                    position,
                    kind,
                    end,
                } => {
                    if let Some(note) = retained.get(position) {
                        if note.kind != kind
                            || note.end.is_some() != end.is_some()
                            || note.end.zip(end).is_some_and(|(new, old)| new > old)
                        {
                            return invalid(
                                "Timeline projection only supports removal and hold shortening",
                            );
                        }
                        entries.push(Entry {
                            kind: EntryKind::Head {
                                position,
                                kind,
                                end: note.end,
                            },
                            ..entry
                        });
                        found += 1;
                    }
                }
                _ => entries.push(entry),
            }
        }
        let mut expected = 0;
        for _ in retained.iter() {
            check(cancelled)?;
            expected += 1;
        }
        if found != expected {
            return invalid("Projected notes were not present in the original timeline");
        }
        if let Some((start, end)) = region {
            valid_row(start)?;
            valid_row(end)?;
            if start > end {
                return invalid("Invalid projected pattern region");
            }
            if !entries.iter().any(|entry| {
                entry.row == start && matches!(entry.kind,EntryKind::Region {end:old} if old==end)
            }) {
                let entry = Entry {
                    row: start,
                    kind: EntryKind::Region { end },
                    time: self.query(start, Some(9))?,
                };
                let index = entries
                    .partition_point(|existing| event_cmp(existing.key(), entry.key()).is_le());
                entries.insert(index, entry);
            }
        }
        let mut heads = Vec::new();
        for head in self.heads.iter().copied() {
            check(cancelled)?;
            if retained.get(head.position).is_some() {
                heads.push(head);
            }
        }
        let mut rates = Vec::with_capacity(self.rates.len());
        for rate in self.rates.iter().copied() {
            check(cancelled)?;
            rates.push(rate);
        }
        check(cancelled)?;
        Ok(Self {
            columns: self.columns,
            entries,
            rates,
            rates_by_time: self.rates_by_time.clone(),
            heads,
        })
    }

    /// Extract complete hold pairs and local non-timing metadata, then include
    /// every global timing event exactly as the original pattern action does.
    /// Frames retain the time of the first included event at their row.
    pub fn prepare_region(
        &self,
        grid: &Grid,
        range: Option<(u64, u64)>,
        cancelled: &AtomicBool,
    ) -> Result<PreparedRegion, Error> {
        use super::expressed;
        check(cancelled)?;
        if grid.columns() != self.columns || grid.players() != 1 {
            return invalid("Context grid does not match its timeline");
        }
        if let Some((start, end)) = range {
            valid_row(start)?;
            valid_row(end)?;
        }
        let (selected, head_events) =
            if let Some((start, end)) = range.filter(|(start, end)| start <= end) {
                pattern_region::extract(grid, 0, start, end, cancelled)?
            } else {
                (Grid::new(grid.columns(), 1)?, Vec::new())
            };
        // Movement frames have one lane state, while the original action's
        // editor-event counters retain repeated generated tap instances.
        let mut repeated_events = Vec::new();
        for event in head_events {
            check(cancelled)?;
            let position = Position {
                player: 0,
                lane: event.lane.unwrap(),
                row: event.row,
            };
            let note = selected
                .get(position)
                .ok_or_else(|| Error::InvalidEdit("Context note is missing".into()))?;
            let key = Entry {
                row: event.row,
                kind: EntryKind::Head {
                    position,
                    kind: note.kind,
                    end: note.end,
                },
                time: 0.0,
            }
            .key();
            let first = self
                .entries
                .partition_point(|entry| event_cmp(entry.key(), key).is_lt());
            let count = self.entries[first..]
                .iter()
                .take_while(|entry| entry.key() == key)
                .count();
            for _ in 0..count.max(1) {
                check(cancelled)?;
                repeated_events.push(event);
            }
        }
        let head_events = repeated_events;
        let mut included: Vec<(u64, u8, u16, f64)> = Vec::new();
        for (position, note) in selected.iter() {
            check(cancelled)?;
            let probe = Entry {
                row: position.row,
                kind: EntryKind::Head {
                    position,
                    kind: note.kind,
                    end: note.end,
                },
                time: 0.0,
            };
            let index = self
                .entries
                .binary_search_by(|entry| event_cmp(entry.key(), probe.key()))
                .map_err(|_| {
                    Error::InvalidEdit("Context head is absent from its timeline".into())
                })?;
            let original = self.entries[index];
            if !matches!(original.kind,EntryKind::Head {kind,end,..} if kind==note.kind && end==note.end)
            {
                return invalid("Context hold differs from its timeline");
            }
            included.push((position.row, probe.order(), position.lane, original.time));
            if let Some(end) = note.end {
                included.push((end, 11, position.lane, self.query(end, Some(11))?));
            }
        }
        let mut metadata = Vec::new();
        for entry in self.entries.iter().copied() {
            check(cancelled)?;
            if matches!(entry.kind, EntryKind::Attack)
                && range.is_some_and(|(start, end)| start <= entry.row && entry.row <= end)
            {
                included.push((entry.row, entry.order(), 0, entry.time));
                metadata.push((
                    entry.row,
                    entry.order(),
                    0,
                    ContextEvent {
                        row: entry.row,
                        lane: None,
                        is_step: false,
                    },
                ));
            }
            if let EntryKind::Timing(kind) = entry.kind {
                let clock = matches!(
                    kind,
                    EventKind::TimeSignature
                        | EventKind::Tempo(_)
                        | EventKind::Stop(_)
                        | EventKind::Delay(_)
                        | EventKind::Warp(_)
                );
                let local =
                    range.is_some_and(|(start, end)| start <= entry.row && entry.row <= end);
                if clock || local {
                    included.push((entry.row, entry.order(), 0, entry.time));
                }
                if !clock && local {
                    metadata.push((
                        entry.row,
                        entry.order(),
                        0,
                        ContextEvent {
                            row: entry.row,
                            lane: None,
                            is_step: false,
                        },
                    ));
                }
            }
        }
        included.sort_by(|a, b| event_cmp((a.0, a.1, a.2), (b.0, b.1, b.2)));
        let frames = expressed::frames(&selected, 0, cancelled)?;
        let mut times = Vec::with_capacity(frames.len());
        for frame in &frames {
            check(cancelled)?;
            let index = included.partition_point(|event| event.0 < frame.row);
            let first = included
                .get(index)
                .filter(|event| event.0 == frame.row)
                .ok_or_else(|| Error::InvalidEdit("Context frame has no timed event".into()))?;
            times.push(first.3);
        }
        let overlaps = if let Some((start, _)) = range {
            selected
                .iter()
                .filter(|(position, note)| note.end.is_some() && position.row <= start)
                .count()
        } else {
            0
        };
        let mut editor_events = head_events[..overlaps].to_vec();
        for event in &head_events[overlaps..] {
            check(cancelled)?;
            let position = Position {
                player: 0,
                lane: event.lane.unwrap(),
                row: event.row,
            };
            let note = selected.get(position).unwrap();
            let entry = Entry {
                row: position.row,
                kind: EntryKind::Head {
                    position,
                    kind: note.kind,
                    end: note.end,
                },
                time: 0.0,
            };
            metadata.push((event.row, entry.order(), position.lane, *event));
        }
        metadata.sort_by(|a, b| event_cmp((a.0, a.1, a.2), (b.0, b.1, b.2)));
        editor_events.extend(metadata.into_iter().map(|event| event.3));
        let event_span = [
            included.first().map_or(0.0, |event| event.3),
            included.last().map_or(0.0, |event| event.3),
        ];
        check(cancelled)?;
        Ok(PreparedRegion {
            frames,
            times,
            event_span,
            editor_events,
        })
    }

    pub(crate) fn attack_times(&self) -> impl Iterator<Item = (u64, f64)> + '_ {
        self.entries.iter().filter_map(|entry| {
            matches!(entry.kind, EntryKind::Attack).then_some((entry.row, entry.time))
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
    /// Ordered rate snapshots for spacing traversal. The order byte preserves
    /// distinct timing events at the same row. Constructed only after validation.
    pub fn rate_mappings(&self) -> impl Iterator<Item = (u8, crate::rate_mapping::RateMapping)> + '_ {
        self.rates.iter().map(|rate| {
            let mapping = crate::rate_mapping::RateMapping::new(
                rate.row as f64, rate.time, rate.warp as f64,
                rate.stop, rate.seconds_per_row,
            ).expect("validated editor timing rate");
            (rate.order, mapping)
        })
    }
    /// Ordered initialized tempo/scroll state, including original backfill of
    /// events preceding the first tempo or first constant-scroll event.
    pub fn spacing_sections(&self) -> impl Iterator<Item = SpacingSection> + '_ {
        self.rates.iter().zip(self.rate_mappings()).map(|(rate, (order, mapping))| {
            SpacingSection { order, mapping, tempo: rate.tempo, scroll: rate.scroll }
        })
    }
    /// A SPEEDS event follows delays/notes/SCROLLS and precedes ordinary stops
    /// and warps at the same row. Plain position_time uses a different phase.
    pub fn speed_event_time(&self, row: u64) -> Result<f64, Error> {
        self.query(row, Some(Event { row: row as i64, kind: EventKind::TrailingMarker }.order()))
    }
    /// Note heads and hold tails follow same-row delays, before scroll/speed,
    /// ordinary stops and warps. All note phases share this rate-event boundary.
    pub fn note_time(&self, row: u64) -> Result<f64, Error> {
        self.query(row, Some(8))
    }
    pub fn position_time(&self, row: u64) -> Result<f64, Error> {
        self.query(row, None)
    }
    /// Continuous cursor position, preceding rate events at an equal row.
    /// Unlike note editing, presentation positions may be fractional or negative.
    pub fn fractional_position_time(&self, row: f64) -> Result<f64, Error> {
        finite(row)?;
        let index = self.rates.partition_point(|rate| (rate.row as f64) < row);
        let rate = index.checked_sub(1)
            .and_then(|index| self.rates.get(index))
            .or_else(|| self.rates.first());
        let time = rate.map_or(0.0, |rate| crate::rate_mapping::rate_time(
            rate.row as f64, rate.time, rate.warp as f64,
            rate.stop, rate.seconds_per_row, row,
        ));
        finite(time)?;
        Ok(time)
    }
    /// Fractional chart row at a chart time, before applying the music offset.
    /// Like GrooveAuthor, an exact boundary precedes every rate event with
    /// that time. Negative time extrapolates into the lead-in; positive stops
    /// hold the cursor, while warps and negative stops advance its position.
    pub fn time_position(&self, time: f64) -> Result<f64, Error> {
        finite(time)?;
        let index = self
            .rates_by_time
            .partition_point(|&index| self.rates[index].time < time);
        let rate = index
            .checked_sub(1)
            .and_then(|index| self.rates_by_time.get(index))
            .or_else(|| self.rates_by_time.first())
            .map(|&index| self.rates[index]);
        let Some(rate) = rate else {
            return invalid("Cannot locate playback without a timing rate");
        };
        let row = crate::rate_mapping::rate_position(rate.row as f64, rate.time,
            rate.warp as f64, rate.stop, 1.0 / rate.seconds_per_row, time);
        finite(row)?;
        Ok(row)
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
        self.patterns_nps(num_steps, Some(last_step))
    }
    /// A batch with only empty markers contributes no candidate step time.
    pub fn patterns_nps(&self, num_steps: u64, last_step: Option<u64>) -> Result<f64, Error> {
        if num_steps > i32::MAX as u64 {
            return invalid("Pattern step count exceeds the reference range");
        }
        let end = self.chart_end()?.max(
            last_step
                .map(|row| self.position_time(row))
                .transpose()?
                .unwrap_or(0.0),
        );
        let nps = if end > 0.0 {
            (self.step_count() as f64 + num_steps as f64) / end
        } else {
            0.0
        };
        finite(nps)?;
        Ok(nps)
    }
}
pub struct PreparedRegion {
    pub frames: Vec<super::expressed::Frame>,
    pub times: Vec<f64>,
    pub event_span: [f64; 2],
    pub editor_events: Vec<ContextEvent>,
}
impl PreparedRegion {
    pub fn request(&self, difficulty: f64) -> super::expressed::PolicyRequest<'_> {
        super::expressed::PolicyRequest {
            frames: &self.frames,
            times: &self.times,
            event_span: self.event_span,
            difficulty,
        }
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

// SMEventComparer compares lanes before types whenever both events are notes.
fn event_cmp(a: (u64, u8, u16), b: (u64, u8, u16)) -> std::cmp::Ordering {
    a.0.cmp(&b.0).then_with(|| {
        let lane = |order| matches!(order, 8 | 10 | 11 | 12);
        if lane(a.1) && lane(b.1) {
            a.2.cmp(&b.2).then(a.1.cmp(&b.1))
        } else {
            a.1.cmp(&b.1)
        }
    })
}
