//! Surrounding-footing and transition rules from GrooveAuthor
//! ActionAutoGeneratePatterns.cs, 48e18f71 (MIT). This consumes independently
//! interpreted preceding/following regions; interpreting the whole chart would
//! change their natural roots and the following-foot choice.
use super::{
    config::PerformedConfig,
    geometry,
    graph::{ArrowState, StepGraph},
    interpretation_path::PathNode,
    search::RootContext,
    step_data::StepType,
};
use crate::{Error, pad::Pad};
use std::sync::atomic::{AtomicBool, Ordering};

/// Original editor-event order, including overlapping hold heads which may
/// precede the requested region. `is_step` includes fake/lift/hold heads, but
/// excludes mines, keysounds, hold tails and timing events.
#[derive(Clone, Copy, Debug)]
pub struct ContextEvent {
    pub row: u64,
    pub lane: Option<u16>,
    pub is_step: bool,
}

pub struct ContextRequest<'a> {
    pub start: u64,
    pub end: u64,
    pub num_steps: u64,
    pub total_steps_before: i64,
    pub range_starts_after_chart: bool,
    pub preceding: &'a PathNode,
    pub following: &'a PathNode,
    pub preceding_events: &'a [ContextEvent],
    pub following_events: &'a [ContextEvent],
    pub config: &'a PerformedConfig,
}
#[derive(Clone, Debug)]
pub struct PatternContext {
    pub previous_foot: i32,
    pub previous_footing: [i32; 2],
    pub following_foot: i32,
    pub following_footing: [i32; 2],
    pub root: RootContext,
}

pub fn derive(
    graph: &StepGraph,
    pad: &Pad,
    request: &ContextRequest<'_>,
    lane_counts: Vec<u64>,
    cancelled: &AtomicBool,
) -> Result<PatternContext, Error> {
    check(cancelled)?;
    request.config.validate()?;
    if graph.columns() != pad.arrows.len() {
        return invalid("Context pad does not match its graph");
    }
    if request.start > request.end
        || request.num_steps == 0
        || request.total_steps_before < 0
        || request.total_steps_before > i64::from(i32::MAX)
        || request.end > i32::MAX as u64
        || !request
            .config
            .transitions
            .transition_cutoff_percentage
            .is_finite()
        || lane_counts.len() != graph.columns()
    {
        return invalid("Invalid surrounding pattern context");
    }
    for event in request
        .preceding_events
        .iter()
        .chain(request.following_events)
    {
        check(cancelled)?;
        if event
            .lane
            .is_some_and(|lane| usize::from(lane) >= graph.columns())
            || (event.is_step && event.lane.is_none())
        {
            return invalid("Context event lane is invalid");
        }
    }
    let before = path(request.preceding, graph, cancelled)?;
    let after = path(request.following, graph, cancelled)?;
    let mut following = Footing::default();
    let mut scanner = Scanner::new(request.following_events, 0, true, graph.columns());
    for node in after.into_iter().filter(|node| node.row >= request.end) {
        check(cancelled)?;
        scanner.update(node.row, cancelled)?;
        following.update(graph, node, &scanner.stepped);
        if following.found == 2 {
            break;
        }
    }

    let mut same_counts = [0u64; 2];
    let mut last_side = None;
    let mut transition_within = None;
    let mut steps_in_region = 0i64;
    let mut event_index = 0;
    let mut lane_sum = 0u64;
    let mut checked_steps = 0u64;
    let mut last = None;
    let mut transition_steps = (request.total_steps_before
        - i64::from(request.config.transitions.steps_per_transition_min))
    .max(0);
    for node in before
        .into_iter()
        .take_while(|node| node.row < request.start)
    {
        check(cancelled)?;
        if let Some(instance) = node.instance {
            if let Some((step, foot)) = instance.link.single_step() {
                if step == StepType::SameArrow {
                    same_counts[foot] += 1;
                } else {
                    same_counts[foot] = 0;
                }
            } else {
                for (foot, count) in same_counts.iter_mut().enumerate() {
                    if instance.link.involves_step(foot) {
                        *count = 0;
                    }
                }
            }
        } else {
            same_counts = [0; 2];
        }
        if node
            .instance
            .is_some_and(|instance| !instance.link.is_release())
        {
            steps_in_region += 1;
        }
        let side = geometry::side(
            pad,
            graph.nodes[node.node].state,
            request.config.transitions.transition_cutoff_percentage,
        )
        .map(|(side, _)| side);
        if side.is_some() && side != last_side {
            if last_side.is_some() {
                transition_within = Some(steps_in_region);
            }
            last_side = side;
        }
        while let Some(event) = request
            .preceding_events
            .get(event_index)
            .filter(|event| event.row <= node.row)
        {
            check(cancelled)?;
            if event.is_step {
                lane_sum += u64::from(event.lane.unwrap());
                checked_steps += 1;
            }
            event_index += 1;
        }
        last = Some(node);
    }
    if let Some(within) = transition_within {
        transition_steps = request.total_steps_before - steps_in_region + within;
    }
    if last_side.is_none() && request.range_starts_after_chart && checked_steps > 0 {
        last_side = Some((lane_sum as f64 / checked_steps as f64) < graph.columns() as f64 * 0.5);
    }

    let mut previous = Footing::default();
    if let Some(last) = last {
        let index = event_index.min(request.preceding_events.len().saturating_sub(1));
        let mut scanner = Scanner::new(request.preceding_events, index, false, graph.columns());
        let mut current = Some(last);
        while let Some(node) = current {
            check(cancelled)?;
            scanner.update(node.row, cancelled)?;
            previous.update(graph, node, &scanner.stepped);
            if previous.found == 2 {
                break;
            }
            current = node.previous.as_deref();
        }
    }
    let mut root = request.preceding;
    while let Some(parent) = root.previous.as_deref() {
        check(cancelled)?;
        root = parent;
    }
    let root_lanes = graph.nodes[root.node]
        .state
        .feet
        .map(|portions| portions[0].lane.map(i32::from).unwrap_or(-1));
    for (foot, lane) in previous.lanes.iter_mut().enumerate() {
        if *lane == -1 {
            *lane = root_lanes[foot];
        }
    }
    if previous.lanes[0] == previous.lanes[1] {
        previous.lanes = root_lanes;
    }
    if previous.foot == -1 {
        previous.foot = if following.foot != -1 {
            if request.num_steps % 2 == 0 {
                1 - following.foot
            } else {
                following.foot
            }
        } else {
            0
        };
    }
    Ok(PatternContext {
        previous_foot: previous.foot,
        previous_footing: previous.lanes,
        following_foot: following.foot,
        following_footing: following.lanes,
        root: RootContext {
            lane_counts,
            previous_times: previous.times,
            total_steps: request.total_steps_before,
            previous_same_counts: same_counts,
            // CreateWithPattern creates its synthetic transition node only for
            // a nonzero absolute step count, even when a side was discovered.
            last_transition: (transition_steps != 0).then_some((last_side, transition_steps)),
        },
    })
}

fn path<'a>(
    last: &'a PathNode,
    graph: &StepGraph,
    cancelled: &AtomicBool,
) -> Result<Vec<&'a PathNode>, Error> {
    let mut nodes = Vec::new();
    let mut current = Some(last);
    while let Some(node) = current {
        check(cancelled)?;
        if node.node >= graph.nodes.len() || node.row > i32::MAX as u64 || !node.time.is_finite() {
            return invalid("Invalid context graph node or timestamp");
        }
        nodes.push(node);
        current = node.previous.as_deref();
    }
    nodes.reverse();
    Ok(nodes)
}

struct Footing {
    foot: i32,
    lanes: [i32; 2],
    times: [f64; 2],
    found: usize,
}
impl Default for Footing {
    fn default() -> Self {
        Self {
            foot: -1,
            lanes: [-1; 2],
            times: [0.0; 2],
            found: 0,
        }
    }
}
impl Footing {
    fn update(&mut self, graph: &StepGraph, node: &PathNode, stepped: &[bool]) {
        if node
            .instance
            .is_none_or(|instance| instance.link.is_release())
        {
            return;
        }
        for (foot, portions) in graph.nodes[node.node].state.feet.iter().enumerate() {
            if self.lanes[foot] != -1 {
                continue;
            }
            for portion in portions {
                if self.lanes[foot] != -1 {
                    continue;
                }
                if portion.state == ArrowState::Lifted {
                    continue;
                }
                let Some(lane) = portion.lane.filter(|lane| stepped[usize::from(*lane)]) else {
                    continue;
                };
                if self.foot == -1 {
                    self.foot = foot as i32;
                } else if self.foot != foot as i32
                    && geometry::same_f64(self.times[self.foot as usize], node.time)
                {
                    self.foot = -1;
                }
                self.lanes[foot] = i32::from(lane);
                self.times[foot] = node.time;
                self.found += 1;
                if self.found == 2 {
                    break;
                }
            }
            if self.found == 2 {
                break;
            }
        }
    }
}
struct Scanner<'a> {
    events: &'a [ContextEvent],
    index: Option<usize>,
    forward: bool,
    row: Option<u64>,
    stepped: Vec<bool>,
}
impl<'a> Scanner<'a> {
    fn new(events: &'a [ContextEvent], index: usize, forward: bool, columns: usize) -> Self {
        Self {
            events,
            index: Some(index),
            forward,
            row: None,
            stepped: vec![false; columns],
        }
    }
    fn update(&mut self, row: u64, cancelled: &AtomicBool) -> Result<(), Error> {
        if self.row == Some(row) {
            return Ok(());
        }
        self.stepped.fill(false);
        while let Some((index, event)) = self
            .index
            .and_then(|index| self.events.get(index).map(|event| (index, event)))
        {
            check(cancelled)?;
            if (self.forward && event.row > row) || (!self.forward && event.row < row) {
                break;
            }
            if event.row == row && event.is_step {
                self.stepped[usize::from(event.lane.unwrap())] = true;
            }
            self.index = if self.forward {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            };
        }
        self.row = Some(row);
        Ok(())
    }
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        return invalid("Pattern context cancelled");
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
