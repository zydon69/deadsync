//! MineUtils release/step extraction and mine expression (MIT, bdbc427f).
//! The lists are shared by expressed-chart association and performed placement.
use super::{
    graph::{ArrowState, FootAction, GraphLink, NodeState, StepGraph},
    interpretation_path::PathNode,
};
use crate::Error;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionEvent {
    pub row: u64,
    pub lane: u16,
    pub foot: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct MovementNode {
    pub row: u64,
    pub state: NodeState,
    pub link: Option<GraphLink>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceMine {
    pub row: u64,
    pub time: f64,
    pub lane: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MineType {
    AfterArrow,
    BeforeArrow,
    NoArrow,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpressedMine {
    pub source: SourceMine,
    pub kind: MineType,
    pub nth: Option<usize>,
    pub foot: Option<usize>,
}

fn state(node: NodeState, columns: usize) -> Result<[Vec<Option<ArrowState>>; 2], Error> {
    let mut result = [vec![None; columns], vec![None; columns]];
    for (foot, portions) in node.feet.iter().enumerate() {
        for portion in portions {
            if let Some(lane) = portion.lane {
                let slot = result[foot].get_mut(usize::from(lane)).ok_or_else(|| {
                    Error::InvalidEdit("Mine context lane exceeds its layout".into())
                })?;
                *slot = Some(portion.state);
            }
        }
    }
    Ok(result)
}

pub fn actions(
    nodes: &[MovementNode],
    columns: usize,
    cancelled: &AtomicBool,
) -> Result<(Vec<ActionEvent>, Vec<ActionEvent>), Error> {
    check(cancelled)?;
    if columns == 0 || columns > u16::MAX as usize {
        return Err(Error::InvalidEdit("Invalid mine layout width".into()));
    }
    let (mut releases, mut steps) = (Vec::new(), Vec::new());
    let Some(first) = nodes.first() else {
        return Ok((releases, steps));
    };
    let mut previous = state(first.state, columns)?;
    let mut previous_row = first.row;
    for node in &nodes[1..] {
        check(cancelled)?;
        if node.row < previous_row {
            return Err(Error::InvalidEdit(
                "Mine movement nodes must be ordered".into(),
            ));
        }
        let current = state(node.state, columns)?;
        let link = node
            .link
            .ok_or_else(|| Error::InvalidEdit("Movement node has no link".into()))?;
        for foot in 0..2 {
            for lane in 0..columns {
                let before = previous[foot][lane];
                let now = current[foot][lane];
                let (step, release) = if matches!(now, None | Some(ArrowState::Resting))
                    && before == Some(ArrowState::Held)
                {
                    (false, true)
                } else if now == Some(ArrowState::Resting) && before.is_none() {
                    (true, true)
                } else if now == Some(ArrowState::Held)
                    && matches!(
                        before,
                        None | Some(ArrowState::Resting | ArrowState::Lifted)
                    )
                {
                    (true, false)
                } else if now == Some(ArrowState::Resting)
                    && matches!(before, Some(ArrowState::Resting | ArrowState::Lifted))
                {
                    let tap = link.feet[foot]
                        .iter()
                        .any(|action| action.valid && action.action == FootAction::Tap);
                    (tap, tap)
                } else {
                    (false, false)
                };
                let event = ActionEvent {
                    row: node.row,
                    lane: lane as u16,
                    foot,
                };
                if step {
                    steps.push(event);
                }
                if release {
                    releases.push(event);
                }
            }
        }
        previous = current;
        previous_row = node.row;
    }
    Ok((releases, steps))
}

pub fn path_nodes(
    graph: &StepGraph,
    last: &PathNode,
    cancelled: &AtomicBool,
) -> Result<Vec<MovementNode>, Error> {
    let mut nodes = Vec::new();
    let mut current = Some(last);
    while let Some(node) = current {
        check(cancelled)?;
        let state = graph
            .nodes
            .get(node.node)
            .ok_or_else(|| Error::InvalidEdit("Unknown mine context node".into()))?
            .state;
        nodes.push(MovementNode {
            row: node.row,
            state,
            link: node.instance.map(|i| i.link),
        });
        current = node.previous.as_deref();
    }
    nodes.reverse();
    Ok(nodes)
}

fn neighboring<'a>(
    events: impl Iterator<Item = &'a ActionEvent>,
    columns: usize,
    lane: u16,
    cancelled: &AtomicBool,
) -> Result<Option<(usize, usize)>, Error> {
    let mut seen = vec![false; columns];
    let mut previous_row = None;
    let mut nth = 0;
    for event in events {
        check(cancelled)?;
        if seen[usize::from(event.lane)] {
            continue;
        }
        if previous_row.is_some_and(|row| row != event.row) {
            nth += 1;
        }
        previous_row = Some(event.row);
        if event.lane == lane {
            return Ok(Some((nth, event.foot)));
        }
        seen[usize::from(event.lane)] = true;
    }
    Ok(None)
}

pub fn express(
    graph: &StepGraph,
    last: &PathNode,
    mines: &[SourceMine],
    cancelled: &AtomicBool,
) -> Result<Vec<ExpressedMine>, Error> {
    check(cancelled)?;
    let nodes = path_nodes(graph, last, cancelled)?;
    let columns = graph.columns();
    let (releases, steps) = actions(&nodes, columns, cancelled)?;
    let (mut step_index, mut release_end) = (0, 0);
    let mut previous_row = None;
    let mut output = Vec::new();
    for &source in mines {
        check(cancelled)?;
        if usize::from(source.lane) >= columns
            || !source.time.is_finite()
            || source.row > i32::MAX as u64
            || previous_row.is_some_and(|row| row > source.row)
        {
            return Err(Error::InvalidEdit(
                "Invalid or unordered source mines".into(),
            ));
        }
        while step_index < steps.len() && steps[step_index].row <= source.row {
            step_index += 1;
        }
        while release_end < releases.len() && releases[release_end].row < source.row {
            release_end += 1;
        }
        let after = neighboring(
            releases[..release_end].iter().rev(),
            columns,
            source.lane,
            cancelled,
        )?;
        let before = if after.is_none() {
            neighboring(steps[step_index..].iter(), columns, source.lane, cancelled)?
        } else {
            None
        };
        let (kind, association) = if after.is_some() {
            (MineType::AfterArrow, after)
        } else if before.is_some() {
            (MineType::BeforeArrow, before)
        } else {
            (MineType::NoArrow, None)
        };
        output.push(ExpressedMine {
            source,
            kind,
            nth: association.map(|(n, _)| n),
            foot: association.map(|(_, f)| f),
        });
        previous_row = Some(source.row);
    }
    output.sort_by_key(|mine| (mine.source.row, mine.kind, mine.nth, mine.foot));
    check(cancelled)?;
    Ok(output)
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::InvalidEdit("Mine expression cancelled".into()))
    } else {
        Ok(())
    }
}
