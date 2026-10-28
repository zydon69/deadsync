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

/// Swaps only selected heads, preserving the existing native decoration policy.
/// No force-insertion, invented duration, or intermediate converted state.
pub fn apply(
    session: &mut Session,
    chart: usize,
    selected: &BTreeSet<Position>,
    pair: Pair,
) -> Result<bool, Error> {
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
            return Err(Error::InvalidEdit("Swap would create a zero-length hold or roll. Use note conversion with an explicit sustain duration.".into()));
        }
        if grid.uses_stepf2() {
            crate::stepf2::change_kind(&mut note, destination)?;
        } else {
            note.kind = destination;
        }
        if !matches!(destination, Kind::Hold | Kind::Roll) {
            note.end = None;
            note.tail_attributes = "".into();
        }
        edits.insert(position, Some(note));
    }
    session.edit_notes(chart, format!("Swap note types: {pair:?}"), edits)
}
