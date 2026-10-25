use deadsync_editor::{
    generation::{
        expressed::{self, SearchState},
        graph::{ArrowState, NodeState, Orientation, StepGraph},
    },
    notes::Grid,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

fn fixture(name: &str, suffix: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grooveauthor")
        .join(format!("{name}.{suffix}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn node_json(node: NodeState) -> Value {
    let orientation = match node.orientation {
        Orientation::Normal => 0,
        Orientation::InvertedRightOverLeft => 1,
        Orientation::InvertedLeftOverRight => 2,
    };
    json!({"Orientation": orientation, "Feet": node.feet.map(|foot| foot.map(|portion| json!({
        "Lane": portion.lane.map(i32::from).unwrap_or(-1),
        "State": match portion.state { ArrowState::Resting => 0, ArrowState::Held => 1, ArrowState::Lifted => 2 }
    })))})
}
fn state(value: &Value) -> SearchState {
    match value.as_u64().unwrap() {
        0 => SearchState::Empty,
        1 => SearchState::Tap,
        2 => SearchState::Fake,
        3 => SearchState::Lift,
        4 => SearchState::Hold,
        5 => SearchState::Holding,
        6 => SearchState::Roll,
        7 => SearchState::Rolling,
        _ => panic!("Invalid fixture state"),
    }
}
fn compare_original(name: &str, layout: &str) {
    let input = fixture(name, "request");
    let expected = fixture(name, "expected");
    let frames = input["Frames"].as_array().unwrap();
    let expected = expected.as_array().unwrap();
    assert_eq!(
        frames.len(),
        expected.len(),
        "Original must interpret every fixture row"
    );
    let graph = StepGraph::builtin(layout).unwrap();
    for (frame, expected) in frames.iter().zip(expected) {
        let parent = graph
            .nodes
            .iter()
            .position(|node| node_json(node.state) == expected["Parent"])
            .unwrap();
        let states: Vec<_> = frame["State"]
            .as_array()
            .unwrap()
            .iter()
            .map(state)
            .collect();
        let candidates = expressed::candidates(
            &graph,
            parent,
            &states,
            frame["Release"].as_bool().unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let actual: Vec<_> = candidates.iter().map(|candidate| json!({
            "Node": node_json(graph.nodes[candidate.node].state),
            "Feet": candidate.instance.link.feet.iter().enumerate().map(|(f, foot)| foot.iter().enumerate().map(|(p, action)| json!({
                "Valid": action.valid, "Step": action.step as u8, "Action": action.action as u8,
                "Instance": candidate.instance.types[f][p] as u8,
            })).collect::<Vec<_>>()).collect::<Vec<_>>()
        })).collect();
        assert!(
            !actual.is_empty(),
            "Reference fixture must exercise reachable movements"
        );
        assert_eq!(json!(actual), expected["Matches"], "state {:?}", states);
    }
}
#[test]
fn chart_grid_produces_distinct_release_step_and_mine_context_without_changing_notes() {
    use SearchState::*;
    // Hold on left; roll on right; mine simultaneous with a fake; a lift;
    // and a separate participant who must not enter P1 state.
    let grid = Grid::parse(b"2000\n0004\n3000\n0000\n0003\n0000\n0000\n0000\n,\n0000\n0000\n0000\n0000\n0F0M\n00L0\n0000\n0000\n&\n0001\n", 4).unwrap();
    let before = grid.clone();
    let frames = expressed::frames(&grid, 0, &AtomicBool::new(false)).unwrap();
    assert_eq!(grid, before);
    assert_eq!(frames[0].state, [Hold, Empty, Empty, Empty]);
    assert_eq!(frames[1].state, [Holding, Empty, Empty, Roll]);
    assert!(frames[2].release);
    assert_eq!(frames[2].state, [Empty, Empty, Empty, Rolling]);
    assert!(frames[3].release);
    assert_eq!(frames[3].state, [Empty; 4]);
    let fake = frames.iter().find(|frame| frame.state[1] == Fake).unwrap();
    assert_eq!(fake.last_mines[3], fake.row);
    let lift = frames.iter().find(|frame| frame.state[2] == Lift).unwrap();
    assert_eq!(lift.last_releases[1], fake.row);
    assert_eq!(
        expressed::frames(&grid, 1, &AtomicBool::new(false))
            .unwrap()
            .len(),
        1
    );
    assert!(expressed::frames(&grid, 2, &AtomicBool::new(false)).is_err());
    assert!(expressed::frames(&grid, 0, &AtomicBool::new(true)).is_err());
}

#[test]
fn sustained_and_special_note_candidates_match_original_interpreter() {
    compare_original("interpret-single-sustains", "dance-single");
}

#[test]
fn every_layout_preserves_hold_roll_fake_lift_and_jump_interpretations() {
    for layout in [
        "dance-single",
        "dance-double",
        "dance-solo",
        "dance-threepanel",
        "pump-single",
        "pump-halfdouble",
        "pump-double",
        "smx-beginner",
        "smx-single",
        "smx-dual",
        "smx-full",
    ] {
        compare_original(&format!("interpret-{layout}"), layout);
    }
}

#[test]
fn candidate_errors_do_not_return_partial_interpretations() {
    let graph = StepGraph::builtin("dance-single").unwrap();
    assert!(
        expressed::candidates(
            &graph,
            0,
            &[SearchState::Empty; 3],
            false,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        expressed::candidates(
            &graph,
            usize::MAX,
            &[SearchState::Empty; 4],
            false,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        expressed::candidates(
            &graph,
            0,
            &[SearchState::Empty; 4],
            false,
            &AtomicBool::new(true)
        )
        .is_err()
    );
}

#[test]
fn simultaneous_release_mine_and_head_keep_the_reference_phase_order() {
    use SearchState::*;
    use deadsync_editor::notes::{Kind, Note, Position};
    let mut grid = Grid::new(4, 1).unwrap();
    for (lane, row, kind, end) in [
        (0, 12, Kind::Hold, Some(24)),
        (1, 12, Kind::Roll, Some(36)),
        (2, 24, Kind::Fake, None),
        (3, 24, Kind::Mine, None),
    ] {
        grid.set(
            Position {
                player: 0,
                lane,
                row,
            },
            Some(Note {
                kind,
                end,
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    let frames = expressed::frames(&grid, 0, &AtomicBool::new(false)).unwrap();
    assert_eq!(frames.len(), 4);
    let release = &frames[1];
    let step = &frames[2];
    assert_eq!((release.row, step.row), (24, 24));
    assert!(release.release);
    assert!(!step.release);
    assert_eq!(release.state, [Empty, Rolling, Empty, Empty]);
    assert_eq!(step.state, [Empty, Rolling, Fake, Empty]);
    assert_eq!(release.last_mines[3], 0);
    assert_eq!(step.last_mines[3], 24);
    assert_eq!(step.last_releases[0], 24);
}

#[test]
fn branch_pruning_and_foot_history_match_original_for_all_bracket_policies() {
    use deadsync_editor::generation::interpretation_path::{self, PathNode};
    use std::{collections::HashMap, sync::Arc};
    let graph = StepGraph::builtin("dance-single").unwrap();
    for method in ["balanced", "aggressive", "nobrackets"] {
        let name = format!("interpret-paths-{method}");
        let input = fixture(&name, "request");
        let expected = fixture(&name, "expected");
        let root_index = graph
            .nodes
            .iter()
            .position(|node| node_json(node.state) == expected["Root"])
            .unwrap();
        let root = PathNode::root(&graph, root_index).unwrap();
        let mut ids = HashMap::from([(Arc::as_ptr(&root) as usize, 0u64)]);
        let mut current = vec![root];
        let original_frames = expected["Frames"].as_array().unwrap();
        assert_eq!(
            original_frames.len(),
            input["Frames"].as_array().unwrap().len()
        );
        let mut branches = 0;
        for (frame, expected) in input["Frames"]
            .as_array()
            .unwrap()
            .iter()
            .zip(original_frames)
        {
            let states: Vec<_> = frame["State"]
                .as_array()
                .unwrap()
                .iter()
                .map(state)
                .collect();
            let mut originals = expected["Matches"].as_array().unwrap().iter();
            let mut children = Vec::new();
            for parent in &current {
                let candidates = expressed::candidates(
                    &graph,
                    parent.node,
                    &states,
                    frame["Release"].as_bool().unwrap(),
                    &AtomicBool::new(false),
                )
                .unwrap();
                for candidate in candidates {
                    let original = originals.next().expect("Rust must not add a candidate");
                    assert_eq!(
                        ids[&(Arc::as_ptr(parent) as usize)],
                        original["Parent"].as_u64().unwrap()
                    );
                    assert_eq!(
                        node_json(graph.nodes[candidate.node].state),
                        original["Node"]
                    );
                    // Compare Rust move costs before asserting the same pruning.
                    let child = PathNode::child(
                        &graph,
                        parent,
                        candidate.node,
                        expected["Row"].as_u64().unwrap(),
                        expected["Row"].as_f64().unwrap() / 120.0,
                        candidate.instance,
                        original["Cost"].as_i64().unwrap() as i32,
                        original["OrientationCost"].as_i64().unwrap() as i32,
                    )
                    .unwrap();
                    let scoring = deadsync_editor::generation::interpretation_cost::Scoring {
                        graph: &graph,
                        pad: deadsync_editor::pad::Pad::builtin("dance-single").unwrap(),
                        method: match method {
                            "aggressive" => {
                                deadsync_editor::generation::config::BracketMethod::Aggressive
                            }
                            "nobrackets" => {
                                deadsync_editor::generation::config::BracketMethod::NoBrackets
                            }
                            _ => deadsync_editor::generation::config::BracketMethod::Balanced,
                        },
                    };
                    let cost_frame = expressed::Frame {
                        row: child.row,
                        release: frame["Release"].as_bool().unwrap(),
                        state: states.clone(),
                        last_mines: vec![0; 4],
                        last_releases: vec![0; 4],
                    };
                    assert_eq!(
                        scoring.cost(&child, &cost_frame).unwrap(),
                        original["Cost"].as_i64().unwrap() as i32,
                        "{method}: cost at row {} for {:?}",
                        child.row,
                        candidate.instance.link
                    );
                    assert_eq!(
                        scoring.orientation(&child),
                        original["OrientationCost"].as_i64().unwrap() as i32
                    );
                    assert_eq!(
                        i64::from(child.total_cost),
                        original["TotalCost"].as_i64().unwrap()
                    );
                    assert_eq!(
                        i64::from(child.total_orientation_cost),
                        original["TotalOrientationCost"].as_i64().unwrap()
                    );
                    let feet: Vec<_> = child
                        .last_foot
                        .iter()
                        .map(|foot| foot.map(|f| f as i32).unwrap_or(-1))
                        .collect();
                    assert_eq!(json!(feet), original["LastFoot"]);
                    let previous: Vec<_> = [1, 2]
                        .into_iter()
                        .map(|nth| {
                            child
                                .previous_step_node(nth)
                                .map(|node| ids[&(node as *const PathNode as usize)] as i64)
                                .unwrap_or(-1)
                        })
                        .collect();
                    assert_eq!(json!(previous), original["PreviousSteps"]);
                    ids.insert(
                        Arc::as_ptr(&child) as usize,
                        original["Id"].as_u64().unwrap(),
                    );
                    children.push(child);
                }
            }
            assert!(
                originals.next().is_none(),
                "Rust must not drop a matching candidate"
            );
            branches += children.len();
            current = interpretation_path::prune(children);
            let retained: Vec<_> = current
                .iter()
                .map(|node| ids[&(Arc::as_ptr(node) as usize)])
                .collect();
            assert_eq!(
                json!(retained),
                expected["Retained"],
                "{method}: prune at {}",
                expected["Row"]
            );
            let best = interpretation_path::best(&current)
                .map(|node| ids[&(Arc::as_ptr(node) as usize)] as i64)
                .unwrap_or(-1);
            assert_eq!(best, expected["Best"].as_i64().unwrap(), "{method}");
        }
        assert!(
            branches > 100,
            "Exercise branching, not just one linear path"
        );
    }
}

#[test]
fn equal_total_cost_prefers_the_earlier_easier_move_and_long_history_drops_iteratively() {
    use deadsync_editor::generation::{
        expressed::{InstanceType, LinkInstance},
        graph::{FootAction, GraphLink, PortionAction},
        interpretation_path::PathNode,
        search::BLANK_ACTION,
        step_data::StepType,
    };
    use std::{cmp::Ordering, sync::Arc};
    let graph = StepGraph::builtin("dance-single").unwrap();
    let root = PathNode::root(&graph, 0).unwrap();
    let link = LinkInstance {
        link: GraphLink {
            feet: [
                [
                    PortionAction {
                        valid: true,
                        step: StepType::SameArrow,
                        action: FootAction::Tap,
                    },
                    BLANK_ACTION,
                ],
                [BLANK_ACTION; 2],
            ],
        },
        types: [[InstanceType::Default; 2]; 2],
    };
    let easy = PathNode::child(&graph, &root, 0, 12, 0.1, link, 1, 0).unwrap();
    let easy_then_hard = PathNode::child(&graph, &easy, 0, 24, 0.2, link, 2, 0).unwrap();
    let hard = PathNode::child(&graph, &root, 0, 12, 0.1, link, 2, 0).unwrap();
    let hard_then_easy = PathNode::child(&graph, &hard, 0, 24, 0.2, link, 1, 0).unwrap();
    assert_eq!(easy_then_hard.compare(&hard_then_easy), Ordering::Less);
    assert_eq!(hard_then_easy.compare(&easy_then_hard), Ordering::Greater);
    assert_eq!(easy_then_hard.compare(&easy_then_hard), Ordering::Equal);
    assert!(PathNode::child(&graph, &easy, 0, 0, 0.0, link, 0, 0).is_err());
    assert!(PathNode::child(&graph, &root, 0, 12, f64::NAN, link, 0, 0).is_err());
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let mut last = Arc::clone(&root);
            for row in 1..=100_000 {
                last =
                    PathNode::child(&graph, &last, 0, row, row as f64 / 120.0, link, 1, 0).unwrap();
            }
            assert_eq!(last.total_cost, 100_000);
            drop(last);
        })
        .unwrap()
        .join()
        .unwrap();
}

fn chart_grid(input: &Value) -> Grid {
    use deadsync_editor::notes::{Kind, Note, Position};
    let columns =
        deadsync_editor::notes::layout_columns(input["Layout"].as_str().unwrap()).unwrap();
    let mut grid = Grid::new(columns, 1).unwrap();
    let notes = input["Notes"].as_array().unwrap();
    for item in notes {
        let row = item[0].as_u64().unwrap();
        let lane = item[1].as_u64().unwrap() as u16;
        let symbol = item[2].as_str().unwrap();
        let kind = match symbol {
            "1" => Kind::Tap,
            "2" => Kind::Hold,
            "4" => Kind::Roll,
            "M" => Kind::Mine,
            "F" => Kind::Fake,
            "L" => Kind::Lift,
            "K" => Kind::Keysound,
            "3" => continue,
            _ => panic!("Unknown fixture note"),
        };
        let end = matches!(kind, Kind::Hold | Kind::Roll).then(|| {
            notes
                .iter()
                .find(|tail| {
                    tail[0].as_u64().unwrap() > row && tail[1] == item[1] && tail[2] == "3"
                })
                .unwrap()[0]
                .as_u64()
                .unwrap()
        });
        grid.set(
            Position {
                player: 0,
                lane,
                row,
            },
            Some(Note {
                kind,
                end,
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    grid
}

#[test]
fn complete_rust_cost_search_matches_original_chart_search_on_all_layouts() {
    use deadsync_editor::generation::config::BracketMethod;
    let mut successful = 0;
    for layout in [
        "dance-single",
        "dance-double",
        "dance-solo",
        "dance-threepanel",
        "pump-single",
        "pump-halfdouble",
        "pump-double",
        "smx-beginner",
        "smx-single",
        "smx-dual",
        "smx-full",
    ] {
        let graph = StepGraph::builtin(layout).unwrap();
        let pad = deadsync_editor::pad::Pad::builtin(layout).unwrap();
        for (name, method) in [
            ("balanced", BracketMethod::Balanced),
            ("aggressive", BracketMethod::Aggressive),
            ("nobrackets", BracketMethod::NoBrackets),
        ] {
            let name = format!("chart-{layout}-{name}");
            let input = fixture(&name, "request");
            let expected = fixture(&name, "expected");
            let grid = chart_grid(&input);
            let before = grid.clone();
            let frames = expressed::frames(&grid, 0, &AtomicBool::new(false)).unwrap();
            let times: Vec<_> = frames
                .iter()
                .map(|frame| frame.row as f64 / 120.0)
                .collect();
            let result = expressed::search(
                &graph,
                pad,
                &frames,
                &times,
                method,
                &AtomicBool::new(false),
                |_, _| {},
            );
            assert_eq!(grid, before, "{name}: search cannot alter source notes");
            if !expected["Success"].as_bool().unwrap() {
                assert!(result.is_err(), "{name}");
                continue;
            }
            successful += 1;
            let last = result.unwrap_or_else(|error| panic!("{name}: {error}"));
            let mut path = Vec::new();
            let mut current = Some(last.as_ref());
            while let Some(node) = current {
                path.push(node);
                current = node.previous.as_deref();
            }
            path.reverse();
            let original_nodes = expected["Nodes"].as_array().unwrap();
            assert_eq!(path.len(), original_nodes.len(), "{name}");
            for (node, original) in path.iter().zip(original_nodes) {
                assert_eq!(node.row, original["Row"].as_u64().unwrap(), "{name}");
                assert_eq!(node.time, original["Time"].as_f64().unwrap(), "{name}");
                assert_eq!(
                    i64::from(node.cost),
                    original["Cost"].as_i64().unwrap(),
                    "{name}: cost at {}",
                    node.row
                );
                assert_eq!(
                    i64::from(node.total_cost),
                    original["TotalCost"].as_i64().unwrap(),
                    "{name}"
                );
                assert_eq!(
                    i64::from(node.total_orientation_cost),
                    original["OrientationCost"].as_i64().unwrap(),
                    "{name}"
                );
                let state = graph.nodes[node.node].state;
                assert_eq!(
                    node_json(state)["Orientation"],
                    original["Orientation"],
                    "{name}"
                );
                assert_eq!(
                    json!(
                        node.last_foot
                            .iter()
                            .map(|foot| foot.map(|f| f as i32).unwrap_or(-1))
                            .collect::<Vec<_>>()
                    ),
                    original["LastFoot"],
                    "{name}"
                );
                for foot in 0..2 {
                    for portion in 0..2 {
                        let position = state.feet[foot][portion];
                        let action = node.instance.map(|i| i.link.feet[foot][portion]);
                        let actual = json!({"Lane": position.lane.map(i32::from).unwrap_or(-1), "State": position.state as u8,
                        "Valid": action.is_some_and(|a| a.valid), "Step": action.map_or(0, |a| a.step as u8),
                        "Action": action.map_or(0, |a| a.action as u8), "Instance": node.instance.map_or(0, |i| i.types[foot][portion] as u8)});
                        assert_eq!(
                            actual, original["Feet"][foot][portion],
                            "{name}: foot {foot}/{portion} at {}",
                            node.row
                        );
                    }
                }
            }
        }
    }
    assert!(
        successful >= 25,
        "Most fixtures must exercise successful whole searches"
    );
}

#[test]
fn full_search_cancellation_keeps_the_grid_and_returns_no_partial_path() {
    use deadsync_editor::generation::config::BracketMethod;
    let input = fixture("chart-dance-single-balanced", "request");
    let grid = chart_grid(&input);
    let before = grid.clone();
    let cancelled = AtomicBool::new(false);
    let frames = expressed::frames(&grid, 0, &cancelled).unwrap();
    let times: Vec<_> = frames
        .iter()
        .map(|frame| frame.row as f64 / 120.0)
        .collect();
    let graph = StepGraph::builtin("dance-single").unwrap();
    let pad = deadsync_editor::pad::Pad::builtin("dance-single").unwrap();
    let result = expressed::search(
        &graph,
        pad,
        &frames,
        &times,
        BracketMethod::Balanced,
        &cancelled,
        |done, _| {
            if done == 4 {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(grid, before);
}
