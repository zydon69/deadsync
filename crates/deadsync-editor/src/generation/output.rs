//! Validate performed notes before a generated chart can enter editor history.
use super::{
    expressed::InstanceType,
    graph::{FootAction, StepGraph},
    performed::Performance,
};
use crate::{
    Error,
    notes::{Grid, Kind, Note, Position},
};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct AssembledChart {
    pub grid: Grid,
    pub merged_mines: usize,
}
#[derive(Clone, Copy)]
enum Action {
    Release,
    Head(Kind),
}

pub fn assemble(
    performance: &Performance,
    cancelled: &AtomicBool,
) -> Result<AssembledChart, Error> {
    check(cancelled)?;
    let graph = StepGraph::builtin(&performance.steps.layout)?;
    let columns = u16::try_from(graph.columns())
        .map_err(|_| Error::InvalidEdit("Generated layout is too wide".into()))?;
    let mut grid = Grid::new(columns, 1)?;
    let mut holds = vec![None; usize::from(columns)];
    let mut previous_row = None;
    for step in &performance.steps.steps {
        check(cancelled)?;
        if step.row > i32::MAX as u64 || previous_row.is_some_and(|row| row > step.row) {
            return invalid("Generated steps are unordered or out of range");
        }
        let state = graph
            .nodes
            .get(step.node)
            .ok_or_else(|| Error::InvalidEdit("Unknown generated graph node".into()))?
            .state;
        let mut actions = vec![None; usize::from(columns)];
        for foot in 0..2 {
            for portion in 0..2 {
                let action = step.instance.link.feet[foot][portion];
                if !action.valid {
                    continue;
                }
                let lane = state.feet[foot][portion]
                    .lane
                    .ok_or_else(|| Error::InvalidEdit("Generated action has no lane".into()))?;
                let kind = step.instance.types[foot][portion];
                actions[usize::from(lane)] = Some(match action.action {
                    FootAction::Release => Action::Release,
                    FootAction::Hold => Action::Head(if kind == InstanceType::Roll {
                        Kind::Roll
                    } else {
                        Kind::Hold
                    }),
                    FootAction::Tap => Action::Head(match kind {
                        InstanceType::Fake => Kind::Fake,
                        InstanceType::Lift => Kind::Lift,
                        _ => Kind::Tap,
                    }),
                });
            }
        }
        for (lane, action) in actions.into_iter().enumerate() {
            let Some(action) = action else {
                continue;
            };
            match action {
                Action::Release => {
                    let (row, kind) = holds[lane].take().ok_or_else(|| {
                        Error::InvalidEdit("Generated hold tail has no head".into())
                    })?;
                    put(
                        &mut grid,
                        Position {
                            player: 0,
                            lane: lane as u16,
                            row,
                        },
                        Note {
                            kind,
                            end: Some(step.row),
                            ..Note::tap()
                        },
                    )?;
                }
                Action::Head(kind) => {
                    if holds[lane].is_some() {
                        return invalid("Generated note overlaps an active hold");
                    }
                    if matches!(kind, Kind::Hold | Kind::Roll) {
                        holds[lane] = Some((step.row, kind));
                    } else {
                        put(
                            &mut grid,
                            Position {
                                player: 0,
                                lane: lane as u16,
                                row: step.row,
                            },
                            Note {
                                kind,
                                ..Note::tap()
                            },
                        )?;
                    }
                }
            }
        }
        previous_row = Some(step.row);
    }
    if holds.iter().any(Option::is_some) {
        return invalid("Generated hold has no tail");
    }
    let mut merged_mines = 0;
    for mine in &performance.mines {
        check(cancelled)?;
        if mine.lane >= columns || mine.row > i32::MAX as u64 {
            return invalid("Generated mine lies outside the chart");
        }
        let position = Position {
            player: 0,
            lane: mine.lane,
            row: mine.row,
        };
        if grid
            .get(position)
            .is_some_and(|note| note.kind == Kind::Mine)
        {
            merged_mines += 1;
            continue;
        }
        put(
            &mut grid,
            position,
            Note {
                kind: Kind::Mine,
                ..Note::tap()
            },
        )?;
    }
    let bytes = grid.serialize()?;
    if Grid::parse(&bytes, columns)? != grid {
        return invalid("Generated notes do not round-trip");
    }
    check(cancelled)?;
    Ok(AssembledChart { grid, merged_mines })
}
fn put(grid: &mut Grid, position: Position, note: Note) -> Result<(), Error> {
    if grid.get(position).is_some() {
        return invalid("Generated notes collide in one cell");
    }
    grid.set(position, Some(note))?;
    Ok(())
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        invalid("Generated chart assembly cancelled")
    } else {
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
