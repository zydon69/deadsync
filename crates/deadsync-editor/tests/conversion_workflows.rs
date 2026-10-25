use deadsync_editor::{
    generation::{
        config::{BracketDetermination, BracketMethod, ExpressedConfig, PerformedConfig},
        expressed::{self, InstanceType, LinkInstance, StepEvent},
        fallbacks::Fallbacks,
        graph::{FootAction, StepGraph},
        performed::{self, Request},
        search::BLANK_LINK,
        step_data::StepType,
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
fn source_data(
    input: &Value,
) -> (
    Vec<StepEvent>,
    Vec<deadsync_editor::generation::mines::ExpressedMine>,
) {
    if let Some(events) = input["LinkEvents"].as_array() {
        let events = events
            .iter()
            .map(|event| {
                let mut instance = LinkInstance {
                    link: BLANK_LINK,
                    types: [[InstanceType::Default; 2]; 2],
                };
                for f in 0..2 {
                    for p in 0..2 {
                        let i = f * 8 + p * 4;
                        instance.link.feet[f][p].valid = event["Link"][i] == 1;
                        instance.link.feet[f][p].step =
                            StepType::from_byte(event["Link"][i + 1].as_u64().unwrap() as u8)
                                .unwrap();
                        instance.link.feet[f][p].action =
                            match event["Link"][i + 2].as_u64().unwrap() {
                                0 => FootAction::Tap,
                                1 => FootAction::Hold,
                                2 => FootAction::Release,
                                _ => panic!(),
                            };
                        instance.types[f][p] = match event["Link"][i + 3].as_u64().unwrap() {
                            0 => InstanceType::Default,
                            1 => InstanceType::Roll,
                            2 => InstanceType::Fake,
                            3 => InstanceType::Lift,
                            _ => panic!(),
                        };
                    }
                }
                StepEvent {
                    row: event["Row"].as_u64().unwrap(),
                    time: event["Time"].as_f64().unwrap(),
                    instance,
                }
            })
            .collect();
        let mines = input["LinkMines"]
            .as_array()
            .map(|mines| {
                mines
                    .iter()
                    .map(|mine| {
                        use deadsync_editor::generation::mines::{
                            ExpressedMine, MineType, SourceMine,
                        };
                        ExpressedMine {
                            source: SourceMine {
                                row: mine["Row"].as_u64().unwrap(),
                                time: mine["Time"].as_f64().unwrap(),
                                lane: mine["Lane"].as_u64().unwrap() as u16,
                            },
                            kind: match mine["Kind"].as_u64().unwrap() {
                                0 => MineType::AfterArrow,
                                1 => MineType::BeforeArrow,
                                _ => MineType::NoArrow,
                            },
                            nth: mine["Nth"].as_u64().map(|n| n as usize),
                            foot: mine["Foot"].as_u64().map(|f| f as usize),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        return (events, mines);
    }
    let source = input["SourceLayout"].as_str().unwrap();
    let columns = deadsync_editor::notes::layout_columns(source).unwrap();
    let notes = input["Notes"].as_array().unwrap();
    let last = notes
        .iter()
        .map(|note| note[0].as_u64().unwrap())
        .max()
        .unwrap_or(0);
    let mut rows = vec![vec![b'0'; usize::from(columns)]; ((last / 192 + 1) * 192) as usize];
    for note in notes {
        rows[note[0].as_u64().unwrap() as usize][note[1].as_u64().unwrap() as usize] =
            note[2].as_str().unwrap().as_bytes()[0];
    }
    let text = rows
        .chunks(192)
        .map(|measure| {
            measure
                .iter()
                .map(|row| std::str::from_utf8(row).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n,\n");
    let grid = Grid::parse(text.as_bytes(), columns).unwrap();
    let before = grid.clone();
    let cancelled = AtomicBool::new(false);
    let frames = expressed::frames(&grid, 0, &cancelled).unwrap();
    let times: Vec<_> = frames.iter().map(|f| f.row as f64 / 120.0).collect();
    let graph = StepGraph::builtin(source).unwrap();
    let pad = deadsync_editor::pad::Pad::builtin(source).unwrap();
    let config = ExpressedConfig {
        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
        default_bracket_parsing_method: BracketMethod::Balanced,
        ..ExpressedConfig::default()
    };
    let source_mines: Vec<_> = notes
        .iter()
        .filter(|note| note[2] == "M")
        .map(|note| deadsync_editor::generation::mines::SourceMine {
            row: note[0].as_u64().unwrap(),
            time: note[0].as_f64().unwrap() / 120.0,
            lane: note[1].as_u64().unwrap() as u16,
        })
        .collect();
    let result = expressed::build_chart(
        &graph,
        pad,
        &expressed::PolicyRequest {
            frames: &frames,
            times: &times,
            difficulty: 12.0,
            event_span: [0.0, last as f64 / 120.0],
        },
        &config,
        &source_mines,
        &cancelled,
        |_, _, _| {},
    )
    .unwrap();
    assert_eq!(grid, before);
    (result.events, result.mines)
}
fn encode(
    graph: &StepGraph,
    row: u64,
    time: f64,
    node: usize,
    instance: Option<LinkInstance>,
) -> Value {
    let state = graph.nodes[node].state;
    json!({"Row": row, "Time": time, "Orientation": state.orientation as u8,
        "Feet": (0..2).map(|f| (0..2).map(|p| {
            let state = state.feet[f][p]; let action = instance.map(|i| i.link.feet[f][p]);
            json!({"Lane": state.lane.map(i32::from).unwrap_or(-1), "State": state.state as u8,
                "Valid": action.is_some_and(|a| a.valid), "Step": action.map_or(0, |a| a.step as u8),
                "Action": action.map_or(0, |a| a.action as u8), "Instance": instance.map_or(0, |i| i.types[f][p] as u8)})
        }).collect::<Vec<_>>()).collect::<Vec<_>>()})
}
fn compare(name: &str) {
    let input = fixture(name, "request");
    let expected = fixture(name, "expected");
    let (events, mines) = source_data(&input);
    let config: PerformedConfig = serde_json::from_value(input["PerformedConfig"].clone()).unwrap();
    let identity;
    let fallbacks = match input["FallbackMode"].as_str().unwrap() {
        "default" => Some(Fallbacks::builtin()),
        "identity" => {
            let lists: serde_json::Map<_, _> = StepType::ALL
                .into_iter()
                .map(|s| (format!("{s:?}"), json!([format!("{s:?}")])))
                .collect();
            identity = Fallbacks::parse(
                &serde_json::to_vec(&json!({"StepTypeFallbacks": lists})).unwrap(),
            )
            .unwrap();
            Some(&identity)
        }
        _ => None,
    };
    let layout = input["Layout"].as_str().unwrap();
    let result = performed::convert(
        &Request {
            layout,
            events: &events,
            config: &config,
            fallbacks,
            seed: input["Seed"].as_i64().unwrap() as i32,
            roots: None,
        },
        &mines,
        &AtomicBool::new(false),
        |_, _, _| {},
    );
    if !expected["Success"].as_bool().unwrap() {
        assert!(result.is_err(), "{name}");
        return;
    }
    let result = result.unwrap_or_else(|error| panic!("{name}: {error}"));
    let expected_mines = expected["Mines"].as_array().unwrap();
    assert_eq!(result.mines.len(), expected_mines.len(), "{name}");
    assert_eq!(result.dropped_mines, mines.len() - result.mines.len());
    for (actual, original) in result.mines.iter().zip(expected_mines) {
        assert_eq!(actual.row, original["Row"].as_u64().unwrap(), "{name}");
        assert_eq!(actual.time, original["Time"].as_f64().unwrap(), "{name}");
        assert_eq!(
            u64::from(actual.lane),
            original["Lane"].as_u64().unwrap(),
            "{name}"
        );
    }
    let result = result.steps;
    assert_eq!(result.layout, layout);
    assert_eq!(
        result.root_tier as u64,
        expected["Tier"].as_u64().unwrap(),
        "{name}"
    );
    assert_eq!(
        result.skipped_events,
        events.len() - result.steps.len(),
        "{name}"
    );
    let graph = StepGraph::builtin(layout).unwrap();
    let mut actual = vec![encode(&graph, 0, 0.0, result.root, None)];
    actual.extend(
        result
            .steps
            .iter()
            .map(|s| encode(&graph, s.row, s.time, s.node, Some(s.instance))),
    );
    let original = expected["Steps"].as_array().unwrap();
    assert_eq!(actual.len(), original.len(), "{name}");
    for (actual, original) in actual.iter().zip(original) {
        assert_eq!(actual["Time"].as_f64(), original["Time"].as_f64(), "{name}");
        let mut actual = actual.as_object().unwrap().clone();
        let mut original = original.as_object().unwrap().clone();
        actual.remove("Time");
        original.remove("Time");
        assert_eq!(actual, original, "{name}");
    }
}
#[test]
fn full_step_conversions_match_original_fixed_seeds_across_layouts() {
    for (source, target) in [
        ("dance-single", "dance-double"),
        ("dance-double", "dance-single"),
        ("dance-single", "pump-single"),
        ("pump-single", "dance-single"),
        ("smx-single", "dance-double"),
        ("dance-double", "pump-double"),
        ("pump-double", "smx-full"),
        ("dance-single", "dance-single"),
    ] {
        for seed in [42, -7] {
            compare(&format!("convert-{source}-to-{target}-{seed}"));
        }
    }
}
#[test]
fn skipped_steps_root_fallbacks_and_impossible_paths_match_original() {
    for variant in [
        "blank-carry",
        "missing-final",
        "missing-path",
        "root-fallback",
    ] {
        compare(&format!("convert-manual-{variant}"));
    }
    assert!(
        fixture("convert-manual-root-fallback", "expected")["Steps"]
            .as_array()
            .unwrap()
            .len()
            > 1
    );
    assert!(
        fixture("convert-manual-root-fallback", "expected")["Tier"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn conversion_cancellation_invalid_roots_and_empty_chart_are_explicit() {
    let input = fixture("convert-dance-double-to-dance-single-42", "request");
    let (events, _) = source_data(&input);
    let config: PerformedConfig = serde_json::from_value(input["PerformedConfig"].clone()).unwrap();
    let cancelled = AtomicBool::new(false);
    let request = Request {
        layout: "dance-single",
        events: &events,
        config: &config,
        fallbacks: Some(Fallbacks::builtin()),
        seed: 42,
        roots: None,
    };
    let result = performed::convert_steps(&request, &cancelled, |_, done, _| {
        if done == 4 {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    assert!(result.is_err());
    assert!(performed::convert_steps(&request, &cancelled, |_, _, _| {}).is_err());
    cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
    let roots = vec![Vec::new()];
    let invalid = Request {
        roots: Some(&roots),
        ..request
    };
    assert!(performed::convert_steps(&invalid, &cancelled, |_, _, _| {}).is_err());
    let invalid_roots = vec![vec![usize::MAX]];
    let invalid = Request {
        roots: Some(&invalid_roots),
        ..request
    };
    assert!(performed::convert_steps(&invalid, &cancelled, |_, _, _| {}).is_err());
    let empty = Request {
        events: &[],
        ..request
    };
    let mut result = performed::convert_steps(&empty, &cancelled, |_, _, _| {}).unwrap();
    assert!(result.steps.is_empty());
    assert_eq!(result.skipped_events, 0);
    assert_eq!(result.root_tier, 0);
    assert_eq!(
        result.random.next_i32(),
        deadsync_editor::generation::random::SeededRandom::new(42).next_i32()
    );
}

#[test]
fn destination_mines_empty_charts_and_prng_continuation_match_original() {
    compare("convert-mines-simultaneous-no-arrow");
    for label in ["associations", "only-mines", "empty"] {
        for target in ["dance-double", "pump-single"] {
            compare(&format!("convert-mines-{label}-to-{target}"));
        }
    }
    for seed in [42, -7] {
        compare(&format!("convert-manual-mine-recency-{seed}"));
    }
}

#[test]
fn random_root_order_skips_singleton_key_evaluation_like_dotnet() {
    use deadsync_editor::generation::random::SeededRandom;
    let input = fixture("random-order", "request");
    let expected = fixture("random-order", "expected");
    for (input, expected) in input["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
    {
        let mut random = SeededRandom::new(input["Seed"].as_i64().unwrap() as i32);
        let length = input["Length"].as_u64().unwrap() as usize;
        assert_eq!(json!(random.order_indices(length)), expected["Order"]);
        assert_eq!(
            i64::from(random.next_i32()),
            expected["Next"].as_i64().unwrap()
        );
        assert_eq!(
            expected["Calls"].as_u64().unwrap(),
            if length < 2 { 0 } else { length as u64 }
        );
    }
}

#[test]
fn mine_placement_cancellation_and_invalid_associations_fail_without_partial_output() {
    use deadsync_editor::generation::{
        mines::{self, ExpressedMine, MineType, MovementNode, SourceMine},
        random::SeededRandom,
    };
    let graph = StepGraph::builtin("dance-single").unwrap();
    let nodes = [MovementNode {
        row: 0,
        state: graph.nodes[0].state,
        link: None,
    }];
    let mut random = SeededRandom::new(42);
    assert!(mines::place(&nodes, 4, &[], &mut random, &AtomicBool::new(true)).is_err());
    let mine = ExpressedMine {
        source: SourceMine {
            row: 12,
            time: 0.1,
            lane: 0,
        },
        kind: MineType::BeforeArrow,
        nth: None,
        foot: Some(0),
    };
    assert!(mines::place(&nodes, 4, &[mine], &mut random, &AtomicBool::new(false)).is_err());
    let mine = ExpressedMine {
        kind: MineType::AfterArrow,
        nth: Some(0),
        ..mine
    };
    assert!(mines::place(&nodes, 4, &[mine], &mut random, &AtomicBool::new(false)).is_err());
}
