pub mod presets;
use super::{Attacks, Clipboard, Timing, scope};
use crate::{
    Error,
    history::{RevisionToken, Session},
    notes::{Kind, Position},
};
use std::collections::{BTreeMap, BTreeSet};

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
}
impl Event {
    pub fn row(self) -> u64 {
        match self {
            Self::Note(p) => p.row,
            Self::Timing { row, .. } | Self::Attack { row } | Self::Pattern { row } => row,
        }
    }
    fn order(self, note_kinds: &BTreeMap<Position, Kind>) -> u8 {
        match self {
            Self::Note(position) => match note_kinds.get(&position) {
                Some(Kind::Tap | Kind::Fake | Kind::Lift) => 8,
                Some(Kind::Hold | Kind::Roll) => 10,
                _ => 12,
            },
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
}
/// Snapshot-bound choices; detached clipboard values are created only after
/// validating the source revision. Choosing items never changes the document.
pub struct Selection {
    token: RevisionToken,
    chart: usize,
    events: Vec<Event>,
    selected: BTreeSet<Event>,
    note_kinds: BTreeMap<Position, Kind>,
}
impl Selection {
    pub fn capture(
        session: &mut Session,
        chart: usize,
        start: u64,
        end: u64,
    ) -> Result<Self, Error> {
        if start > end || end > i32::MAX as u64 {
            return Err(Error::InvalidEdit("Invalid event selection bounds".into()));
        }
        let mut events: Vec<_> = Timing::capture(session, chart, scope(session), start, end)?
            .keys()
            .collect();
        let attacks = Attacks::capture(session, chart, scope(session))?;
        events.extend(
            attacks
                .attacks()
                .events()
                .iter()
                .filter(|e| (start..=end).contains(&e.row))
                .map(|e| Event::Attack { row: e.row }),
        );
        let note_kinds: BTreeMap<_, _> = session
            .grid(chart)?
            .iter()
            .filter(|(p, _)| (start..=end).contains(&p.row))
            .map(|(p, n)| (p, n.kind))
            .collect();
        events.extend(note_kinds.keys().copied().map(Event::Note));
        if let Some(data) = crate::generation::metadata::ChartData::read(session.document(), chart)? {
            events.extend(data.patterns.into_iter().flatten()
                .filter(|(row, _)| (start..=end).contains(row))
                .map(|(row, _)| Event::Pattern { row }));
        }
        // Keep a total presentation order. Fumen compares lanes before event
        // type only when both events are lane notes, which can be nontransitive
        // around a Pattern. Never feed that comparator to Rust's sort.
        events.sort_by_key(|e| {
            let (lane, player) = match e { Event::Note(p) => (p.lane, p.player), _ => (0, 0) };
            (e.row(), e.order(&note_kinds), lane, player, *e)
        });
        let selected = events.iter().copied().collect();
        Ok(Self {
            token: session.revision_token(),
            chart,
            events,
            selected,
            note_kinds,
        })
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
    pub fn select_all(&mut self, selected: bool) {
        self.selected = if selected {
            self.events.iter().copied().collect()
        } else {
            BTreeSet::new()
        };
    }
    pub fn set_selected(&mut self, index: usize, selected: bool) -> Result<(), Error> {
        let event = *self
            .events
            .get(index)
            .ok_or_else(|| Error::InvalidEdit("Selected event does not exist".into()))?;
        if selected {
            self.selected.insert(event);
        } else {
            self.selected.remove(&event);
        }
        Ok(())
    }
    fn validate(&self, session: &Session) -> Result<(), Error> {
        if session.accepts_token(&self.token) {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Event selection belongs to another session or revision".into(),
            ))
        }
    }
    pub fn prepare_note_operation(&self, session: &mut Session, operation: crate::note_operation::Operation) -> Result<crate::note_operation::Plan, crate::note_operation::Failure> {
        self.validate(session)?;
        let heads = self.selected.iter().filter_map(|event| match event { Event::Note(position) => Some(*position), _ => None }).collect();
        crate::note_operation::Plan::prepare(session, self.chart, crate::note_operation::Scope::Heads(heads), operation)
    }
    pub fn prepare_timing_conversion(&self, session: &Session, direction: crate::timing_edit::conversion::Direction) -> Result<crate::timing_edit::conversion::Plan, crate::timing_edit::conversion::Failure> {
        use crate::timing_edit::conversion::{Direction, Plan, Failure};
        self.validate(session).map_err(|_| Failure::stale())?;
        let rows: Vec<_> = self.selected.iter().filter_map(|event| match (direction, event) {
            (Direction::WarpsToNegativeStops, Event::Timing { row, kind: TimingKind::Warp })
            | (Direction::NegativeStopsToWarps, Event::Timing { row, kind: TimingKind::Stop }) => Some(*row),
            _ => None,
        }).collect();
        Plan::prepare_detailed(session, self.chart, direction, Some(&rows))
    }
    pub fn prepare_transform(&self, session: &mut Session, transform: crate::transform::Transform) -> Result<crate::selected_transform::Plan, crate::selected_transform::Failure> {
        self.validate(session).map_err(|_| crate::selected_transform::Failure::stale())?;
        let heads = self.selected.iter().filter_map(|event| match event { Event::Note(position) => Some(*position), _ => None }).collect();
        crate::selected_transform::Plan::prepare(session, self.chart, crate::note_operation::Scope::Heads(heads), transform)
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
