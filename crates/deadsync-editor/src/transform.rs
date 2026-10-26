//! Selection transforms preserve owners, sustain tails, and note attributes.
//! Refer to GrooveAuthor EditorActions/Action*Selection*.cs (MIT).
use crate::{
    Error,
    history::Session,
    notes::{Kind, Note, Position},
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

pub fn apply(
    session: &mut Session,
    chart: usize,
    start: u64,
    end: u64,
    player: Option<u32>,
    transform: Transform,
) -> Result<bool, Error> {
    if start > end {
        return Err(Error::InvalidEdit("Selection ends before it starts".into()));
    }
    if matches!(
        transform,
        Transform::ShiftLanes { .. }
            | Transform::ShiftRows(_)
            | Transform::Mirror
            | Transform::Flip
            | Transform::MirrorAndFlip
    ) {
        let pad = if matches!(
            transform,
            Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip
        ) {
            let layout = &session
                .document()
                .charts()
                .get(chart)
                .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?
                .steps_type;
            Some(Pad::builtin(layout)?)
        } else {
            None
        };
        let original = session.grid(chart)?;
        let mut staged = original.clone();
        let mut selected: Vec<_> = original
            .iter()
            .filter(|(pos, _)| {
                pos.row >= start
                    && pos.row <= end
                    && player.is_none_or(|p| pos.player == p)
                    && pad.is_none_or(|pad| symmetry_lane(pad, pos.lane, transform).is_some())
            })
            .map(|(pos, note)| (pos, note.clone()))
            .collect();
        selected.sort_by_key(|(pos, _)| (pos.row, pos.lane, pos.player));
        for (pos, _) in &selected {
            staged.set(*pos, None)?;
        }
        for (mut pos, mut note) in selected {
            match transform {
                Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip => {
                    pos.lane = symmetry_lane(pad.unwrap(), pos.lane, transform).unwrap();
                }
                Transform::ShiftLanes { right, wrap } => {
                    let lane = if right {
                        pos.lane
                            .checked_add(1)
                            .filter(|lane| *lane < staged.columns())
                    } else {
                        pos.lane.checked_sub(1)
                    };
                    pos.lane = match lane {
                        Some(lane) => lane,
                        None if wrap => {
                            if right {
                                0
                            } else {
                                staged.columns() - 1
                            }
                        }
                        None => continue,
                    };
                }
                Transform::ShiftRows(delta) => {
                    if delta < 0 && pos.row < delta.unsigned_abs() {
                        continue;
                    }
                    pos.row = shift(pos.row, delta)?;
                    note.end = note.end.map(|end| shift(end, delta)).transpose()?;
                }
                _ => unreachable!("staged shift"),
            }
            staged.force_insert(pos, note)?;
        }
        let label = if matches!(transform, Transform::ShiftRows(_)) {
            "Shift note rows"
        } else if matches!(transform, Transform::ShiftLanes { .. }) {
            "Shift note columns"
        } else {
            "Transform note lanes"
        };
        return session.apply_grid_delta(chart, &staged, label);
    }
    let grid = session.grid(chart)?;
    if let Transform::SwapPlayers(a, b) = transform {
        if a >= grid.players() || b >= grid.players() {
            return Err(Error::InvalidEdit(
                "Player swap targets must exist in this chart".into(),
            ));
        }
    }
    let selected: Vec<_> = grid
        .iter()
        .filter(|(pos, _)| {
            pos.row >= start && pos.row <= end && player.is_none_or(|player| pos.player == player)
        })
        .map(|(pos, note)| (pos, note.clone()))
        .collect();
    let sources: BTreeSet<_> = selected.iter().map(|(pos, _)| *pos).collect();
    let mut edits: BTreeMap<Position, Option<Note>> =
        sources.iter().map(|&pos| (pos, None)).collect();
    let mut targets = BTreeSet::new();
    for (mut pos, mut note) in selected {
        match transform {
            Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip => {
                unreachable!("symmetry handled above")
            }
            Transform::ShiftLanes { .. } => unreachable!("lane shift handled above"),
            Transform::ShiftRows(_) => unreachable!("row shift handled above"),
            Transform::AssignPlayer(owner) => pos.player = owner,
            Transform::SwapPlayers(a, b) => {
                if pos.player == a {
                    pos.player = b;
                } else if pos.player == b {
                    pos.player = a;
                }
            }
            Transform::Convert {
                from,
                to,
                sustain_rows,
            } => {
                if note.kind == from {
                    if matches!(to, Kind::Keysound | Kind::Attack) {
                        return Err(Error::Unsupported(
                            "Conversion to an attributed note needs explicit attributes".into(),
                        ));
                    }
                    note.kind = to;
                    note.end = if matches!(to, Kind::Hold | Kind::Roll) {
                        Some(match note.end {
                            Some(end) => end,
                            None => pos.row.checked_add(sustain_rows).ok_or_else(|| {
                                Error::InvalidEdit("Sustain exceeds row range".into())
                            })?,
                        })
                    } else {
                        note.tail_attributes = "".into();
                        None
                    };
                }
            }
        }
        if !targets.insert(pos) || (grid.get(pos).is_some() && !sources.contains(&pos)) {
            return Err(Error::InvalidEdit(
                "Transformation would overwrite an unselected note".into(),
            ));
        }
        edits.insert(pos, Some(note));
    }
    session.edit_notes(chart, format!("{transform:?}"), edits)
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
