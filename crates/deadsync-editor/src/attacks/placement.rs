//! File-loader attack placement (Fumen 0fe292f9, SMCommon.SetAttackRowsAndTimes).
//! Retain source indices for inspection; rendering groups coincident loader rows.
use super::{Attacks, Draft, Modifier};
use crate::{
    Error,
    generation::timing::{EventKind, GenerationTiming},
    history::Session,
    notes::{Grid, Kind},
    playback::Clock,
    timing_edit::Scope,
};
#[derive(Clone, Debug)]
pub struct Marker {
    pub row: u64,
    /// File-loader snapped time; for resident-only positions, their editor time.
    pub loaded_time: f64,
    pub chart_time: f64,
    /// Raw record indices, or resident event indices when row metadata is active.
    pub source_indices: Vec<usize>,
    pub modifiers: Vec<Modifier>,
    /// Overview duration is the maximum modifier duration; details remain in the inspector.
    pub end_time: f64,
    pub end_row: f64,
}
pub struct Timeline {
    markers: Vec<Marker>,
    regions: crate::region_index::RegionIndex,
    resident: bool,
}
#[derive(Clone, Copy)]
struct Input {
    row: i64,
    order: u8,
    lane: u16,
    kind: Option<EventKind>,
    time: f64,
}
impl Timeline {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let scope = if crate::timing_edit::supports_chart_timing(session.document()) {
            Scope::Chart
        } else {
            Scope::Song
        };
        Self::capture_scope(session, chart, scope)
    }
    pub fn capture_scope(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        Self::capture_internal(session, chart, scope, true)
    }
    pub(crate) fn capture_file_scope(
        session: &Session,
        chart: usize,
        scope: Scope,
    ) -> Result<Self, Error> {
        Self::capture_internal(session, chart, scope, false)
    }
    pub fn has_resident_rows(&self) -> bool {
        self.resident
    }
    fn capture_internal(
        session: &Session,
        chart: usize,
        scope: Scope,
        use_resident: bool,
    ) -> Result<Self, Error> {
        let draft = Draft::capture(session, chart, scope)?;
        let attacks = draft.attacks().events();
        let origin = if scope == Scope::Chart && !draft.inherited() {
            Some(chart)
        } else {
            None
        };
        if use_resident {
            let raw = session
                .document()
                .raw_property(origin, "ATTACKS")
                .unwrap_or_default();
            if let Some(events) = super::resident::read(session, chart, origin.is_some(), raw)? {
                let markers = events
                    .into_iter()
                    .enumerate()
                    .map(|(index, event)| Marker {
                        row: event.row,
                        loaded_time: 0.0,
                        chart_time: 0.0,
                        source_indices: vec![index],
                        modifiers: event.modifiers,
                        end_time: 0.0,
                        end_row: event.row as f64,
                    })
                    .collect();
                return Self::from_grouped(session, chart, markers, true);
            }
        }
        let source_order = Attacks::parse_source_order(
            session
                .document()
                .raw_property(origin, "ATTACKS")
                .unwrap_or_default(),
        )?;
        let source_indices: Vec<usize> = source_order
            .events()
            .iter()
            .map(|source| {
                attacks
                    .binary_search_by(|attack| attack.time.total_cmp(&source.time))
                    .expect("Both views parse the same attack records")
            })
            .collect();
        if attacks.is_empty() {
            return Ok(Self {
                markers: Vec::new(),
                regions: Default::default(),
                resident: false,
            });
        }
        let input = file_inputs(session, chart)?;
        if input.is_empty() {
            return Ok(Self {
                markers: Vec::new(),
                regions: Default::default(),
                resident: false,
            });
        }
        let offset =
            crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let mut raw_times = Vec::with_capacity(attacks.len());
        for attack in source_order.events() {
            let time = attack.time + offset;
            if !time.is_finite() {
                return Err(error("Nonfinite attack placement"));
            }
            raw_times.push(time.max(0.0));
        }
        let mut markers = Vec::with_capacity(attacks.len());
        let mut next = 0;
        let states = file_states(&input);
        for (index, state) in states.iter().enumerate() {
            while next < attacks.len()
                && (index + 1 == input.len() || input[index + 1].time >= raw_times[next])
            {
                let (row, loaded_time) = state.place(raw_times[next])?;
                let source_index = source_indices[next];
                markers.push(Marker {
                    row,
                    loaded_time,
                    chart_time: 0.0,
                    source_indices: vec![source_index],
                    modifiers: attacks[source_index].modifiers.clone(),
                    end_time: 0.0,
                    end_row: row as f64,
                });
                next += 1;
            }
            if next == attacks.len() {
                break;
            }
        }
        markers.sort_by_key(|marker| marker.row);
        let mut grouped: Vec<Marker> = Vec::new();
        for marker in markers {
            if let Some(previous) = grouped
                .last_mut()
                .filter(|previous| previous.row == marker.row)
            {
                previous.source_indices.extend(marker.source_indices);
                previous.modifiers.extend(marker.modifiers);
            } else {
                grouped.push(marker);
            }
        }
        Self::from_grouped(session, chart, grouped, false)
    }
    fn from_grouped(
        session: &Session,
        chart: usize,
        mut grouped: Vec<Marker>,
        resident: bool,
    ) -> Result<Self, Error> {
        let rows: Vec<_> = grouped.iter().map(|marker| marker.row).collect();
        let clock = Clock::capture_with_attacks(session, chart, &rows)?;
        for (marker, (row, time)) in grouped.iter_mut().zip(clock.attack_times()) {
            debug_assert_eq!(marker.row, row);
            marker.chart_time = time;
            if resident {
                marker.loaded_time = time;
            }
        }
        for marker in &mut grouped {
            let duration = marker
                .modifiers
                .iter()
                .map(|modifier| modifier.length.max(0.0))
                .fold(0.0, f64::max);
            marker.end_time = marker.chart_time + duration;
            if !marker.end_time.is_finite() {
                return Err(error("Attack duration overflow"));
            }
            marker.end_row = if clock.rate_mappings().next().is_some() {
                clock
                    .position(clock.chart_time_to_music(marker.end_time)?)?
                    .max(marker.row as f64)
            } else {
                marker.row as f64
            };
            if !marker.end_row.is_finite() || marker.end_row > i32::MAX as f64 {
                return Err(error("Attack duration exceeds editor rows"));
            }
        }
        let regions = crate::region_index::RegionIndex::new(
            grouped
                .iter()
                .map(|marker| (marker.row, marker.end_row.ceil() as u64))
                .collect(),
        )?;
        Ok(Self {
            markers: grouped,
            regions,
            resident,
        })
    }
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }
    pub fn last_row(&self) -> u64 {
        self.regions.last_row()
    }
    pub fn at(&self, row: u64) -> Option<&Marker> {
        self.markers
            .binary_search_by_key(&row, |marker| marker.row)
            .ok()
            .map(|index| &self.markers[index])
    }
    pub fn neighbor(&self, row: u64, forward: bool) -> Option<u64> {
        if self.markers.is_empty() {
            return None;
        }
        let index = if forward {
            self.markers.partition_point(|marker| marker.row <= row) % self.markers.len()
        } else {
            self.markers
                .partition_point(|marker| marker.row < row)
                .checked_sub(1)
                .unwrap_or(self.markers.len() - 1)
        };
        Some(self.markers[index].row)
    }
    pub fn visible(&self, start: u64, end: u64, limit: usize) -> Result<Vec<&Marker>, Error> {
        Ok(self
            .regions
            .visible(start, end, limit)?
            .into_iter()
            .filter_map(|(row, _)| self.at(row))
            .collect())
    }
}
fn error(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

fn file_inputs(session: &Session, chart: usize) -> Result<Vec<Input>, Error> {
    let document = session.document();
    let descriptor = &document.charts()[chart];
    let parsed;
    let grid = if let Some(grid) = session.loaded_grid(chart) {
        grid
    } else {
        parsed = Grid::parse_layout(document.value(descriptor.notes.clone()), &descriptor.steps_type)?;
        &parsed
    };
    let timing = GenerationTiming::from_document(document, chart)?;
    let mut input: Vec<Input> = timing
        .file_events()
        .iter()
        .map(|event| Input {
            row: event.row,
            order: event.order(),
            lane: 0,
            kind: Some(event.kind),
            time: 0.0,
        })
        .collect();
    for (position, note) in grid.iter() {
        let row = i64::try_from(position.row).map_err(|_| error("Attack source row overflow"))?;
        let order = match note.kind {
            Kind::Tap | Kind::Fake | Kind::Lift => 8,
            Kind::Hold | Kind::Roll => 10,
            _ => 12,
        };
        input.push(Input {
            row,
            order,
            lane: position.lane,
            kind: None,
            time: 0.0,
        });
        if let Some(end) = note.end {
            input.push(Input {
                row: i64::try_from(end).map_err(|_| error("Attack source tail overflow"))?,
                order: 11,
                lane: position.lane,
                kind: None,
                time: 0.0,
            });
        }
    }
    input.sort_by_key(|event| (event.row, event.order, event.lane));
    let mut file_clock = crate::generation::timing::Clock::default();
    for event in &mut input {
        event.time = file_clock.event(event.row, event.kind);
        if !event.time.is_finite() {
            return Err(error("Nonfinite attack source clock"));
        }
    }
    Ok(input)
}
#[derive(Clone, Copy)]
struct FileState {
    event: Input,
    warp: i64,
    stop: f64,
    seconds_per_row: f64,
    rows_per_second: f64,
    has_tempo: bool,
}
impl FileState {
    fn place(self, time: f64) -> Result<(u64, f64), Error> {
        if !self.has_tempo {
            return Ok((0, time));
        }
        let relative = time - (self.event.time + self.stop);
        let rounded =
            (self.event.row as f64 + relative * self.rows_per_second + self.warp as f64 + 0.5)
                .trunc();
        if !rounded.is_finite() || rounded > i32::MAX as f64 {
            return Err(error("Attack position exceeds editor rows"));
        }
        let row = rounded.max(0.0) as u64;
        let loaded = self.event.time
            + ((row as f64 - self.event.row as f64).max(0.0) * self.seconds_per_row + self.stop)
                .max(0.0);
        Ok((row, loaded))
    }
    fn inverse(self, row: u64, offset: f64) -> f64 {
        if !self.has_tempo {
            return -offset;
        }
        (row as f64 - self.event.row as f64 - self.warp as f64) * self.seconds_per_row
            + self.event.time
            + self.stop
            - offset
    }
}
fn file_states(input: &[Input]) -> Vec<FileState> {
    let Some(first) = input.first() else {
        return Vec::new();
    };
    let mut result = Vec::with_capacity(input.len());
    let mut warp = 0i64;
    let mut stop = 0.0;
    let mut seconds_per_row = 1.0;
    let mut rows_per_second = 1.0;
    let mut has_tempo = false;
    // Preserve the pinned loader's first-event tracking for stop/warp state.
    for &event in input {
        let rows = event.row - first.row;
        warp = (warp - rows).max(0);
        if stop != 0.0 {
            stop = (stop + rows as f64 * seconds_per_row).min(0.0);
        }
        match event.kind {
            Some(EventKind::Stop(length) | EventKind::Delay(length)) => stop += length,
            Some(EventKind::Warp(length)) => warp = warp.max(length),
            Some(EventKind::Tempo(bpm)) => {
                has_tempo = true;
                seconds_per_row = 60.0 / bpm / 48.0;
                rows_per_second = 1.0 / seconds_per_row;
            }
            _ => {}
        }
        result.push(FileState {
            event,
            warp,
            stop,
            seconds_per_row,
            rows_per_second,
            has_tempo,
        });
    }
    result
}
/// Encode resident rows using actual loader preimages, preserving source times
/// when possible. A chart-time subtraction alone is not an inverse near stops.
pub(super) fn encode_times(
    session: &Session,
    chart: usize,
    desired: &[(u64, f64, Vec<f64>)],
) -> Result<Vec<f64>, Error> {
    if desired.is_empty() {
        return Ok(Vec::new());
    }
    let input = file_inputs(session, chart)?;
    let states = file_states(&input);
    if states.is_empty() {
        return Err(error("No source events for attack placement"));
    }
    let offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
    let mut cursor = 0;
    let mut used = std::collections::BTreeSet::new();
    let mut output = Vec::with_capacity(desired.len());
    for (row, normalized, hints) in desired {
        let mut accepted = None;
        let mut try_time = |value: f64| {
            for candidate in [value, value.next_up(), value.next_down()] {
                let key = if candidate == 0.0 {
                    0
                } else {
                    candidate.to_bits()
                };
                let time = candidate + offset;
                if !candidate.is_finite() || !time.is_finite() || used.contains(&key) {
                    continue;
                }
                let time = time.max(0.0);
                // Negative stops can make source-event times nonmonotonic.
                // Match the loader's forward traversal from the current event.
                let index = (cursor..states.len().saturating_sub(1))
                    .find(|&index| states[index + 1].event.time >= time)
                    .unwrap_or(states.len() - 1);
                if states[index]
                    .place(time)
                    .is_ok_and(|(actual, _)| actual == *row)
                {
                    accepted = Some((candidate, index, key));
                    return true;
                }
            }
            false
        };
        let mut found = false;
        for &hint in hints.iter().chain(std::iter::once(normalized)) {
            if try_time(hint) {
                found = true;
                break;
            }
        }
        if !found {
            for state in &states[cursor..] {
                if try_time(state.inverse(*row, offset)) {
                    break;
                }
            }
        }
        let Some((time, index, key)) = accepted else {
            return Err(error("Attack row cannot be represented by this file clock"));
        };
        output.push(time);
        cursor = index;
        used.insert(key);
    }
    Ok(output)
}
