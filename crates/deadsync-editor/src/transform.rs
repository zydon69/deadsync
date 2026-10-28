//! Selection transforms preserve owners, sustain tails, and note attributes.
//! Refer to GrooveAuthor EditorActions/Action*Selection*.cs (MIT).
pub(crate) mod preview;
use crate::{
    Error,
    history::Session,
    notes::{Kind, Position},
    pad::Pad,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Mirror,
    Flip,
    MirrorAndFlip,
    ShiftRows(i64),
    ShiftLanes {
        right: bool,
        wrap: bool,
    },
    AssignPlayer(u32),
    SwapPlayers(u32, u32),
    Convert {
        from: Kind,
        to: Kind,
        sustain_rows: u64,
    },
}

pub fn apply(session: &mut Session, chart: usize, start: u64, end: u64, player: Option<u32>, transform: Transform) -> Result<bool, Error> {
    if start > end { return Err(Error::InvalidEdit("Selection ends before it starts".into())); }
    let heads = session.grid(chart)?.iter().filter(|(position, _)| (start..=end).contains(&position.row) && player.is_none_or(|p| p == position.player)).map(|(position, _)| position).collect();
    apply_positions(session, chart, &heads, transform)
}
/// Same native reference engine for exact checked heads. Spatial ForceInsert and
/// strict ownership collision contracts remain separate inside the preview.
pub fn apply_positions(session: &mut Session, chart: usize, heads: &BTreeSet<Position>, transform: Transform) -> Result<bool, Error> {
    if let Transform::Convert { from, to, sustain_rows } = transform {
        let grid = session.grid(chart)?;
        let mut edits = BTreeMap::new();
        for (position, note) in grid.iter().filter(|(position, note)| heads.contains(position) && note.kind == from) {
            edits.insert(position, Some(crate::note_operation::convert_note(note, position.row, to, sustain_rows, grid.uses_stepf2())?));
        }
        return session.edit_notes(chart, format!("{transform:?}"), edits);
    }
    let prepared = preview::positions(session, chart, heads, transform).map_err(|failure| failure.error)?;
    let label = match transform {
        Transform::ShiftRows(_) => "Shift note rows".into(),
        Transform::ShiftLanes { .. } => "Shift note columns".into(),
        Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip => "Transform note lanes".into(),
        _ => format!("{transform:?}"),
    };
    session.apply_grid_delta(chart, &prepared.grid, &label)
}

fn symmetry_lane(pad: &Pad, lane: u16, transform: Transform) -> Option<u16> {
    let lane = if matches!(transform, Transform::Mirror | Transform::MirrorAndFlip) {
        pad.mirror[usize::from(lane)]?
    } else {
        lane
    };
    if matches!(transform, Transform::Flip | Transform::MirrorAndFlip) {
        pad.flip[usize::from(lane)]
    } else {
        Some(lane)
    }
}

fn shift(row: u64, delta: i64) -> Result<u64, Error> {
    if delta >= 0 {
        row.checked_add(delta as u64)
    } else {
        row.checked_sub(delta.unsigned_abs())
    }
    .ok_or_else(|| Error::InvalidEdit("Shift exceeds the valid row range".into()))
}
