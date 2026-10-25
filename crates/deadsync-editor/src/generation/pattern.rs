//! GrooveAuthor CreateWithPattern search, including start/end footing rules,
//! two look-ahead steps, per-node pruning, seeded choices, and full path costs.
use super::{
    config::{EndFootChoice, PatternConfig, PerformedConfig, StartFootChoice, StartingFootChoice},
    graph::{ArrowState, FootAction, FootState, GraphLink, StepGraph},
    random::SeededRandom,
    search::{BLANK_LINK, RootContext, Scoring, SearchNode},
    step_data::StepType,
};
use crate::{
    Error,
    history::Session,
    notes::{Kind, Note, Position, ROWS_PER_BEAT},
    pad::Pad,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct PatternRequest {
    pub layout: String,
    pub start: u64,
    pub end: u64,
    pub seed: i32,
    pub previous_foot: i32,
    pub previous_footing: [i32; 2],
    pub following_footing: [i32; 2],
    pub context: RootContext,
    pub config: PerformedConfig,
    pub pattern: PatternConfig,
    pub average_nps: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndWarning {
    RelaxedToOneFoot,
    FollowingFootingUnreachable,
}

#[derive(Clone, Debug)]
pub struct GeneratedStep {
    pub row: u64,
    pub time: f64,
    pub feet: [[FootState; 2]; 2],
    pub link: GraphLink,
}

pub struct PatternPreview {
    pub layout: String,
    pub start: u64,
    pub end: u64,
    pub steps: Vec<GeneratedStep>,
    pub warnings: Vec<EndWarning>,
}

impl PatternPreview {
    /// Call only after the user accepts the preview. The original selection
    /// and generated notes are one undoable gesture; a stale result is refused.
    pub fn apply(
        &self,
        session: &mut Session,
        chart: usize,
        player: u32,
        expected_revision: u64,
    ) -> Result<bool, Error> {
        if session.revision() != expected_revision {
            return Err(Error::InvalidEdit(
                "The chart changed after generation; generate a fresh preview".into(),
            ));
        }
        if session
            .document()
            .charts()
            .get(chart)
            .is_none_or(|chart| chart.steps_type != self.layout)
        {
            return Err(Error::InvalidEdit(
                "Generated pattern belongs to another layout".into(),
            ));
        }
        let grid = session.grid(chart)?;
        if player >= grid.players() {
            return Err(Error::InvalidEdit(
                "Generated pattern participant does not exist".into(),
            ));
        }
        let mut edits: Vec<_> = grid
            .iter()
            .filter(|(pos, _)| pos.player == player && pos.row >= self.start && pos.row <= self.end)
            .map(|(pos, _)| (pos, None))
            .collect();
        for step in &self.steps {
            for foot in 0..2 {
                for portion in 0..2 {
                    if step.link.feet[foot][portion].valid {
                        let lane = step.feet[foot][portion].lane.ok_or_else(|| {
                            Error::InvalidFormat("Generated step has no lane".into())
                        })?;
                        edits.push((
                            Position {
                                player,
                                lane,
                                row: step.row,
                            },
                            Some(Note {
                                kind: Kind::Tap,
                                ..Note::tap()
                            }),
                        ));
                    }
                }
            }
        }
        session.edit_notes(chart, "Generate pattern", edits)
    }
}

/// Operates only on immutable inputs. The caller can run this on a worker,
/// cancel it, and discard an old result without touching an editor document.
pub fn generate(
    request: &PatternRequest,
    timing: &super::timing::GenerationTiming,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Result<PatternPreview, Error> {
    request.pattern.validate()?;
    request.config.validate()?;
    if request.start > request.end
        || request.context.total_steps < 0
        || !request.average_nps.is_finite()
        || request.average_nps < 0.0
        || request
            .context
            .previous_times
            .iter()
            .any(|time| !time.is_finite())
        || ![-1, 0, 1].contains(&request.previous_foot)
    {
        return invalid("Invalid pattern range or preceding-chart context");
    }
    check_cancel(cancelled)?;
    let graph = StepGraph::builtin(&request.layout)?;
    let pad = Pad::builtin(&request.layout)?;
    if request.context.lane_counts.len() != pad.arrows.len() {
        return invalid("Pattern lane counts do not match its layout");
    }
    let weights = request.config.normalized_weights(&request.layout)?;
    let spacing = ROWS_PER_BEAT / u64::from(request.pattern.beat_sub_division);
    let extended_end = request
        .end
        .checked_add(spacing * 2)
        .filter(|&end| end <= i32::MAX as u64)
        .ok_or_else(|| {
            Error::InvalidEdit("Pattern range exceeds the reference row limit".into())
        })?;
    let count = (extended_end - request.start) / spacing + 1;
    let count = usize::try_from(count)
        .map_err(|_| Error::InvalidEdit("Pattern range is too large".into()))?;
    if count <= 2 {
        return invalid("Pattern range is too short");
    }
    let times = timing.times_checked(
        (0..count).map(|index| request.start + index as u64 * spacing),
        || check_cancel(cancelled),
    )?;
    let (root, first_types) = starting_stance(request, pad, &graph)?;
    let mut random = SeededRandom::new(request.seed);
    let mut foot = match request.pattern.starting_foot_choice {
        StartingFootChoice::Random => usize::from(random.next_f64() >= 0.5),
        StartingFootChoice::Automatic => usize::from(request.previous_foot == 0),
        StartingFootChoice::Specified => request.pattern.starting_foot_specified as usize,
    };
    let scoring = Scoring {
        graph: &graph,
        pad,
        config: &request.config,
        pattern: Some(&request.pattern),
        normalized_weights: &weights,
        average_nps: request.average_nps,
    };
    let root_node = scoring.root(
        root,
        request.context.previous_times[foot],
        random.next_f64(),
        &request.context,
    );
    let mut frontier = vec![root_node];
    let mut allowed = first_types[foot].clone();
    for depth in 0..count {
        check_cancel(cancelled)?;
        let row = request.start + depth as u64 * spacing;
        let time = times[depth];
        if !time.is_finite() {
            return invalid("Pattern timing produced a non-finite timestamp");
        }
        let current_foot = foot;
        foot = 1 - foot;
        let next_allowed = if depth == 0 {
            first_types[foot].clone()
        } else if depth == count - 3 || depth == count - 2 {
            ending_step_types(request, foot).to_vec()
        } else {
            vec![StepType::NewArrow, StepType::SameArrow]
        };
        let mut next: Vec<Arc<SearchNode>> = Vec::new();
        let mut by_node: HashMap<usize, usize> = HashMap::new();
        for previous in &frontier {
            check_cancel(cancelled)?;
            for &step_type in &allowed {
                let mut link = BLANK_LINK;
                link.feet[current_foot][0].valid = true;
                link.feet[current_foot][0].step = step_type;
                link.feet[current_foot][0].action = FootAction::Tap;
                let Some(edge) = graph.edge_for_link(previous.node, &link) else {
                    continue;
                };
                for &target in &graph.targets[edge.targets.clone()] {
                    let candidate =
                        scoring.advance(previous, target, link, time, random.next_f64(), 0.0);
                    if let Some(&index) = by_node.get(&target) {
                        if candidate.cost.compare(&next[index].cost).is_le() {
                            next[index] = candidate;
                        }
                    } else {
                        by_node.insert(target, next.len());
                        next.push(candidate);
                    }
                }
            }
        }
        if next.is_empty() {
            return invalid(&format!("No pattern path at row {row}"));
        }
        frontier = next;
        allowed = next_allowed;
        progress(depth + 1, count);
    }
    check_cancel(cancelled)?;
    let mut warnings = Vec::new();
    let mut remaining: Vec<_> = frontier
        .iter()
        .filter(|node| final_footing_matches(request, pad, &graph, node.node, None))
        .collect();
    if remaining.is_empty() {
        remaining = frontier
            .iter()
            .filter(|node| final_footing_matches(request, pad, &graph, node.node, Some(foot)))
            .collect();
        if remaining.is_empty() {
            remaining = frontier.iter().collect();
            warnings.push(EndWarning::FollowingFootingUnreachable);
        } else {
            warnings.push(EndWarning::RelaxedToOneFoot);
        }
    }
    let best = remaining
        .into_iter()
        .min_by(|a, b| {
            let mut a_cost = a.cost.clone();
            a_cost.pattern_type = a.final_pattern_cost(&request.pattern);
            let mut b_cost = b.cost.clone();
            b_cost.pattern_type = b.final_pattern_cost(&request.pattern);
            a_cost.compare(&b_cost)
        })
        .unwrap();
    let mut steps = Vec::with_capacity(count - 2);
    let mut current = Some(Arc::clone(best));
    while let Some(node) = current {
        if node.depth > 0 && node.depth <= count - 2 {
            steps.push(GeneratedStep {
                row: request.start + (node.depth - 1) as u64 * spacing,
                time: node.time,
                feet: graph.nodes[node.node].state.feet,
                link: node.link.unwrap(),
            });
        }
        current = node.previous.clone();
    }
    steps.reverse();
    Ok(PatternPreview {
        layout: request.layout.clone(),
        start: request.start,
        end: request.end,
        steps,
        warnings,
    })
}

fn starting_stance(
    request: &PatternRequest,
    pad: &Pad,
    graph: &StepGraph,
) -> Result<(usize, [Vec<StepType>; 2]), Error> {
    let mut lanes = [
        request.pattern.left_foot_start_lane_specified,
        request.pattern.right_foot_start_lane_specified,
    ];
    let choices = [
        request.pattern.left_foot_start_choice,
        request.pattern.right_foot_start_choice,
    ];
    let mut types = [vec![StepType::SameArrow], vec![StepType::SameArrow]];
    for foot in 0..2 {
        match choices[foot] {
            StartFootChoice::SpecifiedLane => {
                if lanes[foot] < 0
                    || (lanes[foot] as usize >= pad.arrows.len()
                        && request.previous_footing[foot] >= 0
                        && (request.previous_footing[foot] as usize) < pad.arrows.len())
                {
                    lanes[foot] = request.previous_footing[foot];
                }
            }
            StartFootChoice::AutomaticSameLane => lanes[foot] = request.previous_footing[foot],
            StartFootChoice::AutomaticNewLane => {
                lanes[foot] = request.previous_footing[foot];
                types[foot] = vec![StepType::NewArrow];
            }
            StartFootChoice::AutomaticSameOrNewLane => {
                lanes[foot] = request.previous_footing[foot];
                types[foot] = vec![StepType::NewArrow, StepType::SameArrow];
            }
        }
    }
    if lanes
        .iter()
        .any(|&lane| lane < 0 || lane as usize >= pad.arrows.len())
    {
        return invalid("Cannot find the pattern's starting footing");
    }
    let root = graph
        .find_normal_stance(
            FootState {
                lane: Some(lanes[0] as u16),
                state: ArrowState::Resting,
            },
            FootState {
                lane: Some(lanes[1] as u16),
                state: ArrowState::Resting,
            },
        )
        .ok_or_else(|| Error::InvalidEdit("Cannot find the pattern's starting footing".into()))?;
    Ok((root, types))
}

fn ending_step_types(request: &PatternRequest, foot: usize) -> &'static [StepType] {
    let choice = [
        request.pattern.left_foot_end_choice,
        request.pattern.right_foot_end_choice,
    ][foot];
    match choice {
        EndFootChoice::AutomaticNewLaneToFollowing => &[StepType::NewArrow],
        EndFootChoice::SpecifiedLane | EndFootChoice::AutomaticSameLaneToFollowing => {
            &[StepType::SameArrow]
        }
        _ => &[StepType::NewArrow, StepType::SameArrow],
    }
}

fn final_footing_matches(
    request: &PatternRequest,
    pad: &Pad,
    graph: &StepGraph,
    node: usize,
    only_foot: Option<usize>,
) -> bool {
    let choices = [
        request.pattern.left_foot_end_choice,
        request.pattern.right_foot_end_choice,
    ];
    let specified = [
        request.pattern.left_foot_end_lane_specified,
        request.pattern.right_foot_end_lane_specified,
    ];
    for foot in 0..2 {
        if only_foot.is_some_and(|selected| selected != foot) {
            continue;
        }
        if only_foot.is_none() && choices[foot] == EndFootChoice::AutomaticIgnoreFollowingSteps {
            continue;
        }
        let lane = if choices[foot] == EndFootChoice::SpecifiedLane {
            specified[foot]
        } else {
            request.following_footing[foot]
        };
        if lane >= 0
            && (lane as usize) < pad.arrows.len()
            && graph.nodes[node].state.feet[foot][0].lane != Some(lane as u16)
        {
            return false;
        }
    }
    true
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        return invalid("Generation cancelled");
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.to_owned()))
}
