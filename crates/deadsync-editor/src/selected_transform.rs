//! Exact-head spatial and owner transforms with a detached effects review.
use crate::{
    Error,
    history::{Session, file_intent::IntentGuard},
    note_operation::Scope,
    notes::{Note, Position},
    transform::{Transform, preview},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactKind {
    SelectedDeleted,
    SelectedChanged,
    OtherDeleted,
    OtherChanged,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Impact {
    pub position: Position,
    pub kind: ImpactKind,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub selected: usize,
    pub processed: usize,
    pub moved: usize,
    pub deleted: usize,
    pub other_affected: usize,
    pub selection_after: Vec<Position>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    OwnerCollision,
    MissingReview,
    Stale,
    Invalid,
}
#[derive(Debug)]
pub struct Failure {
    kind: Refusal,
    error: Error,
}
impl Failure {
    pub fn kind(&self) -> Refusal {
        self.kind
    }
    pub(crate) fn stale() -> Self {
        Self::new(
            Refusal::Stale,
            "Transform selection belongs to another chart, session or history state",
        )
    }
    fn new(kind: Refusal, message: &str) -> Self {
        Self {
            kind,
            error: invalid(message),
        }
    }
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            kind: Refusal::Invalid,
            error,
        }
    }
}
impl From<Failure> for Error {
    fn from(value: Failure) -> Self {
        value.error
    }
}
pub struct Plan {
    guard: IntentGuard,
    chart: usize,
    identity: crate::generation::metadata::Guid,
    report: Report,
    impacts: BTreeSet<Impact>,
    edits: BTreeMap<Position, Option<Note>>,
    selection: Vec<(Position, Option<Position>)>,
    owner_selection_order: bool,
}
impl Plan {
    pub fn prepare(
        session: &mut Session,
        chart: usize,
        scope: Scope,
        transform: Transform,
    ) -> Result<Self, Failure> {
        Self::prepare_bounded(
            session,
            chart,
            scope,
            transform,
            2_000_000,
            &AtomicBool::new(false),
        )
    }
    /// Grid cloning and the reference transform are synchronous. Cancellation is
    /// cooperative at bounded sparse scan/diff boundaries, not an allocator abort.
    pub fn prepare_bounded(
        session: &mut Session,
        chart: usize,
        scope: Scope,
        transform: Transform,
        maximum_notes: usize,
        cancelled: &AtomicBool,
    ) -> Result<Self, Failure> {
        check(cancelled)?;
        if matches!(transform, Transform::Convert { .. }) {
            return Err(invalid("Kind conversion uses the note operation review").into());
        }
        if let Scope::Range { start, end, .. } = &scope {
            if start > end || *end > i32::MAX as u64 {
                return Err(invalid("Invalid transformation row range").into());
            }
        }
        let identity = session
            .chart_ids()
            .get(chart)
            .cloned()
            .ok_or_else(|| invalid("No chart to transform"))?;
        let guard = IntentGuard::capture(session);
        let grid = session.grid(chart)?;
        let mut heads = BTreeSet::new();
        let owner = match &scope {
            Scope::All { player } | Scope::Range { player, .. } => *player,
            Scope::Heads(_) => None,
        };
        if owner.is_some_and(|owner| owner >= grid.players()) {
            return Err(invalid("Selected source player does not exist").into());
        }
        for (index, (position, _)) in grid.iter().enumerate() {
            if index >= maximum_notes {
                return Err(invalid("Transformation note scan limit exceeded").into());
            }
            if index % 1024 == 0 {
                check(cancelled)?;
            }
            if match &scope {
                Scope::All { player } => player.is_none_or(|p| p == position.player),
                Scope::Range { start, end, player } => {
                    (*start..=*end).contains(&position.row)
                        && player.is_none_or(|p| p == position.player)
                }
                Scope::Heads(heads) => heads.contains(&position),
            } {
                heads.insert(position);
            }
        }
        check(cancelled)?;
        let prepared =
            preview::positions(session, chart, &heads, transform).map_err(|failure| Failure {
                kind: match failure.kind {
                    preview::Refusal::OwnerCollision => Refusal::OwnerCollision,
                    preview::Refusal::Invalid => Refusal::Invalid,
                },
                error: failure.error,
            })?;
        check(cancelled)?;
        let original = session.grid(chart)?;
        let mut report = Report {
            selected: heads.len(),
            processed: prepared.mappings.len(),
            ..Default::default()
        };
        let mut impacts = BTreeSet::new();
        for (&source, destination) in &prepared.mappings {
            match destination {
                None => {
                    report.deleted += 1;
                    impacts.insert(Impact {
                        position: source,
                        kind: ImpactKind::SelectedDeleted,
                    });
                }
                Some((target, expected)) => {
                    if source != *target {
                        report.moved += 1;
                    }
                    match prepared.grid.get(*target) {
                        Some(actual) if actual == expected => report.selection_after.push(*target),
                        Some(_) => {
                            report.selection_after.push(*target);
                            impacts.insert(Impact {
                                position: source,
                                kind: ImpactKind::SelectedChanged,
                            });
                        }
                        None => {
                            report.deleted += 1;
                            impacts.insert(Impact {
                                position: source,
                                kind: ImpactKind::SelectedDeleted,
                            });
                        }
                    }
                }
            }
        }
        let positions: BTreeSet<_> = original
            .iter()
            .map(|(p, _)| p)
            .chain(prepared.grid.iter().map(|(p, _)| p))
            .collect();
        let mut edits = BTreeMap::new();
        for (index, position) in positions.into_iter().enumerate() {
            if index % 1024 == 0 {
                check(cancelled)?;
            }
            let before = original.get(position);
            let after = prepared.grid.get(position);
            if before != after {
                edits.insert(position, after.cloned());
                if before.is_some() && !prepared.mappings.contains_key(&position) {
                    report.other_affected += 1;
                    impacts.insert(Impact {
                        position,
                        kind: if after.is_none() {
                            ImpactKind::OtherDeleted
                        } else {
                            ImpactKind::OtherChanged
                        },
                    });
                }
            }
        }
        report.selection_after.sort_by_key(|position| {
            (
                position.row,
                prepared
                    .grid
                    .get(*position)
                    .expect("selected destination exists")
                    .kind
                    .editor_phase(),
                position.lane,
                position.player,
            )
        });
        report.selection_after.dedup();
        let mut selection: Vec<_> = prepared
            .mappings
            .iter()
            .map(|(&source, destination)| {
                (
                    source,
                    destination
                        .as_ref()
                        .and_then(|(target, _)| prepared.grid.get(*target).map(|_| *target)),
                )
            })
            .collect();
        selection.sort_by_key(|(position, _)| {
            (
                position.row,
                original
                    .get(*position)
                    .expect("transformed source exists")
                    .kind
                    .editor_phase(),
                position.lane,
                position.player,
            )
        });
        check(cancelled)?;
        Ok(Self {
            guard,
            chart,
            identity,
            report,
            impacts,
            edits,
            selection,
            owner_selection_order: matches!(
                transform,
                Transform::AssignPlayer(_) | Transform::SwapPlayers(_, _)
            ),
        })
    }
    pub(crate) fn with_selection_order(mut self, order: &[Position]) -> Self {
        // Lane transformations explicitly sort their source and result lists.
        // Player actions retain the selected-event enumeration instead.
        if self.owner_selection_order {
            let mappings: BTreeMap<_, _> = self.selection.into_iter().collect();
            self.selection = order
                .iter()
                .filter_map(|source| mappings.get(source).map(|target| (*source, *target)))
                .collect();
        }
        self
    }
    pub fn chart(&self) -> usize {
        self.chart
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn impacts(&self) -> &BTreeSet<Impact> {
        &self.impacts
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.chart_ids().get(chart) == Some(&self.identity)
            && self.guard.is_current(session)
    }
    pub fn apply(
        &self,
        session: &mut Session,
        chart: usize,
        acknowledged: &BTreeSet<Impact>,
    ) -> Result<bool, Failure> {
        self.apply_named(
            session,
            chart,
            acknowledged,
            "Apply reviewed note transform",
        )
    }
    pub(crate) fn apply_named(
        &self,
        session: &mut Session,
        chart: usize,
        acknowledged: &BTreeSet<Impact>,
        label: &str,
    ) -> Result<bool, Failure> {
        if !self.is_current(session, chart) {
            return Err(Failure::stale());
        }
        if acknowledged != &self.impacts {
            return Err(Failure::new(
                Refusal::MissingReview,
                "Acknowledge every deleted, overwritten or trimmed original note before applying",
            ));
        }
        let before = crate::history::SelectionEffect::capture(
            session,
            chart,
            self.selection
                .iter()
                .map(|(source, _)| crate::event_clipboard::Event::Note(*source))
                .collect(),
        );
        let changed = session.edit_notes(chart, label, self.edits.clone())?;
        let after = if self.owner_selection_order {
            self.selection
                .iter()
                .filter_map(|(_, target)| target.map(crate::event_clipboard::Event::Note))
                .collect()
        } else {
            self.report
                .selection_after
                .iter()
                .copied()
                .map(crate::event_clipboard::Event::Note)
                .collect()
        };
        let after = crate::history::SelectionEffect::capture(session, chart, after);
        session.record_event_selection(
            crate::history::selection_effect::SelectionHistory { before, after },
            changed,
        );
        Ok(changed)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Transformation preparation cancelled"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        event_clipboard::{Event, Selection},
    };

    fn fixture() -> Session {
        Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;".to_vec(), Format::Ssc).unwrap())
    }
    fn pos(row: u64, lane: u16) -> Position {
        Position {
            row,
            lane,
            player: 0,
        }
    }
    fn events(positions: &[Position]) -> Vec<Event> {
        positions.iter().copied().map(Event::Note).collect()
    }

    #[test]
    fn reviewed_mirror_selects_sorted_destinations_and_restores_originals_on_undo() {
        let mut session = fixture();
        let mut selection = Selection::capture(&mut session, 0, 0, 192).unwrap();
        selection.select_all(false);
        for position in [pos(48, 1), pos(0, 0)] {
            let index = selection
                .events()
                .iter()
                .position(|event| *event == Event::Note(position))
                .unwrap();
            selection.set_selected(index, true).unwrap();
        }
        let before = session.serialize().unwrap();
        let plan = selection
            .prepare_transform(&mut session, Transform::Mirror)
            .unwrap();
        assert!(session.take_event_selection_update().is_none());
        assert!(plan.apply(&mut session, 0, plan.impacts()).unwrap());
        let after = session.serialize().unwrap();
        let update = session.take_event_selection_update().unwrap();
        assert!(update.is_current(&session));
        assert_eq!(update.events(), events(&[pos(0, 3), pos(48, 1)]));
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            events(&[pos(0, 0), pos(48, 1)])
        );
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            events(&[pos(0, 3), pos(48, 1)])
        );
    }

    #[test]
    fn boundary_deletion_selects_only_survivors_but_undo_selects_all_transformed_originals() {
        let mut session = fixture();
        let before = session.serialize().unwrap();
        let plan = Plan::prepare(
            &mut session,
            0,
            Scope::All { player: None },
            Transform::ShiftLanes {
                right: false,
                wrap: false,
            },
        )
        .unwrap();
        assert_eq!(plan.report().deleted, 1);
        assert!(plan.apply(&mut session, 0, plan.impacts()).unwrap());
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            events(&[pos(48, 0)])
        );
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            events(&[pos(0, 0), pos(48, 1)])
        );
    }

    #[test]
    fn owner_conversion_preserves_checked_order_and_excludes_notes_already_owned_by_target() {
        let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n01000000\n00000000\n00000000;".to_vec(), Format::Ssc).unwrap());
        let owned = Position {
            row: 96,
            lane: 2,
            player: 1,
        };
        session
            .edit_notes(0, "fixture", BTreeMap::from([(owned, Some(Note::tap()))]))
            .unwrap();
        let mut selection = Selection::capture(&mut session, 0, 0, 192).unwrap();
        selection.select_all(false);
        for position in [pos(48, 1), owned, pos(0, 0)] {
            let index = selection
                .events()
                .iter()
                .position(|event| *event == Event::Note(position))
                .unwrap();
            selection.set_selected(index, true).unwrap();
        }
        let before = session.serialize().unwrap();
        let plan = selection
            .prepare_transform(&mut session, Transform::AssignPlayer(1))
            .unwrap();
        assert_eq!(plan.report().processed, 2);
        assert!(plan.apply(&mut session, 0, plan.impacts()).unwrap());
        let expected = events(&[
            Position {
                player: 1,
                ..pos(48, 1)
            },
            Position {
                player: 1,
                ..pos(0, 0)
            },
        ]);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            expected
        );
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            events(&[pos(48, 1), pos(0, 0)])
        );
        session.redo().unwrap();
        assert_eq!(
            session.take_event_selection_update().unwrap().events(),
            expected
        );
    }
}
