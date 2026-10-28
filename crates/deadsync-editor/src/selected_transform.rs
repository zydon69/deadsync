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
        report.selection_after.sort_unstable();
        report.selection_after.dedup();
        check(cancelled)?;
        Ok(Self {
            guard,
            chart,
            identity,
            report,
            impacts,
            edits,
        })
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
        if !self.is_current(session, chart) {
            return Err(Failure::stale());
        }
        if acknowledged != &self.impacts {
            return Err(Failure::new(
                Refusal::MissingReview,
                "Acknowledge every deleted, overwritten or trimmed original note before applying",
            ));
        }
        Ok(session.edit_notes(chart, "Apply reviewed note transform", self.edits.clone())?)
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
