//! PerformedChart.CreateFromExpressedChart step search (MIT, bdbc427f).
//! Full conversion continues the search PRNG through destination mine placement.
use super::{
    config::PerformedConfig,
    expressed::{LinkInstance, StepEvent},
    fallbacks::{self, Fallbacks},
    graph::{ArrowState, FootState, StepGraph},
    random::SeededRandom,
    search::{LaneAction, RootContext, Scoring, SearchNode},
};
use crate::{Error, pad::Pad};
use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
};

pub struct Request<'a> {
    pub layout: &'a str,
    pub events: &'a [StepEvent],
    pub config: &'a PerformedConfig,
    pub fallbacks: Option<&'a Fallbacks>,
    pub seed: i32,
    pub roots: Option<&'a [Vec<usize>]>,
}
#[derive(Clone, Copy, Debug)]
pub struct PerformanceStep {
    pub row: u64,
    pub time: f64,
    pub node: usize,
    pub instance: LinkInstance,
}
pub struct StepPerformance {
    pub layout: String,
    pub root: usize,
    pub root_tier: usize,
    pub steps: Vec<PerformanceStep>,
    pub skipped_events: usize,
    pub last: Option<Arc<SearchNode>>,
    /// PRNG continuation, advanced further by full conversion mine placement.
    pub random: SeededRandom,
}
struct Cursor {
    path: Arc<SearchNode>,
    links: Arc<[LinkInstance]>,
}

pub fn root_tiers(graph: &StepGraph, pad: &Pad) -> Result<Vec<Vec<usize>>, Error> {
    pad.starting_positions
        .iter()
        .map(|tier| {
            tier.iter()
                .map(|lanes| {
                    let foot = |lane| FootState {
                        lane: Some(lane as u16),
                        state: ArrowState::Resting,
                    };
                    graph
                        .find_normal_stance(foot(lanes[0]), foot(lanes[1]))
                        .ok_or_else(|| {
                            Error::InvalidEdit(
                                "Starting position is absent from the destination graph".into(),
                            )
                        })
                })
                .collect()
        })
        .collect()
}
pub fn average_nps(events: &[StepEvent]) -> f64 {
    let (mut start, mut end, mut count) = (f64::MAX, 0.0f64, 0u64);
    for event in events {
        for action in event.instance.link.feet.iter().flatten() {
            if action.valid && action.action != super::graph::FootAction::Release {
                start = start.min(event.time);
                count += 1;
                end = event.time;
            }
        }
    }
    if end > start {
        count as f64 / (end - start)
    } else {
        0.0
    }
}
fn fallback_cost(links: &[LinkInstance], index: usize) -> f64 {
    let link = links[index];
    if link.link.is_blank() {
        return 1000.0;
    }
    if fallbacks::contains_blank_foot(links[0], link) {
        return 900.0;
    }
    if links.len() == 1 {
        return 0.0;
    }
    fallbacks::removed_steps(links[0], link) as f64 * 100.0
        + index as f64 / (links.len() - 1) as f64
}
fn overlaps_release(child: &SearchNode, events: &[StepEvent]) -> bool {
    let Some(parent) = child
        .previous
        .as_deref()
        .filter(|parent| parent.previous.is_some())
    else {
        return false;
    };
    if events[parent.depth - 1].row != events[child.depth - 1].row {
        return false;
    }
    parent
        .actions
        .iter()
        .zip(&child.actions)
        .any(|(&before, &now)| {
            before == LaneAction::Release && !matches!(now, LaneAction::None | LaneAction::Release)
        })
}
fn prune(cursors: Vec<Cursor>) -> Vec<Cursor> {
    let mut slots: HashMap<usize, usize> = HashMap::new();
    let mut best: Vec<Cursor> = Vec::new();
    for cursor in cursors {
        if let Some(&slot) = slots.get(&cursor.path.node) {
            if cursor.path.cost.compare(&best[slot].path.cost) != Ordering::Greater {
                best[slot] = cursor;
            }
        } else {
            slots.insert(cursor.path.node, best.len());
            best.push(cursor);
        }
    }
    best
}
pub fn convert_steps(
    request: &Request<'_>,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(usize, usize, usize),
) -> Result<StepPerformance, Error> {
    check(cancelled)?;
    request.config.validate()?;
    let graph = StepGraph::builtin(request.layout)?;
    let pad = Pad::builtin(request.layout)?;
    let weights = request.config.normalized_weights(request.layout)?;
    let builtin_roots;
    let roots = if let Some(roots) = request.roots {
        roots
    } else {
        builtin_roots = root_tiers(&graph, pad)?;
        &builtin_roots
    };
    if roots.is_empty()
        || roots[0].is_empty()
        || roots
            .iter()
            .flatten()
            .any(|&root| root >= graph.nodes.len())
    {
        return invalid("Invalid destination starting positions");
    }
    let mut previous_row = None;
    for event in request.events {
        check(cancelled)?;
        if event.row > i32::MAX as u64
            || !event.time.is_finite()
            || previous_row.is_some_and(|row| row > event.row)
        {
            return invalid("Invalid expressed event sequence");
        }
        previous_row = Some(event.row);
    }
    let mut random = SeededRandom::new(request.seed);
    let scoring = Scoring {
        graph: &graph,
        pad,
        config: request.config,
        pattern: None,
        normalized_weights: &weights,
        average_nps: average_nps(request.events),
    };
    if request.events.is_empty() {
        return Ok(StepPerformance {
            layout: request.layout.to_owned(),
            root: roots[0][0],
            root_tier: 0,
            steps: Vec::new(),
            skipped_events: 0,
            last: None,
            random,
        });
    }
    for (tier, roots) in roots.iter().enumerate() {
        let order = random.order_indices(roots.len());
        for index in order {
            let root = roots[index];
            check(cancelled)?;
            let context = RootContext {
                lane_counts: vec![0; graph.columns()],
                previous_times: [0.0; 2],
                total_steps: 0,
                last_transition: None,
                previous_same_counts: [0; 2],
            };
            let path = scoring.root(root, 0.0, random.next_f64(), &context);
            let links =
                fallbacks::replacements(request.events[0].instance, request.fallbacks, cancelled)?;
            let mut current = vec![Cursor { path, links }];
            progress(tier, 0, request.events.len());
            for (depth, event) in request.events.iter().enumerate() {
                check(cancelled)?;
                let mut children = Vec::new();
                for cursor in &current {
                    for (index, &instance) in cursor.links.iter().enumerate() {
                        check(cancelled)?;
                        let cost = fallback_cost(&cursor.links, index);
                        if instance.link.is_blank() {
                            let path = scoring.advance_instance(
                                &cursor.path,
                                cursor.path.node,
                                instance,
                                event.time,
                                random.next_f64(),
                                cost,
                            );
                            if !overlaps_release(&path, request.events) {
                                // The reference carries the same outgoing replacements
                                // across a blank instead of advancing their source event.
                                children.push(Cursor {
                                    path,
                                    links: Arc::clone(&cursor.links),
                                });
                            }
                        } else if let Some(edge) =
                            graph.edge_for_link(cursor.path.node, &instance.link)
                        {
                            for &destination in &graph.targets[edge.targets.clone()] {
                                check(cancelled)?;
                                let links = if let Some(next) = request.events.get(depth + 1) {
                                    fallbacks::replacements(
                                        next.instance,
                                        request.fallbacks,
                                        cancelled,
                                    )?
                                } else {
                                    Arc::from([])
                                };
                                let path = scoring.advance_instance(
                                    &cursor.path,
                                    destination,
                                    instance,
                                    event.time,
                                    random.next_f64(),
                                    cost,
                                );
                                if !overlaps_release(&path, request.events) {
                                    children.push(Cursor { path, links });
                                }
                            }
                        }
                    }
                }
                current = prune(children);
                progress(tier, depth + 1, request.events.len());
                // The source checks completion before detecting a dead-end at
                // the final depth; normal valid replacements always have a path.
                if current.is_empty() {
                    if depth + 1 == request.events.len() {
                        return Ok(StepPerformance {
                            layout: request.layout.to_owned(),
                            root,
                            root_tier: tier,
                            steps: Vec::new(),
                            skipped_events: request.events.len(),
                            last: None,
                            random,
                        });
                    }
                    break;
                }
            }
            if let Some(best) = current.iter().reduce(|best, next| {
                if next.path.cost.compare(&best.path.cost) == Ordering::Less {
                    next
                } else {
                    best
                }
            }) {
                if best.path.depth != request.events.len() {
                    continue;
                }
                check(cancelled)?;
                let mut steps = Vec::new();
                let mut node = Some(best.path.as_ref());
                while let Some(current) = node.filter(|node| node.previous.is_some()) {
                    check(cancelled)?;
                    let instance = current
                        .instance
                        .expect("Conversion nodes retain their incoming instance");
                    if !instance.link.is_blank() {
                        let event = &request.events[current.depth - 1];
                        steps.push(PerformanceStep {
                            row: event.row,
                            time: event.time,
                            node: current.node,
                            instance,
                        });
                    }
                    node = current.previous.as_deref();
                }
                steps.reverse();
                return Ok(StepPerformance {
                    layout: request.layout.to_owned(),
                    root,
                    root_tier: tier,
                    skipped_events: request.events.len() - steps.len(),
                    steps,
                    last: Some(Arc::clone(&best.path)),
                    random,
                });
            }
        }
    }
    invalid("No performed chart found from the configured starting positions")
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(AtomicOrdering::Relaxed) {
        invalid("Chart conversion cancelled")
    } else {
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}

pub struct Performance {
    pub steps: StepPerformance,
    pub mines: Vec<super::mines::PlacedMine>,
    pub dropped_mines: usize,
}
pub fn convert(
    request: &Request<'_>,
    mines: &[super::mines::ExpressedMine],
    cancelled: &AtomicBool,
    progress: impl FnMut(usize, usize, usize),
) -> Result<Performance, Error> {
    let mut steps = convert_steps(request, cancelled, progress)?;
    let graph = StepGraph::builtin(request.layout)?;
    let mut nodes = vec![super::mines::MovementNode {
        row: 0,
        state: graph.nodes[steps.root].state,
        link: None,
    }];
    nodes.extend(steps.steps.iter().map(|step| super::mines::MovementNode {
        row: step.row,
        state: graph.nodes[step.node].state,
        link: Some(step.instance.link),
    }));
    let placed = super::mines::place(&nodes, graph.columns(), mines, &mut steps.random, cancelled)?;
    Ok(Performance {
        dropped_mines: mines.len() - placed.len(),
        steps,
        mines: placed,
    })
}
