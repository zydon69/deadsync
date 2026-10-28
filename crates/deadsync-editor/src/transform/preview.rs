//! One sparse transform engine shared by range commands and exact-head review.
//! Spatial actions use ForceInsert. Ownership actions use strict insertion and
//! never acquire spatial replacement semantics.
use super::{Transform, shift, symmetry_lane};
use crate::{
    Error,
    history::Session,
    notes::{Grid, Note, Position},
    pad::Pad,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    OwnerCollision,
    Invalid,
}
pub(crate) struct Failure {
    pub kind: Refusal,
    pub error: Error,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            kind: Refusal::Invalid,
            error,
        }
    }
}
pub(crate) struct Preview {
    pub grid: Grid,
    /// Source head -> intended destination and expected transformed value. None
    /// means source boundary deletion; final collision effects are read from Grid.
    pub mappings: BTreeMap<Position, Option<(Position, Note)>>,
}
pub(crate) fn positions(
    session: &mut Session,
    chart: usize,
    heads: &BTreeSet<Position>,
    transform: Transform,
) -> Result<Preview, Failure> {
    let pad = if matches!(
        transform,
        Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip
    ) {
        let layout = &session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| invalid("Chart does not exist"))?
            .steps_type;
        Some(Pad::builtin(layout)?)
    } else {
        None
    };
    let grid = session.grid(chart)?;
    if matches!(
        transform,
        Transform::AssignPlayer(_) | Transform::SwapPlayers(_, _)
    ) {
        return owners(grid, heads, transform);
    }
    if matches!(transform, Transform::Convert { .. }) {
        return Err(invalid("Kind conversion uses the shared note operation model").into());
    }
    let mut selected: Vec<_> = grid
        .iter()
        .filter(|(p, _)| {
            heads.contains(p)
                && pad.is_none_or(|pad| symmetry_lane(pad, p.lane, transform).is_some())
        })
        .map(|(p, n)| (p, n.clone()))
        .collect();
    selected.sort_by_key(|(p, _)| (p.row, p.lane, p.player));
    let mut staged = grid.clone();
    for (p, _) in &selected {
        staged.set(*p, None)?;
    }
    let mut mappings = BTreeMap::new();
    for (source, mut note) in selected {
        let mut target = source;
        match transform {
            Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip => {
                target.lane = symmetry_lane(
                    pad.ok_or_else(|| invalid("Missing symmetry pad"))?,
                    target.lane,
                    transform,
                )
                .ok_or_else(|| invalid("Selected lane has no symmetry mapping"))?;
            }
            Transform::ShiftLanes { right, wrap } => {
                let lane = if right {
                    target
                        .lane
                        .checked_add(1)
                        .filter(|lane| *lane < staged.columns())
                } else {
                    target.lane.checked_sub(1)
                };
                target.lane = match lane {
                    Some(lane) => lane,
                    None if wrap => {
                        if right {
                            0
                        } else {
                            staged.columns() - 1
                        }
                    }
                    None => {
                        mappings.insert(source, None);
                        continue;
                    }
                };
            }
            Transform::ShiftRows(rows) => {
                if rows < 0 && target.row < rows.unsigned_abs() {
                    mappings.insert(source, None);
                    continue;
                }
                target.row = shift(target.row, rows)?;
                note.end = note.end.map(|end| shift(end, rows)).transpose()?;
            }
            _ => return Err(invalid("Unsupported spatial transform").into()),
        }
        mappings.insert(source, Some((target, note.clone())));
        staged.force_insert(target, note)?;
    }
    Ok(Preview {
        grid: staged,
        mappings,
    })
}
fn owners(
    grid: &Grid,
    heads: &BTreeSet<Position>,
    transform: Transform,
) -> Result<Preview, Failure> {
    if let Transform::SwapPlayers(a, b) = transform {
        if a >= grid.players() || b >= grid.players() {
            return Err(invalid("Player swap targets must exist in this chart").into());
        }
    }
    let selected: Vec<_> = grid
        .iter()
        .filter(|(p, _)| {
            heads.contains(p)
                && match transform {
                    Transform::AssignPlayer(owner) => p.player != owner,
                    Transform::SwapPlayers(a, b) => a != b && (p.player == a || p.player == b),
                    _ => false,
                }
        })
        .map(|(p, n)| (p, n.clone()))
        .collect();
    let sources: BTreeSet<_> = selected.iter().map(|(p, _)| *p).collect();
    let mut targets = BTreeSet::new();
    let mut mappings = BTreeMap::new();
    let mut changes = Vec::new();
    for (source, note) in selected {
        let mut target = source;
        match transform {
            Transform::AssignPlayer(owner) => target.player = owner,
            Transform::SwapPlayers(a, b) => {
                if target.player == a {
                    target.player = b;
                } else if target.player == b {
                    target.player = a;
                }
            }
            _ => return Err(invalid("Unsupported ownership transform").into()),
        }
        if target.player >= grid.players() {
            return Err(invalid("Lane or participant lies outside this chart").into());
        }
        if !targets.insert(target) || (grid.get(target).is_some() && !sources.contains(&target)) {
            return Err(Failure {
                kind: Refusal::OwnerCollision,
                error: invalid("Transformation would overwrite an unselected note"),
            });
        }
        mappings.insert(source, Some((target, note.clone())));
        changes.push((target, note));
    }
    let mut staged = grid.clone();
    for source in sources {
        staged.set(source, None)?;
    }
    for (target, note) in changes {
        staged.set(target, Some(note)).map_err(|error| Failure {
            kind: Refusal::OwnerCollision,
            error,
        })?;
    }
    Ok(Preview {
        grid: staged,
        mappings,
    })
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
