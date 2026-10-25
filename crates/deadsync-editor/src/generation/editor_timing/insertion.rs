//! Row dependencies for generated taps, without refreshing surviving events.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl EditorTimeline {
    /// The editor inserts generated taps using typed rate queries and
    /// IsEventInFake. Rebuilding the full clock would retime surviving notes.
    pub fn with_taps(
        &self,
        taps: impl IntoIterator<Item = Position>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        self.insert_taps(taps, false, cancelled)
    }
    /// Keep repeated generated taps in working context until file export.
    pub(crate) fn with_pattern_taps(
        &self,
        taps: impl IntoIterator<Item = Position>,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        self.insert_taps(taps, true, cancelled)
    }
    fn insert_taps(
        &self,
        taps: impl IntoIterator<Item = Position>,
        allow_repeated: bool,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        let mut occupied = BTreeMap::new();
        for entry in &self.entries {
            check(cancelled)?;
            if let EntryKind::Head {
                position,
                kind,
                end,
            } = entry.kind
            {
                occupied.insert((position.lane, position.row), (kind, end));
            }
        }
        let mut selected = BTreeSet::new();
        for position in taps {
            check(cancelled)?;
            valid_row(position.row)?;
            if position.player != 0 || position.lane >= self.columns || !selected.insert(position) {
                return invalid(
                    "Generated tap has an invalid participant, lane or duplicate position",
                );
            }
            if let Some((&(lane, row), (kind, end))) =
                occupied.range(..=(position.lane, position.row)).next_back()
            {
                if lane == position.lane
                    && (row == position.row || end.is_some_and(|end| end >= position.row))
                    && !(allow_repeated
                        && row == position.row
                        && *kind == Kind::Tap
                        && end.is_none())
                {
                    return invalid("Generated tap overlaps an existing note");
                }
            }
        }
        let fakes = InsertionFakes::build(self, cancelled)?;
        let mut entries = self.entries.clone();
        let mut heads = self.heads.clone();
        for position in selected {
            check(cancelled)?;
            let time = self.query(position.row, Some(8))?;
            entries.push(Entry {
                row: position.row,
                kind: EntryKind::Head {
                    position,
                    kind: Kind::Tap,
                    end: None,
                },
                time,
            });
            heads.push(TimedHead {
                position,
                kind: Kind::Tap,
                time,
                fake: fakes.contains(position.row, time),
            });
        }
        entries.sort_by(|a, b| event_cmp(a.key(), b.key()));
        heads.sort_by_key(|head| {
            Entry {
                row: head.position.row,
                kind: EntryKind::Head {
                    position: head.position,
                    kind: head.kind,
                    end: None,
                },
                time: head.time,
            }
            .key()
        });
        check(cancelled)?;
        Ok(Self {
            columns: self.columns,
            entries,
            heads,
            rates: self.rates.clone(),
        })
    }
}
struct StopProbe {
    row: u64,
    time: f64,
    negative: bool,
    end: f64,
}
struct InsertionFakes {
    fakes: Vec<(u64, u64)>,
    warps: Vec<(u64, u64)>,
    stops: Vec<StopProbe>,
    delays: BTreeSet<u64>,
}
impl InsertionFakes {
    fn build(timeline: &EditorTimeline, cancelled: &AtomicBool) -> Result<Self, Error> {
        let mut fakes = Vec::new();
        let mut raw_warps = Vec::new();
        let mut stops = Vec::new();
        let mut delays = BTreeSet::new();
        let mut stop_rows = BTreeSet::new();
        for entry in &timeline.entries {
            check(cancelled)?;
            match entry.kind {
                EntryKind::Timing(EventKind::Delay(_)) => {
                    delays.insert(entry.row);
                }
                EntryKind::Timing(EventKind::Stop(_)) => {
                    stop_rows.insert(entry.row);
                }
                _ => {}
            }
        }
        for entry in &timeline.entries {
            check(cancelled)?;
            match entry.kind {
                EntryKind::Timing(EventKind::Fake(length)) => {
                    fakes.push((entry.row, entry.row + length as u64))
                }
                EntryKind::Timing(EventKind::Warp(length)) => {
                    let start = entry.row
                        + u64::from(delays.contains(&entry.row) || stop_rows.contains(&entry.row));
                    let end = entry.row + length as u64;
                    if start < end {
                        raw_warps.push((start, end));
                    }
                }
                EntryKind::Timing(EventKind::Stop(length)) => {
                    let mut end = entry.row as f64;
                    if length < 0.0 {
                        let first = timeline.rates.partition_point(|rate| {
                            (rate.row, rate.order) < (entry.row, entry.order())
                        });
                        for rate in timeline.rates[first..]
                            .iter()
                            .take_while(|rate| rate.time <= entry.time)
                        {
                            check(cancelled)?;
                            let relative = entry.time - (rate.time + rate.stop);
                            let relative = if entry.time >= 0.0 {
                                relative.max(0.0)
                            } else {
                                relative
                            };
                            end = rate.row as f64
                                + relative * (1.0 / rate.seconds_per_row)
                                + rate.warp as f64;
                        }
                        finite(end)?;
                    }
                    stops.push(StopProbe {
                        row: entry.row,
                        time: entry.time,
                        negative: length < 0.0,
                        end,
                    });
                }
                _ => {}
            }
        }
        // Fake segments replace earlier fake ends. Warps instead form a union.
        // Index both once for the entire insertion, rather than scan timing for
        // every generated tap.
        raw_warps.sort_unstable();
        let mut warps: Vec<(u64, u64)> = Vec::new();
        for (start, end) in raw_warps {
            check(cancelled)?;
            if let Some(previous) = warps.last_mut().filter(|previous| start <= previous.1) {
                previous.1 = previous.1.max(end);
            } else {
                warps.push((start, end));
            }
        }
        stops.sort_by(|a, b| a.time.total_cmp(&b.time));
        check(cancelled)?;
        Ok(Self {
            fakes,
            warps,
            stops,
            delays,
        })
    }
    fn contains(&self, row: u64, time: f64) -> bool {
        if self
            .fakes
            .partition_point(|segment| segment.0 <= row)
            .checked_sub(1)
            .is_some_and(|index| row < self.fakes[index].1)
        {
            return true;
        }
        if self
            .warps
            .partition_point(|segment| segment.0 <= row)
            .checked_sub(1)
            .is_some_and(|index| row < self.warps[index].1)
        {
            return true;
        }
        let end = self.stops.partition_point(|stop| stop.time <= time);
        for stop in self.stops[..end].iter().rev() {
            if time > stop.time {
                return false;
            }
            if stop.negative {
                if stop.row == row && self.delays.contains(&row) {
                    return false;
                }
                return (row as f64) < stop.end;
            }
        }
        false
    }
}
