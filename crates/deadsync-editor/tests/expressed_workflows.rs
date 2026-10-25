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
