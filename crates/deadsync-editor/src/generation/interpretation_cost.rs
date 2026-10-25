//! ExpressedChart move costs from ExpressedChart.cs (MIT, bdbc427f).
//! The source's decision order is retained, including its bracket heuristics.
use super::{
    config::BracketMethod,
    expressed::{Frame, SearchState},
    graph::{ArrowState, GraphLink, NodeState, Orientation, StepGraph},
    interpretation_path::PathNode,
    step_data::StepType,
};
use crate::{Error, pad::Pad};

#[derive(Default)]
struct OneArrowInfo {
    any_held: bool,
    all_held: bool,
    can_step: bool,
    can_normal: bool,
    can_bracket: bool,
    can_cross: bool,
    mine: Option<u64>,
    release: u64,
    arrows: [Option<u16>; 2],
    bracket: bool,
    lifted: bool,
}
fn one_arrow_info(
    foot: usize,
    lane: usize,
    frame: &Frame,
    pad: &Pad,
    state: NodeState,
) -> OneArrowInfo {
    let mut info = OneArrowInfo {
        all_held: true,
        bracket: true,
        ..OneArrowInfo::default()
    };
    let other = 1 - foot;
    for portion in 0..2 {
        let own = state.feet[foot][portion];
        let opposite = state.feet[other][portion];
        info.arrows[portion] = own.lane;
        if opposite.lane.map(usize::from) == Some(lane) {
            info.lifted = own.state == ArrowState::Lifted;
        }
        if let Some(previous) = opposite.lane.map(usize::from) {
            let arrow = &pad.arrows[previous];
            info.can_cross |= arrow.other_foot_pairings_crossover_behind[other][lane]
                || arrow.other_foot_pairings_crossover_front[other][lane];
            info.can_normal |= arrow.other_foot_pairings[other][lane];
        }
        if let Some(previous) = own.lane.map(usize::from) {
            let arrow = &pad.arrows[previous];
            let bracket = arrow.bracketable_pairings_heel[foot][lane]
                || arrow.bracketable_pairings_toe[foot][lane];
            if own.state == ArrowState::Held {
                info.any_held = true;
                info.can_bracket = bracket;
            } else {
                info.all_held = false;
                if !info.any_held {
                    info.can_bracket = bracket;
                }
                if frame.last_mines[previous] > frame.last_releases[previous] {
                    info.mine = Some(frame.last_mines[previous]);
                }
                info.release = info.release.max(frame.last_releases[previous]);
            }
        } else {
            info.bracket = false;
            info.all_held = false;
        }
    }
    if info.any_held {
        info.can_cross = false;
    }
    if info.all_held {
        info.can_normal = false;
        info.can_bracket = false;
    }
    info.can_step = info.can_cross || info.can_normal;
    info
}
#[derive(Default)]
struct TwoArrowInfo {
    could_bracket: bool,
    any_held: bool,
    all_held: bool,
    moves_off: bool,
    new_arrows: usize,
    bracket_distance: bool,
    swap: bool,
    cross: bool,
    invert: bool,
}
fn two_arrow_info(
    node: &PathNode,
    foot: usize,
    frame: &Frame,
    pad: &Pad,
    graph: &StepGraph,
) -> TwoArrowInfo {
    let mut info = TwoArrowInfo::default();
    let parent = node.previous.as_deref();
    if let Some(parent) = parent {
        let previous = graph.nodes[parent.node].state;
        let current = graph.nodes[node.node].state;
        let release = previous.feet[foot]
            .iter()
            .filter(|p| p.state != ArrowState::Held)
            .filter_map(|p| p.lane.map(|lane| frame.last_releases[usize::from(lane)]))
            .max()
            .unwrap_or(0);
        for (lane, state) in frame.state.iter().enumerate() {
            if state.is_step()
                && !previous.feet[foot]
                    .iter()
                    .any(|p| p.lane.map(usize::from) == Some(lane))
            {
                info.new_arrows += 1;
            }
        }
        info.all_held = true;
        for portion in previous.feet[foot] {
            if let Some(lane) = portion.lane.map(usize::from) {
                let held = portion.state == ArrowState::Held
                    || (node.row == release && parent.last_foot[lane] == Some(foot));
                info.any_held |= held;
                info.all_held &= held;
                if portion.state != ArrowState::Held && frame.state[lane] == SearchState::Empty {
                    info.moves_off = true;
                }
                if let Some(new) = current.feet[foot]
                    .iter()
                    .find_map(|p| p.lane.map(usize::from))
                {
                    let arrow = &pad.arrows[lane];
                    info.bracket_distance |= arrow.bracketable_pairings_heel[foot][new]
                        || arrow.bracketable_pairings_toe[foot][new];
                }
            } else {
                info.all_held = false;
            }
        }
    }
    if !info.any_held {
        info.could_bracket = true;
        let mut previous_step = None;
        let mut count = 0;
        for (lane, _) in frame
            .state
            .iter()
            .enumerate()
            .filter(|(_, state)| state.is_step())
        {
            count += 1;
            if let Some(previous) = previous_step {
                let a: &crate::pad::Arrow = &pad.arrows[lane];
                let b: &crate::pad::Arrow = &pad.arrows[previous];
                info.could_bracket &= (a.bracketable_pairings_heel[foot][previous]
                    && b.bracketable_pairings_toe[foot][lane])
                    || (a.bracketable_pairings_toe[foot][previous]
                        && b.bracketable_pairings_heel[foot][lane]);
            }
            if let Some(parent) = parent {
                for opposite in graph.nodes[parent.node].state.feet[1 - foot] {
                    if let Some(other) = opposite.lane.map(usize::from) {
                        info.swap |= other == lane;
                        // The pinned reference checks Behind twice here, not Front.
                        info.cross |=
                            pad.arrows[other].other_foot_pairings_crossover_behind[1 - foot][lane];
                        info.invert |=
                            pad.arrows[other].other_foot_pairings_inverted[1 - foot][lane];
                    }
                }
            }
            previous_step = Some(lane);
        }
        info.could_bracket &= count == 2;
    }
    info
}
fn crossover(pad: &Pad, node: NodeState) -> bool {
    for left in node.feet[0].iter().filter_map(|p| p.lane.map(usize::from)) {
        for right in node.feet[1].iter().filter_map(|p| p.lane.map(usize::from)) {
            let a = &pad.arrows[left];
            if a.other_foot_pairings_crossover_front[0][right]
                || a.other_foot_pairings_crossover_behind[0][right]
                || a.other_foot_pairings_crossover_front_stretch[0][right]
                || a.other_foot_pairings_crossover_behind_stretch[0][right]
            {
                return true;
            }
        }
    }
    false
}
fn step_node(mut node: Option<&PathNode>, mut nth: usize) -> Option<&PathNode> {
    while let Some(current) = node.filter(|n| n.previous.is_some()) {
        if current.instance.is_some_and(|i| !i.link.is_release()) {
            nth -= 1;
            if nth == 0 {
                return Some(current);
            }
        }
        node = current.previous.as_deref();
    }
    None
}
fn from_jump(this: &OneArrowInfo, other: &OneArrowInfo) -> i32 {
    if !other.can_step || other.can_cross {
        return 1;
    }
    if this.can_cross {
        return 10;
    }
    match (this.mine, other.mine) {
        (None, Some(_)) => return 9,
        (Some(a), Some(b)) if b > a => return 4,
        (Some(a), Some(b)) if a > b => return 8,
        (Some(_), None) => return 2,
        _ => {}
    }
    if other.release > this.release {
        return 2;
    }
    if this.release > other.release {
        return 8;
    }
    if other.can_bracket && !this.can_bracket {
        return 5;
    }
    if !other.can_bracket && this.can_bracket {
        return 3;
    }
    2
}

pub struct Scoring<'a> {
    pub graph: &'a StepGraph,
    pub pad: &'a Pad,
    pub method: BracketMethod,
}
impl Scoring<'_> {
    pub fn orientation(&self, node: &PathNode) -> i32 {
        let state = self.graph.nodes[node.node].state;
        if state.orientation != Orientation::Normal {
            2
        } else if crossover(self.pad, state) {
            1
        } else {
            0
        }
    }
    pub fn cost(&self, node: &PathNode, frame: &Frame) -> Result<i32, Error> {
        if frame.state.len() != self.graph.columns()
            || frame.last_mines.len() != frame.state.len()
            || frame.last_releases.len() != frame.state.len()
            || self.pad.arrows.len() != frame.state.len()
            || node.node >= self.graph.nodes.len()
        {
            return Err(Error::InvalidEdit(
                "Invalid interpretation scoring context".into(),
            ));
        }
        let instance = node
            .instance
            .ok_or_else(|| Error::InvalidEdit("Root has no movement to score".into()))?;
        let link = instance.link;
        if link.is_blank() {
            return Err(Error::InvalidEdit(
                "Blank movement cannot express a chart step".into(),
            ));
        }
        if link.is_release() {
            return Ok(0);
        }
        let Some(parent) = node.previous.as_deref() else {
            return Err(Error::InvalidEdit("Movement has no parent".into()));
        };
        let count = frame.state.iter().filter(|s| s.is_step()).count();
        let previous = parent.previous_step_link(1);
        let previous_previous = parent.previous_step_link(2);
        Ok(match count {
            1 => self.single(node, frame, link, parent, previous, previous_previous),
            2 => self.pair(node, frame, link, previous),
            3 | 4 => {
                let mut cost = 0;
                for foot in link.feet {
                    if let Some(action) = foot.iter().find(|a| a.valid) {
                        let data = action.step.data();
                        cost += 16 * i32::from(data.invert)
                            + 8 * i32::from(data.crossover)
                            + 4 * i32::from(data.foot_swap.iter().any(|&v| v))
                            + i32::from(data.possible_new_arrows);
                    }
                }
                cost
            }
            _ => 1000,
        })
    }
    fn single(
        &self,
        node: &PathNode,
        frame: &Frame,
        link: GraphLink,
        parent: &PathNode,
        previous: Option<GraphLink>,
        previous_previous: Option<GraphLink>,
    ) -> i32 {
        use StepType::*;
        let lane = frame.state.iter().rposition(|s| s.is_step()).unwrap();
        // Source GetSingleStepStepAndFoot takes the last valid portion.
        let (foot, action) = link
            .feet
            .iter()
            .enumerate()
            .flat_map(|(f, ps)| ps.iter().filter(|p| p.valid).map(move |p| (f, p)))
            .last()
            .unwrap();
        let step = action.step;
        let other_foot = 1 - foot;
        let state = self.graph.nodes[parent.node].state;
        let mut this = one_arrow_info(foot, lane, frame, self.pad, state);
        let mut other = one_arrow_info(other_foot, lane, frame, self.pad, state);
        let double = previous.is_some_and(|l| l.is_step_with_foot(foot)) && !other.any_held;
        let released_same = other.release == node.row;
        let released_after =
            released_same || (other.release < node.row && other.release > this.release);
        let triple = double && previous_previous.is_some_and(|l| l.is_step_with_foot(foot));
        let most_recent_release = this.release.max(other.release);
        this.any_held |= node.row > 0 && this.release == node.row;
        other.any_held |= node.row > 0 && other.release == node.row;
        let bracket = step.data().one_arrow_bracket;
        if self.method == BracketMethod::NoBrackets && bracket {
            return 1000;
        }
        match step {
            Swing => 800,
            SameArrow | BracketOneArrowHeelSame | BracketOneArrowToeSame => {
                if previous
                    .and_then(|p| p.foot_swap())
                    .is_some_and(|(f, _)| f == foot)
                {
                    20
                } else if this.any_held && !other.any_held && other.can_step {
                    7
                } else {
                    0
                }
            }
            NewArrow
            | NewArrowStretch
            | BracketOneArrowHeelNew
            | BracketOneArrowToeNew
            | BracketStretchOneArrowHeelNew
            | BracketStretchOneArrowToeNew => {
                if other.any_held {
                    if other.all_held {
                        return if this.can_bracket { 1 } else { 2 };
                    }
                    if this.any_held {
                        if other.can_bracket {
                            if previous.is_some_and(|l| l.is_step_with_foot(other_foot)) {
                                return 5;
                            }
                            if previous.is_some_and(|l| l.is_jump()) {
                                return from_jump(&this, &other);
                            }
                            return 6;
                        }
                        return 4;
                    }
                    return if previous.is_some_and(|l| l.is_step_with_foot(other_foot)) {
                        0
                    } else {
                        3
                    };
                }
                if this.any_held {
                    if !this.can_bracket {
                        return 160;
                    }
                    return match (other.can_step, double) {
                        (true, true) => 51,
                        (true, false) => 31,
                        (false, true) => 10,
                        (false, false) => 9,
                    };
                }
                if double {
                    if released_same {
                        return 3;
                    }
                    if released_after {
                        return 5;
                    }
                    if triple {
                        return 200;
                    }
                    return if this.mine.is_some() { 50 } else { 100 };
                }
                let Some(previous) = previous else {
                    return if other.can_bracket && !this.can_bracket {
                        3
                    } else if !other.can_bracket && this.can_bracket {
                        1
                    } else if !other.can_step {
                        0
                    } else {
                        2
                    };
                };
                if previous.is_step_with_foot(other_foot) {
                    0
                } else if previous.is_jump() {
                    from_jump(&this, &other)
                } else {
                    1000
                }
            }
            CrossoverFront
            | CrossoverBehind
            | CrossoverFrontStretch
            | CrossoverBehindStretch
            | BracketCrossoverFrontOneArrowHeelNew
            | BracketCrossoverFrontOneArrowToeNew
            | BracketCrossoverBehindOneArrowHeelNew
            | BracketCrossoverBehindOneArrowToeNew => {
                // after jump, other held, indicated double, double, ordinary, bracket posture
                let costs = if bracket {
                    [50, 33, 130, 230, 34, 36]
                } else if matches!(step, CrossoverFrontStretch | CrossoverBehindStretch) {
                    [25, 5, 120, 220, 6, 8]
                } else {
                    [20, 4, 100, 200, 5, 7]
                };
                if previous.is_some_and(|l| l.is_jump()) && !other.any_held {
                    costs[0]
                } else if other.any_held {
                    costs[1]
                } else if double {
                    costs[if this.mine.is_some() { 2 } else { 3 }]
                } else if other.bracket {
                    costs[5]
                } else {
                    costs[4]
                }
            }
            FootSwap
            | BracketOneArrowHeelSwap
            | BracketOneArrowToeSwap
            | FootSwapCrossoverFront
            | FootSwapCrossoverBehind
            | FootSwapInvertFront
            | FootSwapInvertBehind => {
                if matches!(step, FootSwapInvertFront | FootSwapInvertBehind) {
                    return 250;
                }
                if matches!(step, FootSwapCrossoverFront | FootSwapCrossoverBehind) {
                    return 200;
                }
                if double {
                    return if this.mine.is_some() { 75 } else { 150 };
                }
                if other.any_held && !this.any_held {
                    return 13;
                }
                let free_mine = frame.last_mines.iter().enumerate().any(|(lane, &mine)| {
                    !other.arrows.contains(&Some(lane as u16)) && mine >= most_recent_release
                });
                if other.bracket
                    && !this.any_held
                    && this.mine.is_none()
                    && !free_mine
                    && self.method != BracketMethod::Aggressive
                {
                    return 13;
                }
                let previous_node = step_node(Some(parent), 1);
                let earlier_node = step_node(Some(parent), 2);
                let same = previous_node.is_some_and(|p| {
                    self.graph.nodes[p.node].state.feet[foot]
                        .iter()
                        .any(|p| p.lane.map(usize::from) == Some(lane))
                });
                if let (Some(a), Some(b), Some(a_link), Some(b_link)) =
                    (previous_node, earlier_node, previous, previous_previous)
                {
                    let has = |n: &PathNode| {
                        self.graph.nodes[n.node].state.feet[other_foot]
                            .iter()
                            .any(|p| {
                                p.lane.map(usize::from) == Some(lane)
                                    && p.state != ArrowState::Lifted
                            })
                    };
                    if has(a)
                        && has(b)
                        && a_link.single_step().is_some_and(|(_, f)| f == other_foot)
                        && b_link.single_step().is_some_and(|(_, f)| f == other_foot)
                    {
                        return 20;
                    }
                }
                if this.mine.is_some() {
                    return 1;
                }
                if free_mine {
                    return 2;
                }
                if other.lifted {
                    return 6;
                }
                if same && this.lifted {
                    return 0;
                }
                if this.can_bracket { 7 } else { 8 }
            }
            InvertFront
            | InvertBehind
            | InvertFrontStretch
            | InvertBehindStretch
            | BracketInvertFrontOneArrowHeelNew
            | BracketInvertFrontOneArrowToeNew
            | BracketInvertBehindOneArrowHeelNew
            | BracketInvertBehindOneArrowToeNew => {
                let costs = if bracket {
                    [600, 190, 400, 500, 200, 210]
                } else if matches!(step, InvertFrontStretch | InvertBehindStretch) {
                    [250, 6, 350, 450, 7, 9]
                } else {
                    [200, 5, 300, 400, 6, 8]
                };
                if previous.and_then(|l| l.foot_swap()).is_some() {
                    costs[0]
                } else if other.any_held {
                    costs[1]
                } else if double {
                    costs[if this.mine.is_some() { 2 } else { 3 }]
                } else if other.bracket {
                    costs[5]
                } else {
                    costs[4]
                }
            }
            _ => 1000,
        }
    }
    fn pair(
        &self,
        node: &PathNode,
        frame: &Frame,
        link: GraphLink,
        previous: Option<GraphLink>,
    ) -> i32 {
        use StepType::*;
        let info = [
            two_arrow_info(node, 0, frame, self.pad, self.graph),
            two_arrow_info(node, 1, frame, self.pad, self.graph),
        ];
        let prefer: [bool; 2] = std::array::from_fn(|f| {
            previous.is_some_and(|l| l.is_step_with_foot(1 - f))
                && info[f].could_bracket
                && info[1 - f].moves_off
        });
        let brackets: Vec<_> = link
            .feet
            .iter()
            .enumerate()
            .filter(|(_, p)| p[0].valid && p[0].step.data().bracket)
            .collect();
        if brackets.len() == 1 {
            let (foot, portions) = brackets[0];
            let step = portions[0].step;
            if self.method == BracketMethod::NoBrackets {
                return 1000;
            }
            if step == BracketSwing {
                return 900;
            }
            if step.data().invert {
                return 50;
            }
            if step.data().crossover {
                return 40;
            }
            if info[1 - foot].all_held {
                return 1;
            }
            if previous.is_some_and(|l| l.is_step_with_foot(foot))
                && !info[1 - foot].any_held
                && step != BracketHeelSameToeSame
            {
                return 100;
            }
            let swap = step.data().foot_swap.iter().any(|&v| v);
            if step == BracketHeelSameToeSame {
                return 2;
            }
            if prefer[foot] {
                return if swap { 8 } else { 5 };
            }
            if swap {
                return match info[foot].new_arrows {
                    2 => 14,
                    1 => 12,
                    _ => 11,
                };
            }
            return 10;
        }
        if link.is_jump() {
            if self.method == BracketMethod::Aggressive
                && (0..2).any(|f| prefer[f] && !info[f].swap && !info[f].cross && !info[f].invert)
            {
                return 1000;
            }
            let holding: Vec<_> = (0..2)
                .filter(|&f| info[f].any_held && !info[f].all_held)
                .collect();
            if holding.len() == 1 {
                let other = 1 - holding[0];
                if info[other].could_bracket {
                    return if self.method == BracketMethod::Aggressive {
                        1000
                    } else {
                        20
                    };
                }
                return 15;
            }
            if prefer.iter().any(|&p| p) {
                return 7;
            }
            if self.graph.nodes[node.node].state.orientation != Orientation::Normal {
                return 8;
            }
            let mut both_new = true;
            let mut both_same = true;
            for action in link.feet.iter().flatten().filter(|a| a.valid) {
                if action.step == NewArrow {
                    both_same = false;
                }
                if action.step == SameArrow {
                    both_new = false;
                }
            }
            if !info[0].any_held
                && !info[1].any_held
                && crossover(self.pad, self.graph.nodes[node.node].state)
            {
                return 7;
            }
            if both_same {
                return 2;
            }
            if both_new {
                return match (info[0].bracket_distance, info[1].bracket_distance) {
                    (false, false) => 12,
                    (true, true) => 6,
                    _ => 10,
                };
            }
            return 5;
        }
        1000
    }
}
