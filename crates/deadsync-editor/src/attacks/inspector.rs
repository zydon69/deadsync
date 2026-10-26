//! Resident row-bound attack editing. File-time normalization happens only on Apply.
use super::{Attacks, Draft as FileDraft, Event as FileEvent, Modifier, placement::Timeline};
use crate::{Error, document::Document, history::Session, playback::Clock, timing_edit::Scope};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Resident chart position, independent of the raw attack TIME property.
    pub row: u64,
    /// Editable start in chart seconds minus the effective music offset.
    pub time: f64,
    /// Editor event time after timing phases, including same-row attacks.
    pub chart_time: f64,
    pub modifiers: Vec<Modifier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Events {
    events: Vec<Event>,
}
impl Events {
    pub fn events(&self) -> &[Event] {
        &self.events
    }
}
pub struct Draft {
    file: FileDraft,
    snapshot: Session,
    chart: usize,
    offset: f64,
    clock: Clock,
    current: Events,
    original: Events,
    sources: BTreeMap<usize, u64>,
    hints: BTreeMap<u64, Vec<f64>>,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let file = FileDraft::capture(session, chart, scope)?;
        let placed = Timeline::capture_scope(session, chart, scope)?;
        let offset =
            crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let events: Vec<_> = placed
            .markers()
            .iter()
            .map(|marker| Event {
                row: marker.row,
                time: marker.chart_time - offset,
                chart_time: marker.chart_time,
                modifiers: marker.modifiers.clone(),
            })
            .collect();
        let sources = placed
            .markers()
            .iter()
            .flat_map(|marker| {
                marker
                    .source_indices
                    .iter()
                    .map(|index| (*index, marker.row))
            })
            .collect();
        let hints = placed
            .markers()
            .iter()
            .map(|marker| {
                (
                    marker.row,
                    marker
                        .source_indices
                        .iter()
                        .map(|&index| file.attacks().events()[index].time)
                        .collect(),
                )
            })
            .collect();
        let rows: Vec<_> = events.iter().map(|event| event.row).collect();
        let snapshot = Session::new(Document::parse(
            session.serialize()?,
            session.document().format,
        )?);
        let clock = Clock::capture_with_attacks(&snapshot, chart, &rows)?;
        let current = Events { events };
        Ok(Self {
            file,
            snapshot,
            chart,
            offset,
            clock,
            original: current.clone(),
            current,
            sources,
            hints,
        })
    }
    pub fn scope(&self) -> Scope {
        self.file.scope()
    }
    pub fn inherited(&self) -> bool {
        self.file.inherited()
    }
    pub fn attacks(&self) -> &Events {
        &self.current
    }
    pub fn has_changes(&self) -> bool {
        self.current != self.original
    }
    pub fn event_for_source(&self, index: usize) -> Option<usize> {
        let row = *self.sources.get(&index)?;
        self.current
            .events
            .binary_search_by_key(&row, |event| event.row)
            .ok()
    }
    fn entry(&self, event: usize) -> Result<&Event, Error> {
        self.current
            .events
            .get(event)
            .ok_or_else(|| invalid("Attack does not exist"))
    }
    fn row_at(&self, time: f64) -> Result<u64, Error> {
        let row = self.clock.position(self.clock.chart_time_to_music(time)?)?;
        if !row.is_finite() || row.max(0.0).round() > i32::MAX as f64 {
            return Err(invalid("Attack cursor exceeds editor rows"));
        }
        Ok(row.max(0.0).round() as u64)
    }
    fn replace(&mut self, mut events: Vec<Event>) -> Result<(), Error> {
        events.sort_by_key(|event| event.row);
        if events.iter().any(|event| event.row > i32::MAX as u64)
            || events.windows(2).any(|pair| pair[0].row == pair[1].row)
        {
            return Err(invalid("Attack rows collide or exceed editor limits"));
        }
        let rows: Vec<_> = events.iter().map(|event| event.row).collect();
        let clock = Clock::capture_with_attacks(&self.snapshot, self.chart, &rows)?;
        for (event, (row, time)) in events.iter_mut().zip(clock.attack_times()) {
            debug_assert_eq!(event.row, row);
            event.chart_time = time;
            event.time = time - self.offset;
            if !event.time.is_finite() {
                return Err(invalid("Nonfinite normalized attack time"));
            }
        }
        self.current = Events { events };
        self.clock = clock;
        Ok(())
    }
    pub fn set(
        &mut self,
        event: usize,
        modifier: usize,
        time: f64,
        value: Modifier,
    ) -> Result<usize, Error> {
        if !time.is_finite() {
            return Err(invalid("Attack time must be finite"));
        }
        value.validate()?;
        let original = self.entry(event)?;
        if modifier >= original.modifiers.len() {
            return Err(invalid("Attack modifier does not exist"));
        }
        let row = if original.time == time {
            original.row
        } else {
            let time = time + self.offset;
            if !time.is_finite() {
                return Err(invalid("Attack time overflow"));
            }
            self.row_at(time.max(0.0))?
        };
        self.set_at_row(event, modifier, row, value)
    }
    pub fn set_at_row(
        &mut self,
        event: usize,
        modifier: usize,
        row: u64,
        value: Modifier,
    ) -> Result<usize, Error> {
        value.validate()?;
        let old = self.entry(event)?.row;
        if modifier >= self.current.events[event].modifiers.len() {
            return Err(invalid("Attack modifier does not exist"));
        }
        if row == old {
            self.current.events[event].modifiers[modifier] = value;
            return Ok(event);
        }
        let mut events = self.current.events.clone();
        events[event].row = row;
        events[event].modifiers[modifier] = value;
        self.replace(events)?;
        for source in self.sources.values_mut() {
            if *source == old {
                *source = row;
            }
        }
        Ok(self
            .current
            .events
            .binary_search_by_key(&row, |event| event.row)
            .expect("Moved attack is present"))
    }
    pub fn add(&mut self, row: u64, value: Modifier) -> Result<usize, Error> {
        value.validate()?;
        if self.current.events.iter().any(|event| event.row == row) {
            return Err(invalid("An attack already occupies the cursor row"));
        }
        let mut events = self.current.events.clone();
        events.push(Event {
            row,
            time: 0.0,
            chart_time: 0.0,
            modifiers: vec![value],
        });
        self.replace(events)?;
        Ok(self
            .current
            .events
            .binary_search_by_key(&row, |event| event.row)
            .expect("Added attack is present"))
    }
    pub fn add_modifier(&mut self, event: usize, value: Modifier) -> Result<usize, Error> {
        value.validate()?;
        self.entry(event)?;
        let index = self.current.events[event].modifiers.len();
        self.current.events[event].modifiers.push(value);
        Ok(index)
    }
    pub fn remove_modifier(&mut self, event: usize, modifier: usize) -> Result<(), Error> {
        let entry = self.entry(event)?;
        if modifier >= entry.modifiers.len() {
            return Err(invalid("Attack modifier does not exist"));
        }
        if entry.modifiers.len() == 1 {
            return self.remove(event);
        }
        self.current.events[event].modifiers.remove(modifier);
        Ok(())
    }
    pub fn remove(&mut self, event: usize) -> Result<(), Error> {
        let row = self.entry(event)?.row;
        let mut events = self.current.events.clone();
        events.remove(event);
        self.replace(events)?;
        self.sources.retain(|_, source| *source != row);
        Ok(())
    }
    pub fn start_row(&self, event: usize) -> Result<u64, Error> {
        Ok(self.entry(event)?.row)
    }
    pub fn end_row(&self, event: usize, modifier: usize) -> Result<u64, Error> {
        let event = self.entry(event)?;
        let modifier = event
            .modifiers
            .get(modifier)
            .ok_or_else(|| invalid("Attack modifier does not exist"))?;
        let time = event.chart_time + modifier.length;
        if !time.is_finite() || modifier.length < 0.0 {
            return Err(invalid("Invalid attack endpoint"));
        }
        self.row_at(time)
    }
    pub fn length_to_cursor(&self, event: usize, row: u64) -> Result<f64, Error> {
        let start = self.entry(event)?.chart_time;
        Ok((self.clock.chart_time(row)?.max(0.0) - start).max(0.0))
    }
    pub fn default_length(&self, row: u64) -> Result<f64, Error> {
        let rate = self
            .clock
            .rate_mappings()
            .filter(|(_, rate)| rate.row() <= row as f64)
            .last()
            .map(|(_, rate)| rate)
            .ok_or_else(|| invalid("No tempo available for an attack duration"))?;
        let length = rate.seconds_per_row() * crate::notes::ROWS_PER_MEASURE as f64;
        if !length.is_finite() {
            return Err(invalid("Attack duration overflow"));
        }
        Ok(length)
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.file.token) {
            return Err(invalid("Attack inspector draft is stale"));
        }
        if !self.has_changes() {
            return Ok(false);
        }
        let desired: Vec<_> = self
            .current
            .events
            .iter()
            .map(|event| {
                (
                    event.row,
                    event.time,
                    self.hints.get(&event.row).cloned().unwrap_or_default(),
                )
            })
            .collect();
        let times = super::placement::encode_times(&self.snapshot, chart, &desired)?;
        let current = Attacks {
            events: self
                .current
                .events
                .iter()
                .zip(times)
                .map(|(event, time)| FileEvent {
                    time,
                    modifiers: event.modifiers.clone(),
                })
                .collect(),
        };
        let bytes = current.serialize_preserving_empty()?;
        // A row hidden in a warp may share a file time with another position.
        // Require a faithful reload before accepting a file-format transaction.
        let mut preview = Session::new(self.snapshot.document().clone());
        preview.set_property_bytes(
            if self.scope() == Scope::Chart {
                Some(chart)
            } else {
                None
            },
            "ATTACKS",
            bytes.clone().into(),
        )?;
        let placed = Timeline::capture_scope(&preview, chart, self.scope())?;
        if placed.markers().len() != self.current.events.len()
            || placed
                .markers()
                .iter()
                .zip(&self.current.events)
                .any(|(marker, event)| {
                    marker.row != event.row || marker.modifiers != event.modifiers
                })
        {
            return Err(invalid(
                "Attack rows cannot round-trip through this file timing",
            ));
        }
        if self.scope() == Scope::Chart
            && self.current.events.is_empty()
            && self.file.song_has_attacks
        {
            return Err(invalid(
                "An empty chart attack list would restore song attacks",
            ));
        }
        session.set_property_bytes(
            if self.scope() == Scope::Chart {
                Some(chart)
            } else {
                None
            },
            "ATTACKS",
            bytes.into(),
        )
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
