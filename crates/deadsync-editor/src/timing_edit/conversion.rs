//! Reviewed Warp / negative Stop conversion using the original rate-event clock.
use super::{
    Scope, Target,
    pause::{Kind as PauseKind, Pauses},
    region::{Kind as RegionKind, Regions},
};
use crate::{
    Error,
    document::Format,
    generation::{
        editor_timing::EditorTimeline,
        timing::{Event, EventKind, GenerationTiming},
    },
    history::Session,
    notes::Grid,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    WarpsToNegativeStops,
    NegativeStopsToWarps,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub converted: usize,
    /// Source action deletes these originals without replacing the opposite
    /// kind already at their row. Every row requires explicit acknowledgement.
    pub deleted_on_collision: BTreeSet<u64>,
    pub overlaps: BTreeSet<u64>,
    pub selection_after: Vec<u64>,
}
pub struct Plan {
    target: Target,
    chart: usize,
    stops: String,
    warps: String,
    report: Report,
}
impl Plan {
    /// None selects all eligible events; Some keeps the given ordered selection.
    /// The effective chart scope is used, including shared SM/legacy timing.
    pub fn prepare(
        session: &Session,
        chart: usize,
        direction: Direction,
        selected: Option<&[u64]>,
    ) -> Result<Self, Error> {
        let document = session.document();
        let scope = if super::uses_chart_timing(document, chart) {
            Scope::Chart
        } else {
            Scope::Song
        };
        let target = Target::capture(session, chart, scope)?;
        let origin = target.origin();
        let mut stops = Pauses::parse(
            PauseKind::Stop,
            &document.text(origin, "STOPS").unwrap_or_default(),
        )?;
        let mut warps = Regions::parse(
            RegionKind::Warp,
            &document.text(origin, "WARPS").unwrap_or_default(),
        )?;
        let timing = GenerationTiming::from_document(document, chart)?;
        let original = timeline(&timing)?;
        let phase = match direction {
            Direction::WarpsToNegativeStops => 17,
            Direction::NegativeStopsToWarps => 16,
        };
        let times: BTreeMap<_, _> = original
            .rate_mappings()
            .filter(|(order, _)| *order == phase)
            .map(|(_, rate)| (rate.row() as u64, rate.time()))
            .collect();
        let eligible: BTreeMap<u64, f64> = match direction {
            Direction::WarpsToNegativeStops => warps
                .values()
                .iter()
                .map(|(&row, &length)| (row, length as f64))
                .collect(),
            Direction::NegativeStopsToWarps => stops
                .values()
                .iter()
                .filter(|(_, length)| **length < 0.0)
                .map(|(&row, &length)| (row, length))
                .collect(),
        };
        let mut seen = BTreeSet::new();
        let rows: Vec<_> = match selected {
            None => eligible.keys().copied().collect(),
            Some(rows) => rows
                .iter()
                .copied()
                .filter(|row| eligible.contains_key(row) && seen.insert(*row))
                .collect(),
        };
        if direction == Direction::NegativeStopsToWarps
            && !rows.is_empty()
            && document.format == Format::Sm
        {
            return Err(invalid(
                "Warp results require SSC; convert the file format explicitly before applying this timing conversion",
            ));
        }
        let chosen: BTreeSet<_> = rows.iter().copied().collect();
        let mut events: Vec<_> = timing
            .editor_events()
            .iter()
            .copied()
            .filter(|event| {
                !chosen.contains(&(event.row as u64)) || !matches_direction(event.kind, direction)
            })
            .collect();
        for &row in &rows {
            match direction {
                Direction::WarpsToNegativeStops => {
                    warps.remove(row)?;
                }
                Direction::NegativeStopsToWarps => {
                    stops.remove(row)?;
                }
            }
        }
        let mut report = Report::default();
        let mut previous_end = None;
        for row in rows {
            let start = *times
                .get(&row)
                .ok_or_else(|| invalid("Selected timing event has no original rate mapping"))?;
            let length = eligible[&row];
            let collision = match direction {
                Direction::WarpsToNegativeStops => stops.values().contains_key(&row),
                Direction::NegativeStopsToWarps => warps.values().contains_key(&row),
            };
            if collision {
                report.deleted_on_collision.insert(row);
                continue;
            }
            if previous_end.is_some_and(|end| end > row) {
                report.overlaps.insert(row);
            }
            let current = timeline(&GenerationTiming::new(events.clone())?)?;
            let (kind, end) = match direction {
                Direction::WarpsToNegativeStops => {
                    let end = row
                        .checked_add(length as u64)
                        .ok_or_else(|| invalid("Warp end exceeds row range"))?;
                    let seconds = -(current.position_time(end)? - start);
                    if seconds == 0.0 || crate::number::reference_equal(seconds, 0.0) {
                        return Err(invalid(
                            "Conversion would create a zero-length Stop that cannot survive save/reopen; the original Warp and history were preserved",
                        ));
                    }
                    stops.set(row, seconds)?;
                    (EventKind::Stop(seconds), end)
                }
                Direction::NegativeStopsToWarps => {
                    let end = current.time_position(start - length)?.round_ties_even();
                    if !end.is_finite() || end > i32::MAX as f64 {
                        return Err(invalid("Converted Warp end exceeds editor row range"));
                    }
                    let length = ((end - row as f64).max(0.0) as u64).max(1);
                    warps.set(row, length)?;
                    (EventKind::Warp(length as i64), end.max(0.0) as u64)
                }
            };
            events.push(Event {
                row: row as i64,
                kind,
            });
            previous_end = Some(previous_end.unwrap_or(0).max(end));
            report.converted += 1;
            report.selection_after.push(row);
        }
        Ok(Self {
            target,
            chart,
            stops: stops.serialize(),
            warps: warps.serialize(),
            report,
        })
    }
    pub fn chart(&self) -> usize { self.chart }
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn apply(
        &self,
        session: &mut Session,
        acknowledged_collisions: &BTreeSet<u64>,
    ) -> Result<bool, Error> {
        if acknowledged_collisions != &self.report.deleted_on_collision {
            return Err(invalid(
                "Review every timing original deleted by an opposite-kind collision before applying",
            ));
        }
        let document = session.document();
        let origin = self.target.origin();
        let mut changes = Vec::new();
        // Typed equality avoids canonicalizing untouched formatting on no-op.
        if Pauses::parse(
            PauseKind::Stop,
            &document.text(origin, "STOPS").unwrap_or_default(),
        )?
        .serialize()
            != self.stops
        {
            changes.push(("STOPS", self.stops.clone()));
        }
        if Regions::parse(
            RegionKind::Warp,
            &document.text(origin, "WARPS").unwrap_or_default(),
        )?
        .serialize()
            != self.warps
        {
            changes.push(("WARPS", self.warps.clone()));
        }
        self.target.apply(
            session,
            self.chart,
            &changes,
            "Convert Warps and negative Stops",
        )
    }
}
fn matches_direction(kind: EventKind, direction: Direction) -> bool {
    match direction {
        Direction::WarpsToNegativeStops => matches!(kind, EventKind::Warp(_)),
        Direction::NegativeStopsToWarps => matches!(kind,EventKind::Stop(value)if value<0.0),
    }
}
fn timeline(timing: &GenerationTiming) -> Result<EditorTimeline, Error> {
    EditorTimeline::build(timing, &Grid::new(4, 1)?, None, &AtomicBool::new(false))
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
