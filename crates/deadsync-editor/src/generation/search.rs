//! Performed-chart path scoring, ported from SearchNode.cs (MIT, bdbc427f).
//! Costs retain the reference comparison order and f32/f64 boundaries.
use super::{
    config::{PatternConfig, PerformedConfig, StepConfig},
    geometry,
    graph::{FootAction, GraphLink, NodeState, PortionAction, StepGraph},
    step_data::StepType,
};
use crate::pad::Pad;
use std::{cmp::Ordering, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneAction {
    None,
    Tap,
    Hold,
    Release,
}
impl LaneAction {
    fn is_step(self) -> bool {
        matches!(self, Self::Tap | Self::Hold)
    }
}

pub fn actions(state: NodeState, link: GraphLink, columns: usize) -> Vec<LaneAction> {
    let mut result = vec![LaneAction::None; columns];
    for foot in 0..2 {
        for portion in 0..2 {
            let action = link.feet[foot][portion];
            if action.valid {
                if let Some(lane) = state.feet[foot][portion].lane {
                    result[usize::from(lane)] = match action.action {
                        FootAction::Tap => LaneAction::Tap,
                        FootAction::Hold => LaneAction::Hold,
                        FootAction::Release => LaneAction::Release,
                    };
                }
            }
        }
    }
    result
}

pub const BLANK_ACTION: PortionAction = PortionAction {
    valid: false,
    step: StepType::SameArrow,
    action: FootAction::Tap,
};
pub const BLANK_LINK: GraphLink = GraphLink {
    feet: [[BLANK_ACTION; 2]; 2],
};

#[derive(Clone, Default, Debug, serde::Serialize)]
pub struct Cost {
    pub fallback: f64,
    pub misleading: u64,
    pub ambiguous: u64,
    pub same_run_over_max: u64,
    pub stretch: f64,
    pub facing: f64,
    pub distance: f64,
    pub speed: f64,
    pub pattern_type: f32,
    pub lateral: f64,
    pub early_transition: i64,
    pub late_transition: i64,
    pub distribution: f32,
    pub random: f64,
}

impl Cost {
    pub fn compare(&self, other: &Self) -> Ordering {
        macro_rules! float {
            ($field:ident) => {
                if (f64::from(self.$field) - f64::from(other.$field)).abs() > 0.00001 {
                    return self.$field.partial_cmp(&other.$field).unwrap();
                }
            };
        }
        macro_rules! integer {
            ($field:ident) => {
                if self.$field != other.$field {
                    return self.$field.cmp(&other.$field);
                }
            };
        }
        float!(fallback);
        integer!(misleading);
        integer!(ambiguous);
        integer!(same_run_over_max);
        float!(stretch);
        float!(facing);
        float!(distance);
        float!(speed);
        float!(pattern_type);
        float!(lateral);
        integer!(early_transition);
        integer!(late_transition);
        float!(distribution);
        self.random.partial_cmp(&other.random).unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct RootContext {
    pub lane_counts: Vec<u64>,
    pub previous_times: [f64; 2],
    pub total_steps: i64,
    pub last_transition: Option<(Option<bool>, i64)>,
    pub previous_same_counts: [u64; 2],
}

#[derive(Clone)]
pub struct SearchNode {
    pub node: usize,
    pub link: Option<GraphLink>,
    pub instance: Option<super::expressed::LinkInstance>,
    pub previous: Option<Arc<Self>>,
    pub depth: usize,
    pub time: f64,
    pub actions: Vec<LaneAction>,
    pub cost: Cost,
    pub total_steps: i64,
    total_in_pattern: i64,
    total_inward: u64,
    total_outward: u64,
    same_steps: u64,
    new_steps: u64,
    same_since_new: [u64; 2],
    seen_new: [bool; 2],
    prefix_same: [u64; 2],
    last_transition: Option<(Option<bool>, i64)>,
    last_step_time: [f64; 2],
    last_release_time: [f64; 2],
    last_arrows: [[Option<u16>; 2]; 2],
    counts: Vec<u64>,
    body_x: f32,
    movement: i8,
    movement_start_time: f64,
    movement_start_x: f32,
    movement_steps: i64,
}

// Long charts must not recursively drop a linked history on the stack.
impl Drop for SearchNode {
    fn drop(&mut self) {
        let mut previous = self.previous.take();
        while let Some(node) = previous {
            match Arc::try_unwrap(node) {
                Ok(mut node) => previous = node.previous.take(),
                Err(_) => break,
            }
        }
    }
}

pub struct Scoring<'a> {
    pub graph: &'a StepGraph,
    pub pad: &'a Pad,
    pub config: &'a PerformedConfig,
    pub pattern: Option<&'a PatternConfig>,
    pub normalized_weights: &'a [f64],
    pub average_nps: f64,
}

impl Scoring<'_> {
    pub fn root(
        &self,
        node: usize,
        time: f64,
        random: f64,
        context: &RootContext,
    ) -> Arc<SearchNode> {
        self.make(node, None, None, time, random, 0.0, Some(context))
    }

    pub fn advance(
        &self,
        previous: &Arc<SearchNode>,
        node: usize,
        link: GraphLink,
        time: f64,
        random: f64,
        fallback: f64,
    ) -> Arc<SearchNode> {
        self.make(
            node,
            Some(link),
            Some(Arc::clone(previous)),
            time,
            random,
            fallback,
            None,
        )
    }

    pub fn advance_instance(
        &self,
        previous: &Arc<SearchNode>,
        node: usize,
        instance: super::expressed::LinkInstance,
        time: f64,
        random: f64,
        fallback: f64,
    ) -> Arc<SearchNode> {
        let mut result = self.advance(previous, node, instance.link, time, random, fallback);
        Arc::get_mut(&mut result)
            .expect("Fresh search node is unique")
            .instance = Some(instance);
        result
    }

    fn make(
        &self,
        node: usize,
        link: Option<GraphLink>,
        previous: Option<Arc<SearchNode>>,
        time: f64,
        random: f64,
        fallback: f64,
        root: Option<&RootContext>,
    ) -> Arc<SearchNode> {
        let state = self.graph.nodes[node].state;
        let columns = self.pad.arrows.len();
        let actions = link.map_or_else(
            || vec![LaneAction::None; columns],
            |link| actions(state, link, columns),
        );
        let is_release = link.is_some_and(|link| link.is_release());
        let p = previous.as_deref();
        let mut cost = p.map_or_else(Cost::default, |p| p.cost.clone());
        cost.fallback += fallback;
        cost.random = random;
        let mut result = SearchNode {
            node,
            link,
            instance: None,
            depth: p.map_or(0, |p| p.depth + 1),
            time,
            actions,
            cost,
            total_steps: root
                .map(|r| r.total_steps)
                .unwrap_or_else(|| p.map_or(0, |p| p.total_steps))
                + i64::from(!is_release),
            total_in_pattern: p.map_or(0, |p| p.total_in_pattern) + i64::from(!is_release),
            total_inward: p.map_or(0, |p| p.total_inward),
            total_outward: p.map_or(0, |p| p.total_outward),
            same_steps: p.map_or(0, |p| p.same_steps),
            new_steps: p.map_or(0, |p| p.new_steps),
            same_since_new: p.map_or([0; 2], |p| p.same_since_new),
            seen_new: p.map_or([false; 2], |p| p.seen_new),
            prefix_same: root
                .map(|r| r.previous_same_counts)
                .unwrap_or_else(|| p.map_or([0; 2], |p| p.prefix_same)),
            last_transition: root
                .and_then(|r| r.last_transition)
                .or_else(|| p.and_then(|p| p.last_transition)),
            last_step_time: root
                .map(|r| r.previous_times)
                .unwrap_or_else(|| p.map_or([0.0; 2], |p| p.last_step_time)),
            last_release_time: root
                .map(|r| r.previous_times)
                .unwrap_or_else(|| p.map_or([0.0; 2], |p| p.last_release_time)),
            last_arrows: p.map_or(
                state.feet.map(|foot| foot.map(|portion| portion.lane)),
                |p| p.last_arrows,
            ),
            counts: root
                .map(|r| r.lane_counts.clone())
                .unwrap_or_else(|| p.map_or_else(|| vec![0; columns], |p| p.counts.clone())),
            body_x: 0.0,
            movement: 0,
            movement_start_time: 0.0,
            movement_start_x: 0.0,
            movement_steps: 0,
            previous: previous.clone(),
        };
        for (lane, action) in result.actions.iter().enumerate() {
            if action.is_step() {
                result.counts[lane] += 1;
            }
        }
        self.facing(&mut result, state, is_release);
        self.transition(&mut result, state);
        let is_step = self.travel(&mut result, state);
        let sum: u64 = result.counts.iter().sum();
        result.cost.distribution = if sum == 0 {
            0.0
        } else {
            result.counts.iter().zip(self.normalized_weights).fold(
                0.0f32,
                |total, (&count, &weight)| {
                    total + (count as f32 / sum as f32 - weight as f32).abs()
                },
            ) / columns as f32
        };
        result.cost.stretch += self.stretch(state);
        self.lateral(&mut result, is_step);
        self.pattern_cost(&mut result);
        let (ambiguous, misleading) = self.ambiguity(&result, state);
        result.cost.ambiguous += u64::from(ambiguous);
        result.cost.misleading += u64::from(misleading);
        Arc::new(result)
    }

    fn facing(&self, result: &mut SearchNode, state: NodeState, is_release: bool) {
        let config = &self.config.facing;
        result.total_inward += u64::from(
            !is_release
                && geometry::facing_inward(self.pad, state, config.inward_percentage_cutoff),
        );
        result.total_outward += u64::from(
            !is_release
                && geometry::facing_outward(self.pad, state, config.outward_percentage_cutoff),
        );
        if config.max_inward_percentage < 1.0
            && result.total_inward as f64 / result.total_in_pattern as f64
                > config.max_inward_percentage
        {
            result.cost.facing += 1.0;
        }
        if config.max_outward_percentage < 1.0
            && result.total_outward as f64 / result.total_in_pattern as f64
                > config.max_outward_percentage
        {
            result.cost.facing += 1.0;
        }
    }

    fn transition(&self, result: &mut SearchNode, state: NodeState) {
        let config = &self.config.transitions;
        if config.enabled != Some(true) || geometry::width(self.pad) < config.minimum_pad_width {
            return;
        }
        let left = geometry::side(self.pad, state, config.transition_cutoff_percentage)
            .map(|(left, _)| left);
        if left.is_some() && result.last_transition.is_none() {
            result.last_transition = Some((left, result.total_steps));
            return;
        }
        let elapsed = result.total_steps - result.last_transition.map_or(0, |(_, steps)| steps);
        if left.is_some()
            && result
                .last_transition
                .is_some_and(|(previous, _)| previous.is_some() && previous != left)
        {
            if elapsed < i64::from(config.steps_per_transition_min) {
                result.cost.early_transition +=
                    i64::from(config.steps_per_transition_min) - elapsed;
            }
            result.last_transition = Some((left, result.total_steps));
        } else if elapsed > i64::from(config.steps_per_transition_max) {
            result.cost.late_transition += elapsed - i64::from(config.steps_per_transition_max);
        }
    }

    fn travel(&self, result: &mut SearchNode, state: NodeState) -> bool {
        if result.previous.is_none() {
            return false;
        }
        let config = &self.config.step_tightening;
        let link = result.link.unwrap();
        let mut total_distance = 0.0;
        let mut is_step = false;
        for foot in 0..2 {
            for portion in &link.feet[foot] {
                if portion.valid && matches!(portion.action, FootAction::Release | FootAction::Tap)
                {
                    result.last_release_time[foot] = result.time;
                }
            }
            let stepped = link.involves_step(foot);
            is_step |= stepped;
            if stepped {
                let dt = result.time - result.last_step_time[foot];
                let a = geometry::foot_position(self.pad, result.last_arrows[foot]);
                let b =
                    geometry::foot_position(self.pad, state.feet[foot].map(|portion| portion.lane));
                let distance = compensated_distance(config, a, b);
                total_distance += distance;
                if config.speed_tightening_enabled == Some(true)
                    && distance >= config.speed_tightening_min_distance
                {
                    let penalty = if config.speed_min_time_seconds < config.speed_max_time_seconds {
                        (1.0 - (dt - config.speed_min_time_seconds)
                            / (config.speed_max_time_seconds - config.speed_min_time_seconds))
                            .clamp(0.0, 1.0)
                    } else {
                        if dt < config.speed_min_time_seconds {
                            1.0
                        } else {
                            0.0
                        }
                    };
                    result.cost.speed += penalty * distance;
                }
                result.last_step_time[foot] = result.time;
                for portion in 0..2 {
                    result.last_arrows[foot][portion] = if link.feet[foot][portion].valid
                        && link.feet[foot][portion].action != FootAction::Release
                    {
                        state.feet[foot][portion].lane
                    } else {
                        None
                    };
                }
            }
        }
        if config.distance_tightening_enabled == Some(true) {
            let range = config.distance_max - config.distance_min;
            result.cost.distance += if range > 0.0 {
                ((total_distance - config.distance_min) / range).clamp(0.0, 1.0)
            } else {
                1.0
            };
        }
        is_step
    }

    fn stretch(&self, state: NodeState) -> f64 {
        let config = &self.config.step_tightening;
        if config.stretch_tightening_enabled != Some(true) {
            return 0.0;
        }
        let a = geometry::foot_position(self.pad, state.feet[0].map(|portion| portion.lane));
        let b = geometry::foot_position(self.pad, state.feet[1].map(|portion| portion.lane));
        let distance = compensated_distance(config, a, b);
        if distance < config.stretch_distance_min {
            return 0.0;
        }
        let range = config.stretch_distance_max - config.stretch_distance_min;
        if range > 0.0 {
            ((distance - config.stretch_distance_min) / range).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    fn lateral(&self, result: &mut SearchNode, is_step: bool) {
        let mut points = 0;
        let mut x = 0.0f32;
        for lane in result.last_arrows.iter().flatten().flatten() {
            points += 1;
            x += self.pad.arrows[usize::from(*lane)].x as f32;
        }
        result.body_x = if points > 0 { x / points as f32 } else { x };
        if let Some(previous) = result.previous.as_deref() {
            result.movement_start_time = previous.movement_start_time;
            result.movement_start_x = previous.movement_start_x;
            if !is_step {
                result.movement = previous.movement;
                result.movement_steps = previous.movement_steps;
            } else {
                let delta = result.body_x - previous.body_x;
                if delta.abs() > 0.0001 {
                    result.movement = if delta < 0.0 { -1 } else { 1 };
                    if result.movement != previous.movement {
                        result.movement_start_time = result.time;
                        result.movement_start_x = result.body_x;
                        result.movement_steps = 1;
                    } else {
                        result.movement_steps = previous.movement_steps + 1;
                    }
                }
            }
        }
        let config = &self.config.lateral_tightening;
        if config.enabled == Some(true) {
            let dt = result.time - result.movement_start_time;
            if result.movement != 0 && dt > 0.0 {
                let nps = (result.movement_steps - 1) as f64 / dt;
                let speed = f64::from((result.body_x - result.movement_start_x).abs()) / dt;
                if ((self.average_nps > 0.0 && nps > self.average_nps * config.relative_nps)
                    || nps > config.absolute_nps)
                    && speed > config.speed
                {
                    result.cost.lateral += speed - config.speed;
                }
            }
        }
    }

    fn pattern_cost(&self, result: &mut SearchNode) {
        if let Some((step, foot)) = result.link.and_then(|link| link.single_step()) {
            match step {
                StepType::NewArrow => {
                    result.new_steps += 1;
                    result.same_since_new[foot] = 0;
                    result.seen_new[foot] = true;
                }
                StepType::SameArrow => {
                    result.same_steps += 1;
                    result.same_since_new[foot] += 1;
                    if let Some(config) = self.pattern {
                        if config.limit_same_arrows_in_a_row_per_foot
                            && config.max_same_arrows_in_a_row_per_foot > 0
                        {
                            let consecutive = result.same_since_new[foot]
                                + 1
                                + if result.seen_new[foot] {
                                    0
                                } else {
                                    result.prefix_same[foot]
                                };
                            if consecutive > config.max_same_arrows_in_a_row_per_foot as u64 {
                                result.cost.same_run_over_max += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let Some(config) = self.pattern else {
            result.cost.pattern_type = 0.0;
            return;
        };
        if config.step_type_check_period > 1
            && result.depth > 0
            && result.depth % config.step_type_check_period as usize != 0
        {
            return;
        }
        result.cost.pattern_type = result.final_pattern_cost(config);
    }

    fn ambiguity(&self, result: &SearchNode, state: NodeState) -> (bool, bool) {
        let Some(link) = result.link else {
            return (false, false);
        };
        let Some(previous) = result.previous.as_deref() else {
            return (false, false);
        };
        if link.is_blank() {
            return (false, false);
        }
        let mut jump = true;
        let mut same = false;
        let mut new = false;
        for foot in 0..2 {
            let action = link.feet[foot][0];
            if action.action == FootAction::Release || link.feet[foot][1].valid {
                return (false, false);
            }
            if !action.valid {
                jump = false;
            } else {
                if !matches!(action.step, StepType::NewArrow | StepType::SameArrow) {
                    return (false, false);
                }
                same |= action.step == StepType::SameArrow;
                new |= action.step == StepType::NewArrow;
            }
        }
        if same {
            if !jump {
                return (false, false);
            }
            let matches = same_actions(&result.actions, &previous.actions);
            return (
                false,
                matches && (new || previous.link.is_some_and(|link| link.is_bracket_step())),
            );
        }
        if !jump {
            if !geometry::same_f64(previous.last_release_time[0], previous.last_release_time[1]) {
                return (false, false);
            }
            let Some(lane) = result
                .actions
                .iter()
                .position(|&action| matches!(action, LaneAction::Tap | LaneAction::Hold))
            else {
                return (false, false);
            };
            let arrow = &self.pad.arrows[lane];
            let bracketable = [0, 1].map(|foot| {
                previous.last_arrows[foot].iter().flatten().any(|&from| {
                    arrow.bracketable_pairings_heel[foot][usize::from(from)]
                        || arrow.bracketable_pairings_toe[foot][usize::from(from)]
                })
            });
            if bracketable[0] != bracketable[1] {
                return (false, false);
            }
        }
        let mut other = BLANK_LINK;
        other.feet[0][0] = link.feet[1][0];
        other.feet[1][0] = link.feet[0][0];
        let ambiguous = self.sibling_matches(previous.node, state, other, &result.actions);
        let mut misleading = false;
        if jump {
            for steps in [
                [StepType::SameArrow, StepType::NewArrow],
                [StepType::NewArrow, StepType::SameArrow],
                [StepType::SameArrow; 2],
            ] {
                let mut sibling = BLANK_LINK;
                for foot in 0..2 {
                    sibling.feet[foot][0] = PortionAction {
                        valid: true,
                        step: steps[foot],
                        action: FootAction::Tap,
                    };
                }
                if self.sibling_matches(previous.node, state, sibling, &result.actions) {
                    misleading = true;
                    break;
                }
            }
        }
        (ambiguous, misleading)
    }

    fn sibling_matches(
        &self,
        parent: usize,
        state: NodeState,
        link: GraphLink,
        expected: &[LaneAction],
    ) -> bool {
        self.graph.edge_for_link(parent, &link).is_some_and(|edge| {
            self.graph.targets[edge.targets.clone()]
                .iter()
                .any(|&target| {
                    let other = self.graph.nodes[target].state;
                    !other.same_footing(state)
                        && same_actions(expected, &actions(other, link, self.pad.arrows.len()))
                })
        })
    }
}

impl SearchNode {
    pub fn final_pattern_cost(&self, config: &PatternConfig) -> f32 {
        let total = self.same_steps + self.new_steps;
        if total == 0 {
            return 0.0;
        }
        let desired = config.normalized_step_weights();
        (self.same_steps as f32 / total as f32 - desired[0] as f32).abs()
            + (self.new_steps as f32 / total as f32 - desired[1] as f32).abs()
    }
}

fn same_actions(a: &[LaneAction], b: &[LaneAction]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&a, &b)| a.is_step() == b.is_step())
}

fn compensated_distance(config: &StepConfig, a: (f32, f32, bool), b: (f32, f32, bool)) -> f64 {
    let count = f64::from(u8::from(!a.2) + u8::from(!b.2));
    let dx = (f64::from(b.0) - f64::from(a.0)).abs();
    let dy = (f64::from(b.1) - f64::from(a.1)).abs();
    let dx = if dx == 0.0 {
        0.0
    } else {
        (dx - count * (0.5 - config.lateral_min_panel_distance)).max(0.0)
    };
    let dy = if dy == 0.0 {
        0.0
    } else {
        (dy - count * (0.5 - config.longitudinal_min_panel_distance)).max(0.0)
    };
    (dx * dx + dy * dy).sqrt()
}
