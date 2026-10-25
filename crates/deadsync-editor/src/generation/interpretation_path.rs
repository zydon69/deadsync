//! ExpressedChart.ChartSearchNode history, comparison and pruning (MIT,
//! StepManiaLibrary bdbc427f). Domain move costs are computed separately.
use super::{
    expressed::LinkInstance,
    graph::{FootAction, GraphLink, StepGraph},
};
use crate::Error;
use std::{cmp::Ordering, collections::HashMap, sync::Arc};

pub struct PathNode {
    pub node: usize,
    pub row: u64,
    pub time: f64,
    pub previous: Option<Arc<Self>>,
    pub instance: Option<LinkInstance>,
    pub last_foot: Vec<Option<usize>>,
    pub cost: i32,
    pub total_cost: i32,
    pub total_orientation_cost: i32,
}
impl PathNode {
    pub fn root(graph: &StepGraph, node: usize) -> Result<Arc<Self>, Error> {
        if node >= graph.nodes.len() {
            return invalid("Unknown interpretation root");
        }
        Ok(Arc::new(Self {
            node,
            row: 0,
            time: 0.0,
            previous: None,
            instance: None,
            last_foot: vec![None; graph.columns()],
            cost: 0,
            total_cost: 0,
            total_orientation_cost: 0,
        }))
    }

    pub fn child(
        graph: &StepGraph,
        parent: &Arc<Self>,
        node: usize,
        row: u64,
        time: f64,
        instance: LinkInstance,
        cost: i32,
        orientation_cost: i32,
    ) -> Result<Arc<Self>, Error> {
        let state = graph
            .nodes
            .get(node)
            .ok_or_else(|| Error::InvalidEdit("Unknown interpretation destination".into()))?
            .state;
        if !time.is_finite()
            || row > i32::MAX as u64
            || row < parent.row
            || parent.last_foot.len() != graph.columns()
        {
            return invalid("Invalid interpretation position or context");
        }
        let mut last_foot = parent.last_foot.clone();
        for (foot, portions) in instance.link.feet.iter().enumerate() {
            for (portion, action) in portions.iter().enumerate() {
                if action.valid && matches!(action.action, FootAction::Tap | FootAction::Hold) {
                    let lane = state.feet[foot][portion].lane.ok_or_else(|| {
                        Error::InvalidEdit("Action on an invalid foot portion".into())
                    })?;
                    last_foot[usize::from(lane)] = Some(foot);
                }
            }
        }
        Ok(Arc::new(Self {
            node,
            row,
            time,
            previous: Some(Arc::clone(parent)),
            instance: Some(instance),
            last_foot,
            cost,
            total_cost: parent.total_cost.wrapping_add(cost),
            total_orientation_cost: parent.total_orientation_cost.wrapping_add(orientation_cost),
        }))
    }

    /// Includes this node's link, skipping release links, as the source does.
    pub fn previous_step_link(&self, mut nth: usize) -> Option<GraphLink> {
        let mut current = Some(self);
        while let Some(node) = current.filter(|node| node.previous.is_some()) {
            if let Some(instance) = node.instance {
                if !instance.link.is_release() {
                    nth = nth.saturating_sub(1);
                    if nth == 0 {
                        return Some(instance.link);
                    }
                }
            }
            current = node.previous.as_deref();
        }
        None
    }

    /// Excludes this node, unlike previous_step_link. Root is never a step.
    pub fn previous_step_node(&self, mut nth: usize) -> Option<&Self> {
        let mut current = self.previous.as_deref();
        while let Some(node) = current.filter(|node| node.previous.is_some()) {
            if node
                .instance
                .is_some_and(|instance| !instance.link.is_release())
            {
                nth = nth.saturating_sub(1);
                if nth == 0 {
                    return Some(node);
                }
            }
            current = node.previous.as_deref();
        }
        None
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        if std::ptr::eq(self, other) {
            return Ordering::Equal;
        }
        let total = self.total_cost.cmp(&other.total_cost);
        if total != Ordering::Equal {
            return total;
        }
        let orientation = self
            .total_orientation_cost
            .cmp(&other.total_orientation_cost);
        if orientation != Ordering::Equal {
            return orientation;
        }
        // On equal totals, prefer the lower move cost nearest the shared root.
        let (mut left, mut right) = (Some(self), Some(other));
        let mut earlier = Ordering::Equal;
        while let (Some(a), Some(b)) = (left, right) {
            if std::ptr::eq(a, b) {
                break;
            }
            if a.cost != b.cost {
                earlier = a.cost.cmp(&b.cost);
            }
            left = a.previous.as_deref();
            right = b.previous.as_deref();
        }
        if earlier != Ordering::Equal {
            return earlier;
        }
        // Last resort: link validity and serialized StepType, newest first.
        // Action and instance type intentionally do not enter this comparison.
        let (mut left, mut right) = (Some(self), Some(other));
        while let (Some(a), Some(b)) = (left, right) {
            let (Some(a_link), Some(b_link)) = (a.instance, b.instance) else {
                break;
            };
            for (a, b) in a_link
                .link
                .feet
                .iter()
                .flatten()
                .zip(b_link.link.feet.iter().flatten())
            {
                if a.valid != b.valid {
                    return if a.valid {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
                if a.valid && a.step != b.step {
                    return (a.step as u8).cmp(&(b.step as u8));
                }
            }
            left = a.previous.as_deref();
            right = b.previous.as_deref();
        }
        Ordering::Equal
    }
}

// Branch histories may contain hundreds of thousands of steps. Dropping a
// unique prefix iteratively avoids recursive Arc destruction on the UI stack.
impl Drop for PathNode {
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

/// Keep the first destination's insertion slot. Equal paths keep the earlier
/// candidate, matching the source's strict comparison and dictionary order.
pub fn prune(nodes: impl IntoIterator<Item = Arc<PathNode>>) -> Vec<Arc<PathNode>> {
    let mut slots: HashMap<usize, usize> = HashMap::new();
    let mut best: Vec<Arc<PathNode>> = Vec::new();
    for node in nodes {
        if let Some(&slot) = slots.get(&node.node) {
            if node.compare(&best[slot]) == Ordering::Less {
                best[slot] = node;
            }
        } else {
            slots.insert(node.node, best.len());
            best.push(node);
        }
    }
    best
}

pub fn best(nodes: &[Arc<PathNode>]) -> Option<&Arc<PathNode>> {
    let mut selected: Option<&Arc<PathNode>> = None;
    for node in nodes {
        if selected.is_none_or(|previous| node.compare(previous) == Ordering::Less) {
            selected = Some(node);
        }
    }
    selected
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
