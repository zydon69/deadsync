//! Generation foundation workflows using GrooveAuthor's original graph files.
//! These tests do not claim that chart/pattern generation is already ported.
use deadsync_editor::{
    generation::graph::{ArrowState, FootAction, FootState, StepGraph},
    pad::Pad,
};
use std::collections::BTreeSet;

#[test]
fn every_original_graph_loads_with_all_reference_nodes_reachable() {
    let reference_counts = [
        ("dance-single", 644),
        ("dance-double", 3436),
        ("dance-solo", 3136),
        ("dance-threepanel", 40),
        ("pump-single", 716),
        ("pump-halfdouble", 1940),
        ("pump-double", 4892),
        ("smx-beginner", 552),
        ("smx-single", 3048),
        ("smx-dual", 3496),
        ("smx-full", 12784),
    ];
    for (layout, expected) in reference_counts {
        let graph = StepGraph::builtin(layout).unwrap();
        assert_eq!(graph.nodes.len(), expected, "{layout}");
        let pad = Pad::builtin(layout).unwrap();
        assert_eq!(
            graph.nodes[0].state.feet[0][0].lane,
            Some(pad.starting_positions[0][0][0] as u16),
            "{layout}"
        );
        assert_eq!(
            graph.nodes[0].state.feet[1][0].lane,
            Some(pad.starting_positions[0][0][1] as u16),
            "{layout}"
        );
        let mut visited = BTreeSet::from([0]);
        let mut pending = vec![0];
        while let Some(node) = pending.pop() {
            for edge in &graph.edges[graph.nodes[node].edges.clone()] {
                for &target in &graph.targets[edge.targets.clone()] {
                    assert!(target < graph.nodes.len());
                    if visited.insert(target) {
                        pending.push(target);
                    }
                }
            }
        }
        assert_eq!(
            visited.len(),
            expected,
            "{layout}: missing reachable positions"
        );
    }
}

#[test]
fn original_graph_can_follow_taps_hold_release_and_brackets() {
    let graph = StepGraph::builtin("dance-single").unwrap();
    let root = graph
        .find_normal_stance(
            FootState {
                lane: Some(0),
                state: ArrowState::Resting,
            },
            FootState {
                lane: Some(3),
                state: ArrowState::Resting,
            },
        )
        .unwrap();
    let mut node = root;
    // Follow explicit left/right normal taps on the original graph. No random
    // lanes or fabricated adjacency are used to stand in for the reference.
    for (foot, lane) in [(0, 1), (1, 2), (0, 0), (1, 3)] {
        let next = graph.edges[graph.nodes[node].edges.clone()]
            .iter()
            .filter(|edge| edge.link.is_step_with_foot(foot))
            .flat_map(|edge| graph.targets[edge.targets.clone()].iter().copied())
            .find(|&target| {
                graph.nodes[target].state.feet[foot][0]
                    == FootState {
                        lane: Some(lane),
                        state: ArrowState::Resting,
                    }
                    && graph.nodes[target].state.feet[foot][1].lane.is_none()
                    && graph.nodes[target].state.feet[1 - foot]
                        == graph.nodes[node].state.feet[1 - foot]
            });
        node = next.expect("Reference normal tap transition must be available");
    }
    let held = graph.edges[graph.nodes[root].edges.clone()]
        .iter()
        .filter(|edge| {
            edge.link.is_step_with_foot(0)
                && edge.link.feet[0]
                    .iter()
                    .any(|portion| portion.valid && portion.action == FootAction::Hold)
        })
        .flat_map(|edge| graph.targets[edge.targets.clone()].iter().copied())
        .find(|&target| graph.nodes[target].state.feet[0][0].state == ArrowState::Held)
        .unwrap();
    assert!(
        graph.edges[graph.nodes[held].edges.clone()]
            .iter()
            .any(|edge| edge.link.is_release()
                && graph.targets[edge.targets.clone()]
                    .iter()
                    .any(
                        |&target| graph.nodes[target].state.feet[0][0].state == ArrowState::Resting
                    ))
    );
    assert!(
        StepGraph::builtin("pump-single")
            .unwrap()
            .edges
            .iter()
            .any(|edge| edge.link.is_bracket_step() && edge.link.involves_bracket(false))
    );
}

#[test]
fn corrupt_graph_inputs_fail_without_installing_a_partial_graph() {
    let pad = Pad::builtin("dance-single").unwrap();
    assert!(StepGraph::decode_raw(&[], pad).is_err());
    assert!(StepGraph::decode_raw(&[1, 0, 0, 0, 255, 255, 255, 127], pad).is_err());
    assert!(StepGraph::decode_fsg(b"invalid compressed graph", pad).is_err());
    assert!(StepGraph::builtin("not-a-layout").is_err());
    assert_eq!(StepGraph::builtin("dance-single").unwrap().nodes.len(), 644);
}
