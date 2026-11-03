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
        .join(if name.starts_with("parity-pair-") {
            format!("generation-pairs/{name}.{suffix}.json")
        } else {
            format!("{name}.{suffix}.json")
        });
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn source_data(
    input: &Value,
) -> (
    Vec<StepEvent>,
    Vec<deadsync_editor::generation::mines::ExpressedMine>,
) {
    source_data_checked(input).unwrap()
}
fn source_data_checked(
    input: &Value,
) -> Result<
    (
        Vec<StepEvent>,
        Vec<deadsync_editor::generation::mines::ExpressedMine>,
    ),
    deadsync_editor::Error,
> {
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
        return Ok((events, mines));
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
    );
    assert_eq!(grid, before);
    let result = result?;
    Ok((result.events, result.mines))
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
    let source = source_data_checked(&input);
    if expected["Stage"] == "Expression" {
        assert!(source.is_err(), "{name}: original expression refused");
        return;
    }
    let (events, mines) = source.unwrap_or_else(|error| panic!("{name}: {error}"));
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
    if expected["SourceException"] == true {
        // Original mine placement throws. Rust must return normally, without
        // mutating source events; no successful-output parity is asserted.
        if let Ok(performance) = result {
            assert_eq!(
                performance.dropped_mines,
                mines.len() - performance.mines.len()
            );
            let columns = deadsync_editor::notes::layout_columns(layout).unwrap();
            assert!(
                performance
                    .mines
                    .iter()
                    .all(|mine| mine.lane < u16::from(columns))
            );
        }
        return;
    }
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
    let assembled = deadsync_editor::generation::output::assemble(&result, &AtomicBool::new(false))
        .unwrap_or_else(|error| panic!("{name}: output assembly: {error}"));
    let columns = deadsync_editor::notes::layout_columns(layout).unwrap();
    let note_events = expected["NoteEvents"].as_array().unwrap();
    let max_row = note_events
        .iter()
        .map(|event| event["Row"].as_u64().unwrap())
        .max()
        .unwrap_or(0);
    let mut rows = vec![vec![b'0'; usize::from(columns)]; ((max_row / 192 + 1) * 192) as usize];
    for event in note_events {
        rows[event["Row"].as_u64().unwrap() as usize][event["Lane"].as_u64().unwrap() as usize] =
            event["Symbol"].as_str().unwrap().as_bytes()[0];
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
    let original_grid = Grid::parse(text.as_bytes(), columns).unwrap();
    assert_eq!(assembled.grid, original_grid, "{name}: canonical SM notes");
    let unique_mines = assembled
        .grid
        .iter()
        .filter(|(_, note)| note.kind == deadsync_editor::notes::Kind::Mine)
        .count();
    assert_eq!(assembled.merged_mines, result.mines.len() - unique_mines);
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

#[test]
fn original_pad_containment_controls_automatic_replacements_for_all_layout_pairs() {
    let input = fixture("pad-containment", "request");
    let expected = fixture("pad-containment", "expected");
    let layouts = input["Layouts"].as_array().unwrap();
    assert_eq!(layouts.len(), 14);
    for (a, source) in layouts.iter().enumerate() {
        for (b, target) in layouts.iter().enumerate() {
            let source = source.as_str().unwrap();
            let target = target.as_str().unwrap();
            let fit = deadsync_editor::pad::Pad::builtin(source)
                .unwrap()
                .can_fit_within(deadsync_editor::pad::Pad::builtin(target).unwrap());
            assert_eq!(
                fit,
                expected[a][b].as_bool().unwrap(),
                "{source} -> {target}"
            );
            assert_eq!(
                performed::automatic_fallbacks(source, target)
                    .unwrap()
                    .is_some(),
                !fit
            );
        }
    }
}

#[test]
fn containment_preserves_relationships_and_handles_sparse_custom_coordinates() {
    use deadsync_editor::pad::Pad;
    let source = Pad::builtin("dance-single").unwrap();
    let mut target = source.clone();
    let other = source.arrows[0].other_foot_pairings[0]
        .iter()
        .position(|&value| value)
        .unwrap();
    target.arrows[0].other_foot_pairings[0][other] = false;
    assert!(
        !source.can_fit_within(&target),
        "Same geometry must not lose a supported move"
    );
    let mut translated = source.clone();
    for arrow in &mut translated.arrows {
        arrow.x += 1_000_000_000;
        arrow.y -= 1_000_000_000;
    }
    assert!(source.can_fit_within(&translated));
    let mut sparse = source.clone();
    for (i, arrow) in sparse.arrows.iter_mut().enumerate() {
        arrow.x = i as i32 * 500_000_000;
    }
    assert!(!source.can_fit_within(&sparse));
}

#[test]
fn actual_file_chart_generation_applies_once_saves_and_reopens_through_rssp() {
    use deadsync_editor::{
        analysis,
        generation::{output, source},
        storage::FileSession,
    };
    let input = fixture("convert-dance-single-to-dance-double-42", "request");
    let (events, _) = source_data(&input);
    // Build a real source file from the same source notes, including holds,
    // rolls, mines, fake/lift and keysounds, instead of editing a dummy chart.
    let notes = input["Notes"].as_array().unwrap();
    let mut rows = vec![vec![b'0'; 4]; 192];
    for note in notes {
        rows[note[0].as_u64().unwrap() as usize][note[1].as_u64().unwrap() as usize] =
            note[2].as_str().unwrap().as_bytes()[0];
    }
    let raw = rows
        .iter()
        .map(|row| std::str::from_utf8(row).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = format!("#VERSION:0.83;#TITLE:Keep title;#ARTIST:Keep author;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Source;#DIFFICULTY:Hard;#METER:9;#CREDIT:Keep credit;#VENDOR:keep;#BPMS:0=150;#NOTES:\n{raw}\n;").into_bytes();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, &bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let source_grid = file.session.grid(0).unwrap().clone();
    let prepared = source::prepare(
        file.session.serialize().unwrap().into(),
        deadsync_editor::document::Format::Ssc,
        0,
        &AtomicBool::new(false),
    )
    .unwrap();
    let source_graph = StepGraph::builtin("dance-single").unwrap();
    let config = ExpressedConfig {
        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
        ..ExpressedConfig::default()
    };
    let interpreted = expressed::build_chart(
        &source_graph,
        deadsync_editor::pad::Pad::builtin("dance-single").unwrap(),
        &prepared.request(),
        &config,
        &prepared.mines,
        &AtomicBool::new(false),
        |_, _, _| {},
    )
    .unwrap();
    assert_eq!(interpreted.events.len(), events.len());
    let config: PerformedConfig = serde_json::from_value(input["PerformedConfig"].clone()).unwrap();
    let converted = performed::convert(
        &Request {
            layout: "dance-double",
            events: &interpreted.events,
            config: &config,
            fallbacks: performed::automatic_fallbacks("dance-single", "dance-double").unwrap(),
            seed: 42,
            roots: None,
        },
        &interpreted.mines,
        &AtomicBool::new(false),
        |_, _, _| {},
    )
    .unwrap();
    let assembled = output::assemble(&converted, &AtomicBool::new(false)).unwrap();
    let revision = file.session.revision();
    let added = file
        .session
        .append_chart(
            0,
            &converted.steps.layout,
            &assembled.grid,
            revision,
            "Generate chart",
        )
        .unwrap();
    assert_eq!(added, 1);
    assert_eq!(file.session.grid(0).unwrap(), &source_grid);
    assert!(file.session.serialize().unwrap().starts_with(&bytes));
    let analyzed =
        analysis::chart(&file.session, added, &rssp::AnalysisOptions::default()).unwrap();
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.grid(added).unwrap(), &assembled.grid);
    assert_eq!(
        analysis::chart(&reopened.session, added, &rssp::AnalysisOptions::default())
            .unwrap()
            .short_hash,
        analyzed.short_hash
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), bytes);
    file.session.redo().unwrap();
    assert_eq!(file.session.grid(added).unwrap(), &assembled.grid);
}

#[test]
fn all_eleven_layout_conversion_pairs_match_source_or_document_its_exception() {
    compare("parity-pair-dance-single-to-dance-single");
    compare("parity-pair-dance-single-to-dance-double");
    compare("parity-pair-dance-single-to-dance-solo");
    compare("parity-pair-dance-single-to-dance-threepanel");
    compare("parity-pair-dance-single-to-pump-single");
    compare("parity-pair-dance-single-to-pump-halfdouble");
    compare("parity-pair-dance-single-to-pump-double");
    compare("parity-pair-dance-single-to-smx-beginner");
    compare("parity-pair-dance-single-to-smx-single");
    compare("parity-pair-dance-single-to-smx-dual");
    compare("parity-pair-dance-single-to-smx-full");
    compare("parity-pair-dance-double-to-dance-single");
    compare("parity-pair-dance-double-to-dance-double");
    compare("parity-pair-dance-double-to-dance-solo");
    compare("parity-pair-dance-double-to-dance-threepanel");
    compare("parity-pair-dance-double-to-pump-single");
    compare("parity-pair-dance-double-to-pump-halfdouble");
    compare("parity-pair-dance-double-to-pump-double");
    compare("parity-pair-dance-double-to-smx-beginner");
    compare("parity-pair-dance-double-to-smx-single");
    compare("parity-pair-dance-double-to-smx-dual");
    compare("parity-pair-dance-double-to-smx-full");
    compare("parity-pair-dance-solo-to-dance-single");
    compare("parity-pair-dance-solo-to-dance-double");
    compare("parity-pair-dance-solo-to-dance-solo");
    compare("parity-pair-dance-solo-to-dance-threepanel");
    compare("parity-pair-dance-solo-to-pump-single");
    compare("parity-pair-dance-solo-to-pump-halfdouble");
    compare("parity-pair-dance-solo-to-pump-double");
    compare("parity-pair-dance-solo-to-smx-beginner");
    compare("parity-pair-dance-solo-to-smx-single");
    compare("parity-pair-dance-solo-to-smx-dual");
    compare("parity-pair-dance-solo-to-smx-full");
    compare("parity-pair-dance-threepanel-to-dance-single");
    compare("parity-pair-dance-threepanel-to-dance-double");
    compare("parity-pair-dance-threepanel-to-dance-solo");
    compare("parity-pair-dance-threepanel-to-dance-threepanel");
    compare("parity-pair-dance-threepanel-to-pump-single");
    compare("parity-pair-dance-threepanel-to-pump-halfdouble");
    compare("parity-pair-dance-threepanel-to-pump-double");
    compare("parity-pair-dance-threepanel-to-smx-beginner");
    compare("parity-pair-dance-threepanel-to-smx-single");
    compare("parity-pair-dance-threepanel-to-smx-dual");
    compare("parity-pair-dance-threepanel-to-smx-full");
    compare("parity-pair-pump-single-to-dance-single");
    compare("parity-pair-pump-single-to-dance-double");
    compare("parity-pair-pump-single-to-dance-solo");
    compare("parity-pair-pump-single-to-dance-threepanel");
    compare("parity-pair-pump-single-to-pump-single");
    compare("parity-pair-pump-single-to-pump-halfdouble");
    compare("parity-pair-pump-single-to-pump-double");
    compare("parity-pair-pump-single-to-smx-beginner");
    compare("parity-pair-pump-single-to-smx-single");
    compare("parity-pair-pump-single-to-smx-dual");
    compare("parity-pair-pump-single-to-smx-full");
    compare("parity-pair-pump-halfdouble-to-dance-single");
    compare("parity-pair-pump-halfdouble-to-dance-double");
    compare("parity-pair-pump-halfdouble-to-dance-solo");
    compare("parity-pair-pump-halfdouble-to-dance-threepanel");
    compare("parity-pair-pump-halfdouble-to-pump-single");
    compare("parity-pair-pump-halfdouble-to-pump-halfdouble");
    compare("parity-pair-pump-halfdouble-to-pump-double");
    compare("parity-pair-pump-halfdouble-to-smx-beginner");
    compare("parity-pair-pump-halfdouble-to-smx-single");
    compare("parity-pair-pump-halfdouble-to-smx-dual");
    compare("parity-pair-pump-halfdouble-to-smx-full");
    compare("parity-pair-pump-double-to-dance-single");
    compare("parity-pair-pump-double-to-dance-double");
    compare("parity-pair-pump-double-to-dance-solo");
    compare("parity-pair-pump-double-to-dance-threepanel");
    compare("parity-pair-pump-double-to-pump-single");
    compare("parity-pair-pump-double-to-pump-halfdouble");
    compare("parity-pair-pump-double-to-pump-double");
    compare("parity-pair-pump-double-to-smx-beginner");
    compare("parity-pair-pump-double-to-smx-single");
    compare("parity-pair-pump-double-to-smx-dual");
    compare("parity-pair-pump-double-to-smx-full");
    compare("parity-pair-smx-beginner-to-dance-single");
    compare("parity-pair-smx-beginner-to-dance-double");
    compare("parity-pair-smx-beginner-to-dance-solo");
    compare("parity-pair-smx-beginner-to-dance-threepanel");
    compare("parity-pair-smx-beginner-to-pump-single");
    compare("parity-pair-smx-beginner-to-pump-halfdouble");
    compare("parity-pair-smx-beginner-to-pump-double");
    compare("parity-pair-smx-beginner-to-smx-beginner");
    compare("parity-pair-smx-beginner-to-smx-single");
    compare("parity-pair-smx-beginner-to-smx-dual");
    compare("parity-pair-smx-beginner-to-smx-full");
    compare("parity-pair-smx-single-to-dance-single");
    compare("parity-pair-smx-single-to-dance-double");
    compare("parity-pair-smx-single-to-dance-solo");
    compare("parity-pair-smx-single-to-dance-threepanel");
    compare("parity-pair-smx-single-to-pump-single");
    compare("parity-pair-smx-single-to-pump-halfdouble");
    compare("parity-pair-smx-single-to-pump-double");
    compare("parity-pair-smx-single-to-smx-beginner");
    compare("parity-pair-smx-single-to-smx-single");
    compare("parity-pair-smx-single-to-smx-dual");
    compare("parity-pair-smx-single-to-smx-full");
    compare("parity-pair-smx-dual-to-dance-single");
    compare("parity-pair-smx-dual-to-dance-double");
    compare("parity-pair-smx-dual-to-dance-solo");
    compare("parity-pair-smx-dual-to-dance-threepanel");
    compare("parity-pair-smx-dual-to-pump-single");
    compare("parity-pair-smx-dual-to-pump-halfdouble");
    compare("parity-pair-smx-dual-to-pump-double");
    compare("parity-pair-smx-dual-to-smx-beginner");
    compare("parity-pair-smx-dual-to-smx-single");
    compare("parity-pair-smx-dual-to-smx-dual");
    compare("parity-pair-smx-dual-to-smx-full");
    compare("parity-pair-smx-full-to-dance-single");
    compare("parity-pair-smx-full-to-dance-double");
    compare("parity-pair-smx-full-to-dance-solo");
    compare("parity-pair-smx-full-to-dance-threepanel");
    compare("parity-pair-smx-full-to-pump-single");
    compare("parity-pair-smx-full-to-pump-halfdouble");
    compare("parity-pair-smx-full-to-pump-double");
    compare("parity-pair-smx-full-to-smx-beginner");
    compare("parity-pair-smx-full-to-smx-single");
    compare("parity-pair-smx-full-to-smx-dual");
    compare("parity-pair-smx-full-to-smx-full");
}
