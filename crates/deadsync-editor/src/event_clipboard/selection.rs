mod order;
pub mod presets;
pub mod region;
use super::{
    Attacks, Clipboard, Timing, scope,
    song_widget::{self, SongWidget},
};
use crate::{
    Error,
    history::{ContentToken, Session},
    notes::{Kind, Position},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimingKind {
    Signature,
    Tempo,
    Tick,
    Fake,
    Combo,
    Label,
    Delay,
    Scroll,
    Speed,
    Stop,
    Warp,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    Note(Position),
    Timing { row: u64, kind: TimingKind },
    Attack { row: u64 },
    Pattern { row: u64 },
    SongWidget(SongWidget),
}
impl Event {
    pub fn row(self) -> u64 {
        match self {
            Self::Note(p) => p.row,
            Self::SongWidget(widget) => widget.source_row().max(0) as u64,
            Self::Timing { row, .. } | Self::Attack { row } | Self::Pattern { row } => row,
        }
    }
    /// Exact chart position; use this for presentation and region geometry.
    /// `row()` is the legacy nonnegative integral projection and must never be
    /// used to move, delete, or copy a song widget.
    pub fn chart_position(self) -> f64 {
        match self {
            Self::SongWidget(widget) => widget.position(),
            _ => self.row() as f64,
        }
    }
    pub fn content_row(self) -> Option<u64> {
        (!matches!(self, Self::SongWidget(_))).then(|| self.row())
    }
    pub fn source_row(self) -> i64 {
        match self {
            Self::SongWidget(widget) => widget.source_row(),
            _ => self.row() as i64,
        }
    }
    fn order(self, note_kinds: &BTreeMap<Position, Kind>) -> u8 {
        match self {
            Self::Note(position) => match note_kinds.get(&position) {
                Some(Kind::Tap | Kind::Fake | Kind::Lift) => 8,
                Some(Kind::Hold | Kind::Roll) => 10,
                _ => 12,
            },
            Self::SongWidget(_) => 0,
            Self::Attack { .. } => 6,
            Self::Pattern { .. } => 9,
            Self::Timing { kind, .. } => match kind {
                TimingKind::Signature => 0,
                TimingKind::Tempo => 1,
                TimingKind::Tick => 2,
                TimingKind::Fake => 3,
                TimingKind::Combo => 4,
                TimingKind::Label => 5,
                TimingKind::Delay => 7,
                TimingKind::Scroll => 13,
                TimingKind::Speed => 14,
                TimingKind::Stop => 15,
                TimingKind::Warp => 17,
            },
        }
    }
    pub(super) fn sort_key(
        self,
        note_kinds: &BTreeMap<Position, Kind>,
    ) -> (u64, u8, u16, u32, Self) {
        let (lane, player) = match self {
            Self::Note(position) => (position.lane, position.player),
            _ => (0, 0),
        };
        (self.row(), self.order(note_kinds), lane, player, self)
    }
}
/// Snapshot-bound choices; detached clipboard values are created only after
/// validating the source revision. Choosing items never changes the document.
#[derive(Clone)]
pub struct Selection {
    token: ContentToken,
    chart: usize,
    events: Arc<Vec<Event>>,
    selected: BTreeSet<Event>,
    order: order::Order,
    last_selected: Option<Event>,
    note_kinds: Arc<BTreeMap<Position, Kind>>,
}
impl Selection {
    pub fn capture(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
    ) -> Result<Self, Error> {
        Self::capture_with_policy(session, chart, start, end, true, true)
    }
    pub(crate) fn capture_inventory(
        session: &mut Session,
        chart: usize,
        notes: bool,
    ) -> Result<Self, Error> {
        Self::capture_with_policy(session, chart, 0, i32::MAX as u64, notes, false)
    }
    fn capture_with_policy(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
        include_notes: bool,
        initially_selected: bool,
    ) -> Result<Self, Error> {
        if start > end || end > i32::MAX as u64 {
            return Err(Error::InvalidEdit("Invalid event selection bounds".into()));
        }
        let mut events: Vec<_> =
            Timing::capture_inventory(session, chart, scope(session), start, end)?
                .keys()
                .collect();
        let attacks = Attacks::capture(session, chart, super::attack_scope(session))?;
        events.extend(
            attacks
                .attacks()
                .events()
                .iter()
                .filter(|e| (start..=end).contains(&e.row))
                .map(|e| Event::Attack { row: e.row }),
        );
        let note_kinds: BTreeMap<_, _> = if include_notes {
            session
                .grid(chart)?
                .iter()
                .filter(|(p, _)| (start..=end).contains(&p.row))
                .map(|(p, n)| (p, n.kind))
                .collect()
        } else {
            BTreeMap::new()
        };
        events.extend(note_kinds.keys().copied().map(Event::Note));
        if let Some(data) = crate::generation::metadata::ChartData::read(session.document(), chart)?
        {
            events.extend(
                data.patterns
                    .into_iter()
                    .flatten()
                    .filter(|(row, _)| (start..=end).contains(row))
                    .map(|(row, _)| Event::Pattern { row }),
            );
        }
        // Keep a total presentation order. Fumen compares lanes before event
        // type only when both events are lane notes, which can be nontransitive
        // around a Pattern. Never feed that comparator to Rust's sort.
        events.sort_by_key(|event| event.sort_key(&note_kinds));
        let selected = if initially_selected {
            events.iter().copied().collect()
        } else {
            BTreeSet::new()
        };
        let mut order = order::Order::default();
        if initially_selected {
            for event in &events {
                order.add(*event);
            }
        }
        let last_selected = if initially_selected {
            events.last().copied()
        } else {
            None
        };
        Ok(Self {
            token: session.content_token(),
            chart,
            events: Arc::new(events),
            selected,
            order,
            note_kinds: Arc::new(note_kinds),
            last_selected,
        })
    }
    /// Opt into the two song-level widgets for spatial selection. Existing
    /// bounded content operations keep their established payload inventory.
    pub fn include_song_widgets(&mut self, session: &Session) -> Result<(), Error> {
        self.validate(session)?;
        for widget in song_widget::capture(session, self.chart)? {
            let event = Event::SongWidget(widget);
            if !self.events.contains(&event) {
                Arc::make_mut(&mut self.events).push(event);
            }
        }
        Arc::make_mut(&mut self.events).sort_by(|a, b| {
            a.chart_position()
                .total_cmp(&b.chart_position())
                .then_with(|| {
                    let rank = |e: &Event| match e {
                        Event::SongWidget(w) => match w.kind() {
                            song_widget::WidgetKind::Preview => 0,
                            song_widget::WidgetKind::EndHint => 1,
                        },
                        _ => 2,
                    };
                    rank(a).cmp(&rank(b))
                })
                .then(a.order(&self.note_kinds).cmp(&b.order(&self.note_kinds)))
                .then_with(|| {
                    let lane_player = |e: &Event| match e {
                        Event::Note(p) => (p.lane, p.player),
                        _ => (0, 0),
                    };
                    lane_player(a).cmp(&lane_player(b))
                })
                .then(a.cmp(b))
        });
        Ok(())
    }
    pub fn chart(&self) -> usize {
        self.chart
    }
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn note_kind(&self, position: Position) -> Option<Kind> {
        self.note_kinds.get(&position).copied()
    }
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }
    pub fn is_selected(&self, index: usize) -> bool {
        self.events
            .get(index)
            .is_some_and(|e| self.selected.contains(e))
    }
    /// Exact selected enumeration, distinct from sorted presentation choices.
    pub fn selected_events(&self) -> impl Iterator<Item = Event> + '_ {
        self.order.events()
    }
    /// Explicit source selection anchor, distinct from HashSet enumeration.
    /// Re-selecting a checked event moves the anchor; removing it clears it.
    pub fn last_selected_event(&self) -> Option<Event> {
        self.last_selected
    }
    pub fn select_all(&mut self, selected: bool) {
        self.selected.clear();
        self.order.clear();
        self.last_selected = None;
        if selected {
            for event in self.events.iter() {
                self.selected.insert(*event);
                self.order.add(*event);
                self.last_selected = Some(*event);
            }
        }
    }
    fn choose(&mut self, event: Event, selected: bool) {
        if selected {
            self.last_selected = Some(event);
            if self.selected.insert(event) {
                self.order.add(event);
            }
        } else if self.selected.remove(&event) {
            self.order.remove(event);
            if self.last_selected == Some(event) {
                self.last_selected = None;
            }
        }
    }
    pub fn set_selected(&mut self, index: usize, selected: bool) -> Result<(), Error> {
        let event = *self
            .events
            .get(index)
            .ok_or_else(|| Error::InvalidEdit("Selected event does not exist".into()))?;
        self.choose(event, selected);
        Ok(())
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_content_token(&self.token)
    }
    pub(crate) fn validate(&self, session: &Session) -> Result<(), Error> {
        if session.accepts_content_token(&self.token) {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Event selection belongs to another session or revision".into(),
            ))
        }
    }
    pub fn prepare_note_operation(
        &self,
        session: &mut Session,
        operation: crate::note_operation::Operation,
    ) -> Result<crate::note_operation::Plan, crate::note_operation::Failure> {
        self.validate(session)?;
        let heads: Vec<_> = self
            .selected_events()
            .filter_map(|event| match event {
                Event::Note(position) => Some(position),
                _ => None,
            })
            .collect();
        crate::note_operation::Plan::prepare(
            session,
            self.chart,
            crate::note_operation::Scope::Heads(heads.iter().copied().collect()),
            operation,
        )
        .map(|plan| plan.with_selection_order(&heads))
    }
    pub fn prepare_timing_conversion(
        &self,
        session: &Session,
        direction: crate::timing_edit::conversion::Direction,
    ) -> Result<crate::timing_edit::conversion::Plan, crate::timing_edit::conversion::Failure> {
        use crate::timing_edit::conversion::{Direction, Failure, Plan};
        self.validate(session).map_err(|_| Failure::stale())?;
        let rows: Vec<_> = self
            .selected_events()
            .filter_map(|event| match (direction, event) {
                (
                    Direction::WarpsToNegativeStops,
                    Event::Timing {
                        row,
                        kind: TimingKind::Warp,
                    },
                )
                | (
                    Direction::NegativeStopsToWarps,
                    Event::Timing {
                        row,
                        kind: TimingKind::Stop,
                    },
                ) => Some(row),
                _ => None,
            })
            .collect();
        Plan::prepare_detailed(session, self.chart, direction, Some(&rows))
    }
    pub fn prepare_transform(
        &self,
        session: &mut Session,
        transform: crate::transform::Transform,
    ) -> Result<crate::selected_transform::Plan, crate::selected_transform::Failure> {
        self.validate(session)
            .map_err(|_| crate::selected_transform::Failure::stale())?;
        let heads: Vec<_> = self
            .selected_events()
            .filter_map(|event| match event {
                Event::Note(position) => Some(position),
                _ => None,
            })
            .collect();
        crate::selected_transform::Plan::prepare(
            session,
            self.chart,
            crate::note_operation::Scope::Heads(heads.iter().copied().collect()),
            transform,
        )
        .map(|plan| plan.with_selection_order(&heads))
    }
    pub fn copy(&self, session: &mut Session) -> Result<Option<Clipboard>, Error> {
        self.validate(session)?;
        Clipboard::copy_selected(session, self.chart, &self.selected)
    }
    pub fn cut(&self, session: &mut Session) -> Result<Option<Clipboard>, Error> {
        let Some(clipboard) = self.copy(session)? else {
            return Ok(None);
        };
        clipboard.remove_selected(session, self.chart, &self.selected, "Cut selected events")?;
        Ok(Some(clipboard))
    }
    pub fn delete(&self, session: &mut Session) -> Result<bool, Error> {
        let Some(clipboard) = self.copy(session)? else {
            return Ok(false);
        };
        clipboard.remove_selected(
            session,
            self.chart,
            &self.selected,
            "Delete selected events",
        )
    }
}
