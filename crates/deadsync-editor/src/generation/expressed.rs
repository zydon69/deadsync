//! Expressed-chart lane state and graph matching from ExpressedChart.cs (MIT,
//! StepManiaLibrary bdbc427f). This is the matching stage, not the cost search.
use super::graph::{ArrowState, FootAction, GraphLink, NodeState, StepGraph};
use crate::{
    Error,
    notes::{Grid, Kind},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchState {
    #[default]
    Empty,
    Tap,
    Fake,
    Lift,
    Hold,
    Holding,
    Roll,
    Rolling,
}
impl SearchState {
    pub fn is_step(self) -> bool {
        matches!(
            self,
            Self::Tap | Self::Fake | Self::Lift | Self::Hold | Self::Roll
        )
    }
    pub fn instance_type(self) -> InstanceType {
        match self {
            Self::Roll | Self::Rolling => InstanceType::Roll,
            Self::Fake => InstanceType::Fake,
            Self::Lift => InstanceType::Lift,
            _ => InstanceType::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InstanceType {
    #[default]
    Default,
    Roll,
    Fake,
    Lift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinkInstance {
    pub link: GraphLink,
    pub types: [[InstanceType; 2]; 2],
}

/// Match the entire lane state, including untouched held arrows. A roll shares
/// graph geometry with a hold; fake/lift share tap geometry but retain their
/// specific instance type on the acting foot portion.
pub fn match_state(
    state: &[SearchState],
    node: NodeState,
    link: GraphLink,
) -> Option<LinkInstance> {
    let mut generated = vec![SearchState::Empty; state.len()];
    match_state_buffered(state, node, link, &mut generated)
}

fn match_state_buffered(
    state: &[SearchState],
    node: NodeState,
    link: GraphLink,
    generated: &mut [SearchState],
) -> Option<LinkInstance> {
    generated.fill(SearchState::Empty);
    for foot in 0..2 {
        for portion in 0..2 {
            let position = node.feet[foot][portion];
            let Some(lane) = position.lane.map(usize::from) else {
                continue;
            };
            let slot = generated.get_mut(lane)?;
            let action = link.feet[foot][portion];
            if action.valid && position.state == action.action.resulting_state() {
                *slot = match position.state {
                    ArrowState::Held => SearchState::Hold,
                    ArrowState::Resting if action.action != FootAction::Release => SearchState::Tap,
                    _ => SearchState::Empty,
                };
            } else if position.state == ArrowState::Held {
                *slot = SearchState::Holding;
            }
        }
    }
    for (&generated, &expected) in generated.iter().zip(state) {
        if generated == expected
            || matches!(
                (generated, expected),
                (SearchState::Hold, SearchState::Roll)
                    | (SearchState::Tap, SearchState::Fake | SearchState::Lift)
                    | (SearchState::Holding, SearchState::Rolling)
            )
        {
            continue;
        }
        return None;
    }
    let mut types = [[InstanceType::Default; 2]; 2];
    for (foot, portions) in types.iter_mut().enumerate() {
        for (portion, kind) in portions.iter_mut().enumerate() {
            let position = node.feet[foot][portion];
            let action = link.feet[foot][portion];
            if let Some(lane) = position.lane.map(usize::from) {
                if action.valid {
                    *kind = match (state[lane], action.action) {
                        (SearchState::Roll, FootAction::Hold) => InstanceType::Roll,
                        (SearchState::Fake, FootAction::Tap) => InstanceType::Fake,
                        (SearchState::Lift, FootAction::Tap) => InstanceType::Lift,
                        _ => InstanceType::Default,
                    };
                }
            }
        }
    }
    Some(LinkInstance { link, types })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub node: usize,
    pub instance: LinkInstance,
}

/// Preserve original graph edge/target traversal order for deterministic search.
/// Cost-based pruning belongs to the next stage, after these candidates exist.
pub fn candidates(
    graph: &StepGraph,
    parent: usize,
    state: &[SearchState],
    release: bool,
    cancelled: &AtomicBool,
) -> Result<Vec<Candidate>, Error> {
    if state.len() != graph.columns() {
        return Err(Error::InvalidEdit(
            "Interpretation lane count does not match its graph".into(),
        ));
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Interpretation cancelled".into()));
    }
    let mut generated = vec![SearchState::Empty; state.len()];
    let node = graph
        .nodes
        .get(parent)
        .ok_or_else(|| Error::InvalidEdit("Unknown interpretation node".into()))?;
    let multiple = state.iter().filter(|value| value.is_step()).count() > 1;
    let mut result = Vec::new();
    for edge in &graph.edges[node.edges.clone()] {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Interpretation cancelled".into()));
        }
        if (!multiple && (edge.link.is_jump() || edge.link.involves_bracket(false)))
            || (release && !edge.link.is_release())
        {
            continue;
        }
        for &target in &graph.targets[edge.targets.clone()] {
            if let Some(instance) =
                match_state_buffered(state, graph.nodes[target].state, edge.link, &mut generated)
            {
                result.push(Candidate {
                    node: target,
                    instance,
                });
            }
        }
    }
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub row: u64,
    pub release: bool,
    pub state: Vec<SearchState>,
    pub last_mines: Vec<u64>,
    pub last_releases: Vec<u64>,
}

#[derive(Default)]
struct Row {
    releases: Vec<usize>,
    mines: Vec<usize>,
    steps: Vec<(usize, SearchState)>,
}

/// Translate one participant's lossless editor grid into the exact transient
/// states consumed by ExpressedChart.Search. Release and step phases at the
/// same row are distinct; mines influence steps after releases are processed.
pub fn frames(grid: &Grid, player: u32, cancelled: &AtomicBool) -> Result<Vec<Frame>, Error> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::InvalidEdit("Interpretation cancelled".into()));
    }
    if player >= grid.players() {
        return Err(Error::InvalidEdit(
            "Interpretation participant does not exist".into(),
        ));
    }
    let mut rows: BTreeMap<u64, Row> = BTreeMap::new();
    for (position, note) in grid
        .iter()
        .filter(|(position, _)| position.player == player)
    {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Interpretation cancelled".into()));
        }
        if position.row > i32::MAX as u64 || note.end.is_some_and(|end| end > i32::MAX as u64) {
            return Err(Error::InvalidEdit(
                "Interpretation exceeds reference row range".into(),
            ));
        }
        let lane = usize::from(position.lane);
        let row = rows.entry(position.row).or_default();
        let state = match note.kind {
            Kind::Tap | Kind::Keysound | Kind::Attack => SearchState::Tap,
            Kind::Fake => SearchState::Fake,
            Kind::Lift => SearchState::Lift,
            Kind::Hold => SearchState::Hold,
            Kind::Roll => SearchState::Roll,
            Kind::Mine => {
                row.mines.push(lane);
                continue;
            }
        };
        row.steps.push((lane, state));
        if let Some(end) = note.end {
            rows.entry(end).or_default().releases.push(lane);
        }
    }
    let columns = usize::from(grid.columns());
    let mut state = vec![SearchState::Empty; columns];
    let mut last_mines = vec![0; columns];
    let mut last_releases = vec![0; columns];
    let mut output = Vec::new();
    for (row, events) in rows {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::InvalidEdit("Interpretation cancelled".into()));
        }
        if !events.releases.is_empty() {
            for lane in events.releases {
                state[lane] = SearchState::Empty;
                last_releases[lane] = row;
            }
            output.push(Frame {
                row,
                release: true,
                state: state.clone(),
                last_mines: last_mines.clone(),
                last_releases: last_releases.clone(),
            });
        }
        for lane in events.mines {
            last_mines[lane] = row;
        }
        if !events.steps.is_empty() {
            for (lane, value) in events.steps {
                state[lane] = value;
            }
            output.push(Frame {
                row,
                release: false,
                state: state.clone(),
                last_mines: last_mines.clone(),
                last_releases: last_releases.clone(),
            });
        }
        for (lane, value) in state.iter_mut().enumerate() {
            *value = match *value {
                SearchState::Tap | SearchState::Fake | SearchState::Lift => {
                    last_releases[lane] = row;
                    SearchState::Empty
                }
                SearchState::Hold => SearchState::Holding,
                SearchState::Roll => SearchState::Rolling,
                other => other,
            };
        }
    }
    Ok(output)
}
