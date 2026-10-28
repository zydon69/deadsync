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
use std::collections::{BTreeMap, BTreeSet};

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
pub enum Refusal { ZeroDurationSustain, Invalid }
#[derive(Debug)]
pub struct Failure { kind: Refusal, error: Error }
impl Failure { pub fn kind(&self) -> Refusal { self.kind } }
impl From<Error> for Failure { fn from(error: Error) -> Self { Self { kind: Refusal::Invalid, error } } }
impl From<Failure> for Error { fn from(failure: Failure) -> Self { failure.error } }
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
pub fn apply_detailed(session: &mut Session, chart: usize, selected: &BTreeSet<Position>, pair: Pair) -> Result<bool, Failure> {
    let (a, b) = pair.kinds();
    let grid = session.grid(chart)?;
    let mut edits = BTreeMap::new();
    for &position in selected {
        let Some(original) = grid.get(position) else {
            continue;
        };
        let destination = if original.kind == a {
            b
        } else if original.kind == b {
            a
        } else {
            continue;
        };
        let mut note = original.clone();
        if matches!(destination, Kind::Hold | Kind::Roll) && note.end.is_none() {
            return Err(Failure {kind: Refusal::ZeroDurationSustain, error: Error::InvalidEdit("Swap would create a zero-length hold or roll. Use note conversion with an explicit sustain duration.".into())});
        }
        note = crate::note_operation::convert_note(&note, position.row, destination, 0, grid.uses_stepf2())?;
        edits.insert(position, Some(note));
    }
    session.edit_notes(chart, format!("Swap note types: {pair:?}"), edits).map_err(Into::into)
}
