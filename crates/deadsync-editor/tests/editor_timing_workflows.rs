use deadsync_editor::{
    generation::{
        editor_timing::EditorTimeline,
        timing::{Event, EventKind, GenerationTiming},
    },
    notes::{Grid, Kind, Note, Position},
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

#[test]
fn scheduled_attacks_extend_generation_span_without_creating_steps() {
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(120.0),
    }])
    .unwrap();
    let grid = Grid::new(4, 1).unwrap();
    let cancelled = AtomicBool::new(false);
    let timeline =
        EditorTimeline::build_with_attacks(&timing, &grid, None, &[96, 192], &cancelled).unwrap();
    let region = timeline
        .prepare_region(&grid, Some((0, 192)), &cancelled)
        .unwrap();
    assert_eq!(region.event_span, [0.0, 2.0]);
    assert!(region.frames.is_empty());
    assert!(region.times.is_empty());
    assert_eq!(region.editor_events.len(), 2);
    assert_eq!(region.editor_events[0].row, 96);
    assert_eq!(region.editor_events[1].row, 192);
    assert!(
        region
            .editor_events
            .iter()
            .all(|event| !event.is_step && event.lane.is_none())
    );
    let clipped = timeline
        .prepare_region(&grid, Some((0, 95)), &cancelled)
        .unwrap();
    assert_eq!(clipped.event_span, [0.0, 0.0]);
    assert!(clipped.editor_events.is_empty());
}

#[test]
fn scheduled_attack_clock_keeps_generation_input_guards() {
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(120.0),
    }])
    .unwrap();
    let grid = Grid::new(4, 1).unwrap();
    assert!(
        EditorTimeline::build_with_attacks(
            &timing,
            &grid,
            None,
            &[i32::MAX as u64 + 1],
            &AtomicBool::new(false),
        )
        .is_err()
    );
    assert!(
        EditorTimeline::build_with_attacks(&timing, &grid, None, &[0], &AtomicBool::new(true),)
            .is_err()
    );
    assert!(
        EditorTimeline::build_with_attacks(
            &timing,
            &Grid::new(4, 2).unwrap(),
            None,
            &[0],
            &AtomicBool::new(false),
        )
        .is_err()
    );
}

fn timing(input: &Value) -> GenerationTiming {
    GenerationTiming::new(
        input["Timing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| Event {
                row: value[0].as_i64().unwrap(),
                kind: match value[1].as_str().unwrap() {
                    "tempo" => EventKind::Tempo(value[2].as_f64().unwrap()),
                    "signature" => EventKind::TimeSignature,
                    "stop" => EventKind::Stop(value[2].as_f64().unwrap()),
                    "delay" => EventKind::Delay(value[2].as_f64().unwrap()),
                    "warp" => EventKind::Warp(value[2].as_i64().unwrap()),
                    "fake" => EventKind::Fake(value[2].as_i64().unwrap()),
                    "scroll" => EventKind::Scroll(value[2].as_f64().unwrap()),
                    "label" => EventKind::Label,
                    "tick" => EventKind::TickCount,
                    "combo" => EventKind::Combo,
                    "speed" => EventKind::TrailingMarker,
                    _ => panic!("Unknown timing fixture"),
                },
            })
            .collect(),
    )
    .unwrap()
}
fn grid(input: &Value) -> Grid {
    let mut grid = Grid::new(4, 1).unwrap();
    for value in input["Notes"].as_array().unwrap() {
        let kind = match value[2].as_str().unwrap() {
            "1" => Kind::Tap,
            "2" => Kind::Hold,
            "4" => Kind::Roll,
            "F" => Kind::Fake,
            "L" => Kind::Lift,
            "M" => Kind::Mine,
            "K" => Kind::Keysound,
            _ => panic!("Unknown fixture note"),
        };
        grid.set(
            Position {
                player: 0,
                row: value[0].as_u64().unwrap(),
                lane: value[1].as_u64().unwrap() as u16,
            },
            Some(Note {
                kind,
                end: value.get(3).and_then(Value::as_u64),
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    grid
}
fn loaded_regions(input: &Value) -> Vec<(u64, u64)> {
    input["LoadedRegions"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|v| (v[0].as_u64().unwrap(), v[1].as_u64().unwrap()))
                .collect()
        })
        .unwrap_or_default()
}
#[test]
fn density_fake_flags_end_times_and_rate_queries_match_unchanged_editor_methods() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("editor-timing-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let timing = timing(&input);
        let grid = grid(&input);
        let region = input["Region"]
            .as_array()
            .map(|values| (values[0].as_u64().unwrap(), values[1].as_u64().unwrap()));
        let timeline = EditorTimeline::build(&timing, &grid, region, &AtomicBool::new(false))
            .unwrap()
            .with_markers(&loaded_regions(&input), &AtomicBool::new(false))
            .unwrap();
        let original = expected["Heads"].as_array().unwrap();
        assert_eq!(timeline.heads().len(), original.len(), "{name}");
        for (head, reference) in timeline.heads().iter().zip(original) {
            assert_eq!(
                head.position.row,
                reference["Row"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(head.position.lane),
                reference["Lane"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                head.time,
                reference["Time"].as_f64().unwrap(),
                "{name} row {}",
                head.position.row
            );
            assert_eq!(
                head.fake,
                reference["Fake"].as_bool().unwrap(),
                "{name} fake row {}",
                head.position.row
            );
        }
        assert_eq!(
            timeline.step_count() as u64,
            expected["StepCount"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            timeline.chart_end().unwrap(),
            expected["End"].as_f64().unwrap(),
            "{name} chart end"
        );
        for (row, time) in input["Rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Times"].as_array().unwrap())
        {
            assert_eq!(
                timeline.position_time(row.as_u64().unwrap()).unwrap(),
                time.as_f64().unwrap(),
                "{name} position {}",
                row
            );
        }
        for position in expected["PlaybackPositions"].as_array().unwrap() {
            let time = position["Time"].as_f64().unwrap();
            assert_eq!(
                timeline.time_position(time).unwrap(),
                position["Row"].as_f64().unwrap(),
                "{name} playback at {time}"
            );
        }
        assert_eq!(
            timeline
                .pattern_nps(
                    input["NumSteps"].as_u64().unwrap(),
                    input["LastStep"].as_u64().unwrap()
                )
                .unwrap(),
            expected["Nps"].as_f64().unwrap(),
            "{name} pattern NPS"
        );
    }
    assert_eq!(count, 39, "Every committed editor timing case must run");
}
#[test]
fn editor_timing_rejects_cancelled_invalid_and_overflowing_inputs() {
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(150.0),
    }])
    .unwrap();
    let grid = Grid::new(4, 1).unwrap();
    assert!(EditorTimeline::build(&timing, &grid, None, &AtomicBool::new(true)).is_err());
    assert!(
        EditorTimeline::build(
            &timing,
            &Grid::new(4, 2).unwrap(),
            None,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(EditorTimeline::build(&timing, &grid, Some((96, 0)), &AtomicBool::new(false)).is_err());
    let timeline = EditorTimeline::build(&timing, &grid, None, &AtomicBool::new(false)).unwrap();
    assert!(timeline.position_time(i32::MAX as u64 + 1).is_err());
    assert!(timeline.pattern_nps(i32::MAX as u64 + 1, 0).is_err());
    assert!(
        GenerationTiming::new(vec![Event {
            row: 0,
            kind: EventKind::Tempo(f64::MIN_POSITIVE)
        }])
        .is_err()
    );
    let overflow = GenerationTiming::new(vec![
        Event {
            row: 0,
            kind: EventKind::Stop(-f64::MAX),
        },
        Event {
            row: 0,
            kind: EventKind::Stop(-f64::MAX),
        },
    ])
    .unwrap();
    assert!(EditorTimeline::build(&overflow, &grid, None, &AtomicBool::new(false)).is_err());
    let negative = GenerationTiming::new(vec![Event {
        row: -48,
        kind: EventKind::Tempo(150.0),
    }])
    .unwrap();
    assert!(EditorTimeline::build(&negative, &grid, None, &AtomicBool::new(false)).is_err());
}
#[test]
fn source_reuse_keeps_effective_timing_and_metadata_with_original_grid_unchanged() {
    use deadsync_editor::{document::Format, generation::source};
    use std::sync::Arc;
    let bytes:Arc<[u8]>=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:12;#BPMS:0=150;#FAKES:0=1;#NOTES:1000\n0100\n0010\n0001;".as_slice().into();
    let source = source::read(bytes, Format::Ssc, 0, &AtomicBool::new(false)).unwrap();
    let original = source.grid.clone();
    let timeline =
        EditorTimeline::build(&source.timing, &source.grid, None, &AtomicBool::new(false)).unwrap();
    assert!(timeline.heads()[0].fake);
    assert_eq!(timeline.step_count(), 3);
    let (region, events) = deadsync_editor::generation::pattern_region::extract(
        &source.grid,
        0,
        48,
        144,
        &AtomicBool::new(false),
    )
    .unwrap();
    let prepared = source
        .prepare_grid(&region, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(prepared.difficulty, 12.0);
    assert_eq!(prepared.frames.len(), 3);
    assert_eq!(prepared.times[0], 0.4);
    assert_eq!(events.len(), 3);
    assert_eq!(source.grid, original);
}

#[test]
fn projected_timing_and_context_regions_match_original_deletion_and_extraction() {
    use deadsync_editor::generation::pattern_region;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("editor-timing-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let expected = &expected["Projected"];
        let options = &input["Projection"];
        let original = grid(&input);
        let timing = timing(&input);
        let loaded_region = input["Region"]
            .as_array()
            .map(|values| (values[0].as_u64().unwrap(), values[1].as_u64().unwrap()));
        let timeline =
            EditorTimeline::build(&timing, &original, loaded_region, &AtomicBool::new(false))
                .unwrap()
                .with_markers(&loaded_regions(&input), &AtomicBool::new(false))
                .unwrap();
        let (edits, report) = pattern_region::deletion_edits(
            &original,
            0,
            options["Start"].as_u64().unwrap(),
            options["End"].as_u64().unwrap(),
            options["Spacing"].as_u64().unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut retained = original.clone();
        for (position, note) in edits {
            retained.set(position, note).unwrap();
        }
        let region = options["Region"].as_array().unwrap();
        let region = (region[0].as_u64().unwrap(), region[1].as_u64().unwrap());
        let projected = timeline
            .project(&retained, Some(region), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(
            report.removed as u64,
            expected["Removed"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            report.shortened_holds as u64,
            expected["Shortened"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            projected.heads().len(),
            expected["Heads"].as_array().unwrap().len(),
            "{name}"
        );
        for (head, reference) in projected
            .heads()
            .iter()
            .zip(expected["Heads"].as_array().unwrap())
        {
            assert_eq!(
                head.position.row,
                reference["Row"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(head.position.lane),
                reference["Lane"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                head.time,
                reference["Time"].as_f64().unwrap(),
                "{name} projected time"
            );
            assert_eq!(
                head.fake,
                reference["Fake"].as_bool().unwrap(),
                "{name} projected fake"
            );
            assert_eq!(
                retained.get(head.position).unwrap().end,
                reference["End"].as_u64(),
                "{name} shortened hold"
            );
        }
        assert_eq!(
            projected.step_count() as u64,
            expected["StepCount"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            projected.chart_end().unwrap(),
            expected["End"].as_f64().unwrap(),
            "{name} projected chart end"
        );
        assert_eq!(
            projected
                .pattern_nps(
                    options["NumSteps"].as_u64().unwrap(),
                    options["End"].as_u64().unwrap()
                )
                .unwrap(),
            expected["Nps"].as_f64().unwrap(),
            "{name} projected NPS"
        );
        for (range, reference) in options["Ranges"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Regions"].as_array().unwrap())
        {
            let range = range[1]
                .as_u64()
                .map(|end| (range[0].as_u64().unwrap(), end));
            let prepared = projected
                .prepare_region(&retained, range, &AtomicBool::new(false))
                .unwrap();
            assert_eq!(
                prepared.event_span,
                serde_json::from_value::<[f64; 2]>(reference["Span"].clone()).unwrap(),
                "{name} context span {range:?}"
            );
            for (frame, time) in prepared.frames.iter().zip(&prepared.times) {
                let group = reference["Groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|group| group["Row"].as_u64().unwrap() == frame.row)
                    .unwrap();
                assert_eq!(
                    *time,
                    group["Time"].as_f64().unwrap(),
                    "{name} context row {}",
                    frame.row
                );
            }
            let actual=prepared.editor_events.iter().map(|event|serde_json::json!({"Row":event.row,"Lane":event.lane.map(i64::from).unwrap_or(-1),"Step":event.is_step})).collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!(actual),
                reference["EditorEvents"],
                "{name} editor context order {range:?}"
            );
        }
        assert_eq!(original, grid(&input), "{name} immutable original");
    }
    assert_eq!(count, 39);
}
#[test]
fn projections_reject_new_or_altered_heads_and_do_not_retime_survivors() {
    let timing = GenerationTiming::new(vec![
        Event {
            row: 0,
            kind: EventKind::Tempo(187.3),
        },
        Event {
            row: 48,
            kind: EventKind::Warp(144),
        },
        Event {
            row: 63,
            kind: EventKind::TimeSignature,
        },
        Event {
            row: 120,
            kind: EventKind::Stop(-0.25),
        },
    ])
    .unwrap();
    let mut original = Grid::new(4, 1).unwrap();
    let position = Position {
        player: 0,
        lane: 0,
        row: 0,
    };
    original
        .set(
            position,
            Some(Note {
                kind: Kind::Hold,
                end: Some(240),
                ..Note::tap()
            }),
        )
        .unwrap();
    original
        .set(
            Position {
                player: 0,
                lane: 1,
                row: 60,
            },
            Some(Note::tap()),
        )
        .unwrap();
    let timeline =
        EditorTimeline::build(&timing, &original, None, &AtomicBool::new(false)).unwrap();
    let mut retained = original.clone();
    let mut shortened = retained.get(position).unwrap().clone();
    shortened.end = Some(36);
    retained.set(position, Some(shortened)).unwrap();
    let projected = timeline
        .project(&retained, Some((48, 97)), &AtomicBool::new(false))
        .unwrap();
    for (actual, previous) in projected.heads().iter().zip(timeline.heads()) {
        assert_eq!(actual.time, previous.time);
        assert_eq!(actual.fake, previous.fake);
    }
    assert!(
        timeline
            .project(&original, None, &AtomicBool::new(true))
            .is_err()
    );
    assert!(
        projected
            .prepare_region(&retained, Some((0, 47)), &AtomicBool::new(true))
            .is_err()
    );
    assert!(
        projected
            .prepare_region(
                &retained,
                Some((0, i32::MAX as u64 + 1)),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    let mut altered = original.clone();
    altered.set(position, Some(Note::tap())).unwrap();
    assert!(
        timeline
            .project(&altered, None, &AtomicBool::new(false))
            .is_err()
    );
    let mut inserted = original.clone();
    inserted
        .set(
            Position {
                player: 0,
                lane: 2,
                row: 72,
            },
            Some(Note::tap()),
        )
        .unwrap();
    assert!(
        timeline
            .project(&inserted, None, &AtomicBool::new(false))
            .is_err()
    );
    let mut extended = original.clone();
    extended
        .set(
            position,
            Some(Note {
                kind: Kind::Hold,
                end: Some(288),
                ..Note::tap()
            }),
        )
        .unwrap();
    assert!(
        timeline
            .project(&extended, None, &AtomicBool::new(false))
            .is_err()
    );
}

#[test]
fn native_chart_conversion_uses_editor_assigned_times() {
    native_pipeline(false);
}
#[test]
fn integrated_pattern_task_matches_original_editor_helpers_and_library() {
    native_pipeline(true);
}
fn native_pipeline(pattern: bool) {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            config::{BracketDetermination, ExpressedConfig},
            job::{ChartOptions, ChartTask},
        },
        history::Session,
    };
    use std::sync::Arc;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("editor-timing-") || !name.ends_with(".request.json") {
            continue;
        }
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        // Persisted pattern markers and missing-tempo loader defaults are separate contracts.
        if input.get("Region").is_some()
            || !input["Timing"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v[1] == "tempo")
        {
            continue;
        }
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut tags = std::collections::BTreeMap::<&str, Vec<String>>::new();
        for value in input["Timing"].as_array().unwrap() {
            let beat = value[0].as_f64().unwrap() / 48.0;
            let (tag, data) = match value[1].as_str().unwrap() {
                "tempo" => ("BPMS", value[2].to_string()),
                "signature" => ("TIMESIGNATURES", format!("{}={}", value[2], value[3])),
                "stop" => ("STOPS", value[2].to_string()),
                "delay" => ("DELAYS", value[2].to_string()),
                "warp" => ("WARPS", (value[2].as_f64().unwrap() / 48.0).to_string()),
                "fake" => ("FAKES", (value[2].as_f64().unwrap() / 48.0).to_string()),
                "scroll" => ("SCROLLS", value[2].to_string()),
                "speed" => ("SPEEDS", "1=1=0".into()),
                "label" => ("LABELS", "label".into()),
                "tick" => ("TICKCOUNTS", "4".into()),
                "combo" => ("COMBOS", "1=1".into()),
                _ => panic!("Unknown fixture timing"),
            };
            tags.entry(tag).or_default().push(format!("{beat}={data}"));
        }
        let mut bytes = b"#VERSION:0.83;".to_vec();
        for (tag, values) in tags {
            bytes.extend_from_slice(format!("#{tag}:{};", values.join(",")).as_bytes());
        }
        bytes.extend_from_slice(b"#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:");
        bytes.extend(grid(&input).serialize().unwrap());
        bytes.push(b';');
        if input.get("LoadedRegions").is_some() {
            let patterns = loaded_regions(&input)
                .into_iter()
                .map(|(row, end)| (row.to_string(), serde_json::json!({"Length":end-row})))
                .collect::<serde_json::Map<_, _>>();
            let json = serde_json::json!({"Patterns":patterns}).to_string();
            bytes.extend_from_slice(b"#GAChartDataVersion:1;#GAChartData:");
            bytes.extend(deadsync_editor::document::escape_text(&json));
            bytes.push(b';');
        }
        let session = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
        if pattern {
            use deadsync_editor::generation::pattern_job::{Options, Span, Task};
            let result = Task::capture(
                &session,
                0,
                Options {
                    span: Span {
                        start: input["Projection"]["Region"][0].as_u64().unwrap(),
                        end: input["Projection"]["Region"][1].as_u64().unwrap(),
                        include_start: input["Inclusive"][0].as_bool().unwrap_or(true),
                        include_end: input["Inclusive"][1].as_bool().unwrap_or(true),
                    },
                    seed: 42,
                    expressed: ExpressedConfig {
                        bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
                        ..ExpressedConfig::default()
                    },
                    performed: serde_json::from_value(
                        input["Conversion"]["PerformedConfig"].clone(),
                    )
                    .unwrap(),
                    pattern: serde_json::from_value(input["PatternConfig"].clone()).unwrap(),
                    ignore_preceding_distribution: false,
                },
            )
            .unwrap()
            .run(&AtomicBool::new(false), |_, _| {});
            assert_eq!(
                result.is_ok(),
                expected["Pipeline"]["Success"].as_bool().unwrap(),
                "{name}: {err:?}",
                err = result.as_ref().err()
            );
            if let Ok(preview) = result {
                assert_eq!(
                    preview.average_nps(),
                    expected["Pipeline"]["Nps"].as_f64().unwrap(),
                    "{name}"
                );
                // Full original editor action, rather than the old reconstructed
                // single-call pipeline, owns the generated output contract.
                let worker: Value = serde_json::from_slice(
                    &fs::read(
                        root.parent()
                            .unwrap()
                            .join("pattern-worker")
                            .join(name.replace(".request.json", ".expected.json")),
                    )
                    .unwrap(),
                )
                .unwrap();
                let reference = deadsync_editor::notes::Grid::parse(
                    worker["NoteData"].as_str().unwrap().as_bytes(),
                    4,
                )
                .unwrap();
                assert_eq!(preview.grid(), &reference, "{name} original action output");
                assert_eq!(worker["Before"], worker["Undo"], "{name} source Undo");
                assert_eq!(worker["NoteData"], worker["Redo"], "{name} source Redo");
            }
            count += 1;
            continue;
        }
        let mut options = ChartOptions::balanced("dance-double");
        options.seed = 42;
        options.expressed = ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        };
        options.performed =
            serde_json::from_value(input["Conversion"]["PerformedConfig"].clone()).unwrap();
        let result = ChartTask::capture(&session, 0, options)
            .unwrap()
            .run(&AtomicBool::new(false), |_| {});
        assert_eq!(
            result.is_ok(),
            expected["Conversion"]["Success"].as_bool().unwrap(),
            "{name}"
        );
        if let Ok(preview) = result {
            let mut actual = Vec::new();
            for (position, note) in preview.grid().iter() {
                let symbol = match note.kind {
                    Kind::Tap => "1",
                    Kind::Hold => "2",
                    Kind::Roll => "4",
                    Kind::Mine => "M",
                    Kind::Fake => "F",
                    Kind::Lift => "L",
                    _ => panic!("Unexpected output"),
                };
                actual.push((position.row, u64::from(position.lane), symbol.to_owned()));
                if let Some(end) = note.end {
                    actual.push((end, u64::from(position.lane), "3".to_owned()));
                }
            }
            let mut reference = expected["Conversion"]["Notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    (
                        v["Row"].as_u64().unwrap(),
                        v["Lane"].as_u64().unwrap(),
                        v["Symbol"].as_str().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            actual.sort();
            reference.sort();
            reference.dedup();
            assert_eq!(actual, reference, "{name}");
        }
        count += 1;
    }
    assert_eq!(count, 36);
}

#[test]
fn inserted_taps_preserve_survivors_and_match_original_typed_times_fakes_and_contexts() {
    use deadsync_editor::generation::{pattern_job::Span, pattern_region::deletion_edits};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let cancelled = AtomicBool::new(false);
    let mut count = 0;
    for file in fs::read_dir(&root).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("editor-insertion-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut retained = grid(&input);
        let original = retained.clone();
        let region = input["Region"]
            .as_array()
            .map(|value| (value[0].as_u64().unwrap(), value[1].as_u64().unwrap()));
        let original_timeline =
            EditorTimeline::build(&timing(&input), &retained, region, &cancelled)
                .unwrap()
                .with_markers(&loaded_regions(&input), &cancelled)
                .unwrap();
        for marker in input["InsertionClear"].as_array().unwrap() {
            let start = marker[0].as_u64().unwrap();
            let spacing = marker[2].as_u64().unwrap();
            let span = Span {
                start,
                end: start + marker[1].as_u64().unwrap(),
                include_start: marker[3].as_bool().unwrap(),
                include_end: marker[4].as_bool().unwrap(),
            };
            if let Some((start, end, _)) = span.optional_steps(spacing).unwrap() {
                for (position, note) in
                    deletion_edits(&retained, 0, start, end, spacing, &cancelled)
                        .unwrap()
                        .0
                {
                    retained.set(position, note).unwrap();
                }
            }
        }
        let mut timeline = original_timeline
            .project(&retained, None, &cancelled)
            .unwrap();
        for (stage, reference) in input["Insertions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected.as_array().unwrap())
        {
            let taps: Vec<_> = stage
                .as_array()
                .unwrap()
                .iter()
                .map(|note| Position {
                    player: 0,
                    row: note[0].as_u64().unwrap(),
                    lane: note[1].as_u64().unwrap() as u16,
                })
                .collect();
            timeline = timeline.with_taps(taps.clone(), &cancelled).unwrap();
            for tap in taps {
                retained.set(tap, Some(Note::tap())).unwrap();
            }
            assert_eq!(
                timeline.heads().len(),
                reference["Heads"].as_array().unwrap().len(),
                "{name} heads"
            );
            for (head, expected) in timeline
                .heads()
                .iter()
                .zip(reference["Heads"].as_array().unwrap())
            {
                assert_eq!(
                    head.position.row,
                    expected["Row"].as_u64().unwrap(),
                    "{name} row"
                );
                assert_eq!(
                    u64::from(head.position.lane),
                    expected["Lane"].as_u64().unwrap(),
                    "{name} lane"
                );
                assert_eq!(
                    head.time,
                    expected["Time"].as_f64().unwrap(),
                    "{name} time at {:?}",
                    head.position
                );
                assert_eq!(
                    head.fake,
                    expected["Fake"].as_bool().unwrap(),
                    "{name} fake at {:?}",
                    head.position
                );
            }
            for survivor in original_timeline.heads().iter().filter(|head| {
                retained.get(head.position) == original.get(head.position)
                    && !input["InsertionClear"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|marker| marker[0].as_u64() == Some(head.position.row))
            }) {
                let head = timeline
                    .heads()
                    .iter()
                    .find(|head| head.position == survivor.position)
                    .unwrap();
                assert_eq!(
                    head.time.to_bits(),
                    survivor.time.to_bits(),
                    "{name} survivor time"
                );
                assert_eq!(head.fake, survivor.fake, "{name} survivor fake flag");
            }
            assert_eq!(
                timeline.step_count() as u64,
                reference["StepCount"].as_u64().unwrap(),
                "{name} count"
            );
            assert_eq!(
                timeline.chart_end().unwrap(),
                reference["End"].as_f64().unwrap(),
                "{name} end"
            );
            for (range, reference) in input["InsertionRanges"]
                .as_array()
                .unwrap()
                .iter()
                .zip(reference["Regions"].as_array().unwrap())
            {
                let range = range[1]
                    .as_u64()
                    .map(|end| (range[0].as_u64().unwrap(), end));
                let prepared = timeline
                    .prepare_region(&retained, range, &cancelled)
                    .unwrap();
                assert_eq!(
                    prepared.event_span,
                    serde_json::from_value::<[f64; 2]>(reference["Span"].clone()).unwrap(),
                    "{name} region {range:?}"
                );
                for (frame, time) in prepared.frames.iter().zip(&prepared.times) {
                    let expected = reference["Groups"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|group| group["Row"].as_u64() == Some(frame.row))
                        .unwrap();
                    assert_eq!(
                        *time,
                        expected["Time"].as_f64().unwrap(),
                        "{name} group {}",
                        frame.row
                    );
                }
                let events:Vec<_>=prepared.editor_events.iter().map(|event|serde_json::json!({"Row":event.row,"Lane":event.lane.map(i64::from).unwrap_or(-1),"Step":event.is_step})).collect();
                assert_eq!(
                    serde_json::json!(events),
                    reference["EditorEvents"],
                    "{name} context event order"
                );
            }
        }
    }
    assert_eq!(count, 43);
}
#[test]
fn insertion_rejects_collisions_invalid_positions_and_cancellation_without_changing_timeline() {
    let mut notes = Grid::new(4, 1).unwrap();
    let cancelled = AtomicBool::new(false);
    let position = |row, lane| Position {
        player: 0,
        row,
        lane,
    };
    notes
        .set(
            position(0, 0),
            Some(Note {
                kind: Kind::Hold,
                end: Some(96),
                ..Note::tap()
            }),
        )
        .unwrap();
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(120.0),
    }])
    .unwrap();
    let timeline = EditorTimeline::build(&timing, &notes, None, &cancelled).unwrap();
    for taps in [
        vec![position(0, 0)],
        vec![position(48, 0)],
        vec![position(96, 0)],
        vec![position(48, 1), position(48, 1)],
        vec![position(48, 4)],
        vec![Position {
            player: 1,
            row: 48,
            lane: 1,
        }],
        vec![position(i32::MAX as u64 + 1, 1)],
    ] {
        assert!(timeline.with_taps(taps, &cancelled).is_err());
        assert_eq!(timeline.heads().len(), 1);
    }
    assert!(
        timeline
            .with_taps([position(48, 1)], &AtomicBool::new(true))
            .is_err()
    );
    let flag = AtomicBool::new(false);
    let taps = (0..2).map(|index| {
        if index == 1 {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        position(index * 24, 1)
    });
    assert!(timeline.with_taps(taps, &flag).is_err());
    assert_eq!(timeline.heads().len(), 1);
    let added = timeline.with_taps([position(97, 0)], &cancelled).unwrap();
    assert_eq!(added.heads().len(), 2);
    assert_eq!(timeline.heads().len(), 1);
}

#[test]
fn playback_position_rejects_nonfinite_time_and_missing_rates() {
    let grid = Grid::new(4, 1).unwrap();
    let timeline = EditorTimeline::build(
        &GenerationTiming::new(vec![Event {
            row: 0,
            kind: EventKind::Tempo(120.0),
        }])
        .unwrap(),
        &grid,
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(timeline.time_position(time).is_err());
    }
    assert_eq!(timeline.time_position(-1.0).unwrap(), -96.0);
    assert_eq!(timeline.time_position(0.25).unwrap(), 24.0);
    let empty = EditorTimeline::build(
        &GenerationTiming::new(vec![]).unwrap(),
        &grid,
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(empty.time_position(1.0).is_err());
}
