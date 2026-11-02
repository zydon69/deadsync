//! Simultaneous note-kind swaps from GrooveAuthor UIEditEvents.
//!
//! Original actions create zero-length sustains when swapping a simple note into
//! a hold. Fumen drops such charts on reload. This API rejects that result as one
//! transaction; callers can use Convert with an explicit duration instead.
use crate::{
    Error,
    history::Session,
    notes::{Kind, Position},
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pair {
    TapsMines,
    TapsFakes,
    TapsLifts,
    MinesFakes,
    MinesLifts,
    HoldsRolls,
    HoldsTaps,
    HoldsMines,
    RollsTaps,
    RollsMines,
}
impl Pair {
    pub fn kinds(self) -> (Kind, Kind) {
        use Kind::*;
        match self {
            Self::TapsMines => (Tap, Mine),
            Self::TapsFakes => (Tap, Fake),
            Self::TapsLifts => (Tap, Lift),
            Self::MinesFakes => (Mine, Fake),
            Self::MinesLifts => (Mine, Lift),
            Self::HoldsRolls => (Hold, Roll),
            Self::HoldsTaps => (Hold, Tap),
            Self::HoldsMines => (Hold, Mine),
            Self::RollsTaps => (Roll, Tap),
            Self::RollsMines => (Roll, Mine),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    ZeroDurationSustain,
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
    fn from(failure: Failure) -> Self {
        failure.error
    }
}
/// Swaps only selected heads, preserving the existing native decoration policy.
/// No force-insertion, invented duration, or intermediate converted state.
pub fn apply(
    session: &mut Session,
    chart: usize,
    selected: &BTreeSet<Position>,
    pair: Pair,
) -> Result<bool, Error> {
    apply_detailed(session, chart, selected, pair).map_err(Into::into)
}
pub fn apply_detailed(
    session: &mut Session,
    chart: usize,
    selected: &BTreeSet<Position>,
    pair: Pair,
) -> Result<bool, Failure> {
    let failure = |error: crate::note_operation::Failure| Failure {
        kind: match error.kind() {
            crate::note_operation::Refusal::ZeroDurationSustain => Refusal::ZeroDurationSustain,
            _ => Refusal::Invalid,
        },
        error: error.into(),
    };
    let plan = crate::note_operation::Plan::prepare_bounded(
        session,
        chart,
        crate::note_operation::Scope::Heads(selected.clone()),
        crate::note_operation::Operation::Swap(pair),
        usize::MAX,
        &AtomicBool::new(false),
    )
    .map_err(failure)?;
    plan.apply_named(session, chart, &format!("Swap note types: {pair:?}"))
        .map_err(failure)
}
