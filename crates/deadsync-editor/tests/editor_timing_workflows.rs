use deadsync_editor::{
    generation::{
        editor_timing::EditorTimeline,
        timing::{Event, EventKind, GenerationTiming},
    },
    notes::{Grid, Kind, Note, Position},
};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
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
        let timeline =
            EditorTimeline::build(&timing, &grid, region, &AtomicBool::new(false)).unwrap();
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
    assert_eq!(count, 29, "Every committed editor timing case must run");
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
