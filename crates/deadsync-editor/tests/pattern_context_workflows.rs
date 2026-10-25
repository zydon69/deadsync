use deadsync_editor::{
    generation::{
        config::{BracketDetermination, ExpressedConfig, PatternConfig, PerformedConfig},
        context::{self, ContextEvent, ContextRequest},
        expressed,
        graph::StepGraph,
        interpretation_path::PathNode,
        pattern::{self, PatternRequest},
        timing::{Event, EventKind, GenerationTiming},
    },
    notes::Grid,
    pad::Pad,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn fixture(name: &str, suffix: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grooveauthor")
        .join(format!("{name}.{suffix}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn interpret(
    notes: &[Value],
    graph: &StepGraph,
    pad: &Pad,
    scale: f64,
) -> Result<Arc<PathNode>, deadsync_editor::Error> {
    let last = notes
        .iter()
        .map(|note| note[0].as_u64().unwrap())
        .max()
        .unwrap_or(0);
    let mut rows = vec![vec![b'0'; graph.columns()]; ((last / 192 + 1) * 192) as usize];
    for note in notes {
        let cell =
            &mut rows[note[0].as_u64().unwrap() as usize][note[1].as_u64().unwrap() as usize];
        assert_eq!(*cell, b'0');
        *cell = note[2].as_str().unwrap().as_bytes()[0];
    }
    let body = rows
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
    let grid = Grid::parse(body.as_bytes(), graph.columns() as u16).unwrap();
    let frames = expressed::frames(&grid, 0, &AtomicBool::new(false)).unwrap();
    let times = frames
        .iter()
        .map(|frame| frame.row as f64 / 120.0 * scale)
        .collect::<Vec<_>>();
    let span = [
        notes
            .iter()
            .map(|note| note[0].as_u64().unwrap())
            .min()
            .unwrap_or(0) as f64
            / 120.0,
        last as f64 / 120.0,
    ];
    expressed::interpret(
        graph,
        pad,
        &expressed::PolicyRequest {
            frames: &frames,
            times: &times,
            difficulty: 12.0,
            event_span: span,
        },
        &ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        },
        &AtomicBool::new(false),
        |_, _, _| {},
    )
    .map(|result| result.last)
}
fn events(input: &Value, notes: &str, explicit: &str) -> Vec<ContextEvent> {
    if let Some(values) = input[explicit].as_array() {
        return values
            .iter()
            .map(|event| ContextEvent {
                row: event[0].as_u64().unwrap(),
                lane: event[1]
                    .as_i64()
                    .filter(|&lane| lane >= 0)
                    .map(|lane| lane as u16),
                is_step: event[2].as_bool().unwrap(),
            })
            .collect();
    }
    let mut events = input[notes]
        .as_array()
        .unwrap()
        .iter()
        .filter(|note| note[2] != "3")
        .map(|note| ContextEvent {
            row: note[0].as_u64().unwrap(),
            lane: Some(note[1].as_u64().unwrap() as u16),
            is_step: matches!(note[2].as_str().unwrap(), "1" | "2" | "4" | "F" | "L"),
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|event| (event.row, event.lane));
    events
}
#[test]
fn surrounding_footing_history_and_patterns_match_unmodified_grooveauthor_helpers() {
    for layout in deadsync_editor::generation::job::TARGET_LAYOUTS {
        let graph = StepGraph::builtin(layout).unwrap();
        let pad = Pad::builtin(layout).unwrap();
        for variant in [
            "empty",
            "following-even",
            "following-odd",
            "preceding",
            "jacks",
            "jump",
            "hold",
            "late",
            "bracket",
            "overlap-order",
            "equal-time",
            "mine-key",
            "release-same-row",
            "transitions",
        ] {
            let name = format!("context-{layout}-{variant}");
            let input = fixture(&name, "request");
            let expected = fixture(&name, "expected");
            let before = interpret(
                input["BeforeNotes"].as_array().unwrap(),
                &graph,
                pad,
                input["TimeScale"].as_f64().unwrap_or(1.0),
            );
            let after = interpret(
                input["AfterNotes"].as_array().unwrap(),
                &graph,
                pad,
                input["TimeScale"].as_f64().unwrap_or(1.0),
            );
            assert_eq!(
                before.is_ok() && after.is_ok(),
                expected["Success"].as_bool().unwrap(),
                "{name} source interpretation"
            );
            if !expected["Success"].as_bool().unwrap() {
                continue;
            }
            let before = before.unwrap();
            let after = after.unwrap();
            let before_events = events(&input, "BeforeNotes", "BeforeEditorEvents");
            let after_events = events(&input, "AfterNotes", "AfterEditorEvents");
            let performed: PerformedConfig =
                serde_json::from_value(input["PerformedConfig"].clone()).unwrap();
            let pattern: PatternConfig =
                serde_json::from_value(input["PatternConfig"].clone()).unwrap();
            let request = ContextRequest {
                start: input["Start"].as_u64().unwrap(),
                end: input["End"].as_u64().unwrap(),
                num_steps: input["NumSteps"].as_u64().unwrap(),
                total_steps_before: input["TotalSteps"].as_i64().unwrap(),
                range_starts_after_chart: input["RangeStartsAfterChart"].as_bool().unwrap(),
                preceding: &before,
                following: &after,
                preceding_events: &before_events,
                following_events: &after_events,
                config: &performed,
            };
            let counts: Vec<u64> = serde_json::from_value(input["LaneCounts"].clone()).unwrap();
            let derived = context::derive(
                &graph,
                pad,
                &request,
                counts.clone(),
                &AtomicBool::new(false),
            )
            .unwrap();
            let reference = &expected["Context"];
            assert_eq!(
                derived.previous_foot,
                reference["PreviousFoot"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                derived.previous_footing,
                serde_json::from_value::<[i32; 2]>(reference["PreviousFooting"].clone()).unwrap(),
                "{name}"
            );
            assert_eq!(
                derived.following_foot,
                reference["FollowingFoot"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                derived.following_footing,
                serde_json::from_value::<[i32; 2]>(reference["FollowingFooting"].clone()).unwrap(),
                "{name}"
            );
            assert_eq!(
                derived.root.previous_times,
                serde_json::from_value::<[f64; 2]>(reference["PreviousTimes"].clone()).unwrap(),
                "{name}"
            );
            assert_eq!(
                derived.root.previous_same_counts,
                serde_json::from_value::<[u64; 2]>(reference["SameCounts"].clone()).unwrap(),
                "{name}"
            );
            let transition = reference["LastTransitionSteps"].as_i64().unwrap();
            assert_eq!(
                derived.root.last_transition,
                (transition != 0)
                    .then_some((reference["LastTransitionLeft"].as_bool(), transition)),
                "{name}"
            );
            assert!(
                context::derive(&graph, pad, &request, counts, &AtomicBool::new(true)).is_err()
            );
            let generated = pattern::generate(
                &PatternRequest {
                    layout: layout.into(),
                    start: request.start,
                    end: request.end,
                    seed: input["Seed"].as_i64().unwrap() as i32,
                    previous_foot: derived.previous_foot,
                    previous_footing: derived.previous_footing,
                    following_footing: derived.following_footing,
                    context: derived.root,
                    config: performed,
                    pattern,
                    average_nps: input["Nps"].as_f64().unwrap(),
                },
                &GenerationTiming::new(vec![Event {
                    row: 0,
                    kind: EventKind::Tempo(150.0),
                }])
                .unwrap(),
                &AtomicBool::new(false),
                |_, _| {},
            );
            assert_eq!(
                generated.is_ok(),
                expected["Notes"].is_array(),
                "{name} generated pattern"
            );
            if let Ok(generated) = generated {
                let actual = generated
                    .steps
                    .iter()
                    .flat_map(|step| {
                        (0..2).flat_map(move |foot| {
                            (0..2).filter_map(move |portion| {
                                step.link.feet[foot][portion].valid.then(|| {
                                    (
                                        step.row,
                                        u64::from(step.feet[foot][portion].lane.unwrap()),
                                        "1".to_owned(),
                                    )
                                })
                            })
                        })
                    })
                    .collect::<Vec<_>>();
                let mut actual = actual;
                actual.sort();
                let mut original = expected["Notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|note| {
                        (
                            note["Row"].as_u64().unwrap(),
                            note["Lane"].as_u64().unwrap(),
                            note["Symbol"].as_str().unwrap().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                original.sort();
                assert_eq!(actual, original, "{name} generated lanes");
            }
        }
    }
}

#[test]
fn invalid_or_cancelled_context_cannot_return_partial_footing() {
    let graph = StepGraph::builtin("dance-single").unwrap();
    let pad = Pad::builtin("dance-single").unwrap();
    let root = PathNode::root(&graph, 0).unwrap();
    let config = PerformedConfig::balanced();
    let request = ContextRequest {
        start: 192,
        end: 384,
        num_steps: 17,
        total_steps_before: 0,
        range_starts_after_chart: false,
        preceding: &root,
        following: &root,
        preceding_events: &[],
        following_events: &[],
        config: &config,
    };
    assert!(context::derive(&graph, pad, &request, vec![0; 4], &AtomicBool::new(true)).is_err());
    assert!(context::derive(&graph, pad, &request, vec![0; 5], &AtomicBool::new(false)).is_err());
    let bad_events = [ContextEvent {
        row: 0,
        lane: Some(4),
        is_step: true,
    }];
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                preceding_events: &bad_events,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let bad_events = [ContextEvent {
        row: 0,
        lane: None,
        is_step: true,
    }];
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                following_events: &bad_events,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                total_steps_before: -1,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                num_steps: 0,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let mut malformed = PathNode::root(&graph, 0).unwrap();
    Arc::get_mut(&mut malformed).unwrap().time = f64::NAN;
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                preceding: &malformed,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        context::derive(
            &graph,
            Pad::builtin("pump-single").unwrap(),
            &request,
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let mut invalid = config.clone();
    invalid.transitions.transition_cutoff_percentage = f64::NAN;
    assert!(
        context::derive(
            &graph,
            pad,
            &ContextRequest {
                config: &invalid,
                ..request
            },
            vec![0; 4],
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
