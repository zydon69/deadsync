//! File-loader attack placement (Fumen 0fe292f9, SMCommon.SetAttackRowsAndTimes).
//! Retain source indices for inspection; rendering groups coincident loader rows.
use super::{Attacks, Draft, Modifier};
use crate::{
    Error,
    generation::timing::{EventKind, GenerationTiming},
    history::Session,
    notes::{Grid, Kind, layout_columns},
    playback::Clock,
    timing_edit::Scope,
};
#[derive(Clone, Debug)]
pub struct Marker {
    pub row: u64,
    pub loaded_time: f64,
    pub chart_time: f64,
    pub source_indices: Vec<usize>,
    pub modifiers: Vec<Modifier>,
    /// Overview duration is the maximum modifier duration; details remain in the inspector.
    pub end_time: f64,
    pub end_row: f64,
}
pub struct Timeline {
    markers: Vec<Marker>,
    regions: crate::region_index::RegionIndex,
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
        let draft = Draft::capture(session, chart, scope)?;
        let attacks = draft.attacks().events();
        let origin = if scope == Scope::Chart && !draft.inherited() {
            Some(chart)
        } else {
            None
        };
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
            });
        }
        let document = session.document();
        let descriptor = &document.charts()[chart];
        let columns = layout_columns(&descriptor.steps_type)
            .ok_or_else(|| Error::Unsupported("Unknown attack chart layout".into()))?;
        let parsed;
        let grid = if let Some(grid) = session.loaded_grid(chart) {
            grid
        } else {
            parsed = Grid::parse(document.value(descriptor.notes.clone()), columns)?;
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
            let row =
                i64::try_from(position.row).map_err(|_| error("Attack source row overflow"))?;
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
        if input.is_empty() {
            return Ok(Self {
                markers: Vec::new(),
                regions: Default::default(),
            });
        }
        let offset =
            crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let clock = Clock::capture(session, chart)?;
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
        let mut warp = 0i64;
        let mut stop = 0.0;
        let mut seconds_per_row = 1.0;
        let mut rows_per_second = 1.0;
        let mut has_tempo = false;
        // The pinned reference keeps previousEvent at the first source event.
        let first_row = input[0].row;
        for (index, event) in input.iter().enumerate() {
            let rows = event.row - first_row;
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
            while next < attacks.len()
                && (index + 1 == input.len() || input[index + 1].time >= raw_times[next])
            {
                let (row, loaded_time) = if has_tempo {
                    let relative = raw_times[next] - (event.time + stop);
                    let value = event.row as f64 + relative * rows_per_second + warp as f64;
                    let rounded = (value + 0.5).trunc();
                    if !rounded.is_finite() || rounded > i32::MAX as f64 {
                        return Err(error("Attack position exceeds editor rows"));
                    }
                    let row = rounded.max(0.0) as u64;
                    let time = event.time
                        + (((row as f64 - event.row as f64).max(0.0)) * seconds_per_row + stop)
                            .max(0.0);
                    (row, time)
                } else {
                    (0, raw_times[next])
                };
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
        // Refresh all event times in the editor pass, including note heads and
        // the unique attack phase. A rate-query shortcut can differ by one ULP
        // after crossing a warp; the original helper visits these events too.
        let mut editor: Vec<(i64, u8, u16, Option<EventKind>, Option<usize>)> = timing
            .editor_events()
            .iter()
            .map(|event| (event.row, event.order(), 0, Some(event.kind), None))
            .collect();
        editor.extend(
            input
                .iter()
                .filter(|event| event.kind.is_none() && event.order != 11)
                .map(|event| (event.row, event.order, event.lane, None, None)),
        );
        editor.extend(
            grouped
                .iter()
                .enumerate()
                .map(|(index, marker)| (marker.row as i64, 6, 0, None, Some(index))),
        );
        editor.sort_by_key(|event| (event.0, event.1, event.2));
        let mut editor_clock = crate::generation::timing::Clock::default();
        for (row, _, _, kind, marker) in editor {
            let time = editor_clock.editor_event(row, kind).time;
            if !time.is_finite() {
                return Err(error("Nonfinite editor attack time"));
            }
            if let Some(index) = marker {
                grouped[index].chart_time = time;
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
