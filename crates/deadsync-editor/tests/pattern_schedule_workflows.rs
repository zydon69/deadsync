use deadsync_editor::{
    generation::{
        context::ContextEvent,
        editor_timing::EditorTimeline,
        pattern_job::Span,
        pattern_region::deletion_edits,
        pattern_schedule::{LaneCounts, Region, Schedule},
        timing::{Event, EventKind, GenerationTiming},
    },
    notes::{Grid, Kind, Note, Position},
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
fn region(value: &Value) -> Region {
    let start = value[0].as_u64().unwrap();
    Region {
        span: Span {
            start,
            end: start + value[1].as_u64().unwrap(),
            include_start: value[3].as_bool().unwrap(),
            include_end: value[4].as_bool().unwrap(),
        },
        spacing: value[2].as_u64().unwrap(),
    }
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
                    "signature" => EventKind::TimeSignature,
                    "tempo" => EventKind::Tempo(value[2].as_f64().unwrap()),
                    "delay" => EventKind::Delay(value[2].as_f64().unwrap()),
                    "stop" => EventKind::Stop(value[2].as_f64().unwrap()),
                    "warp" => EventKind::Warp(value[2].as_i64().unwrap()),
                    _ => panic!("Unknown scheduling timing event"),
                },
            })
            .collect(),
    )
    .unwrap()
}
fn grid(input: &Value) -> Grid {
    let mut grid = Grid::new(4, 1).unwrap();
    for value in input["Notes"].as_array().unwrap() {
        grid.set(
            Position {
                player: 0,
                row: value[0].as_u64().unwrap(),
                lane: value[1].as_u64().unwrap() as u16,
            },
            Some(Note {
                kind: match value[2].as_str().unwrap() {
                    "1" => Kind::Tap,
                    "F" => Kind::Fake,
                    "L" => Kind::Lift,
                    "M" => Kind::Mine,
                    "2" => Kind::Hold,
                    _ => panic!("Unknown schedule note"),
                },
                end: value.get(3).and_then(Value::as_u64),
                ..Note::tap()
            }),
        )
        .unwrap();
    }
    grid
}
#[test]
fn batch_overlap_density_and_incremental_lane_tracking_match_original_helpers() {
    let cancelled = AtomicBool::new(false);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut files: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(Result::unwrap)
        .filter(|file| {
            file.file_name()
                .to_string_lossy()
                .starts_with("pattern-schedule-")
                && file
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".request.json")
        })
        .map(|file| file.path())
        .collect();
    files.sort();
    assert_eq!(files.len(), 15, "Scheduling fixture coverage changed");
    for path in files {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let input: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(path.with_file_name(name.replace(".request.json", ".expected.json")))
                .unwrap(),
        )
        .unwrap();
        let regions: Vec<_> = input["Schedule"]
            .as_array()
            .unwrap()
            .iter()
            .map(region)
            .collect();
        let schedule = Schedule::new(regions.clone(), &cancelled).unwrap();
        let original = grid(&input);
        let timing = timing(&input);
        let timeline = EditorTimeline::build(&timing, &original, None, &cancelled)
            .unwrap()
            .with_markers(
                &regions
                    .iter()
                    .map(|region| (region.span.start, region.span.end))
                    .collect::<Vec<_>>(),
                &cancelled,
            )
            .unwrap();
        let mut retained = original.clone();
        for entry in schedule.entries() {
            if let Some((start, end, _)) = entry.steps {
                for (position, note) in
                    deletion_edits(&retained, 0, start, end, entry.region.spacing, &cancelled)
                        .unwrap()
                        .0
                {
                    retained.set(position, note).unwrap();
                }
            }
        }
        let retained = timeline.project(&retained, None, &cancelled).unwrap();
        assert_eq!(
            schedule.total_steps(),
            expected["TotalSteps"].as_u64().unwrap(),
            "{name} total scheduled steps"
        );
        let last_time = schedule
            .last_step()
            .map(|row| retained.position_time(row).unwrap())
            .unwrap_or(0.0);
        assert!(
            (last_time - expected["LastTime"].as_f64().unwrap()).abs() < 1e-9,
            "{name} final marker time"
        );
        assert!(
            (schedule.average_nps(&retained).unwrap() - expected["Nps"].as_f64().unwrap()).abs()
                < 1e-9,
            "{name} global density"
        );
        let mut events = retained.context_events();
        let mut counts = LaneCounts::new(4).unwrap();
        assert_eq!(
            schedule.entries().len(),
            expected["Stages"].as_array().unwrap().len()
        );
        for (index, (entry, stage)) in schedule
            .entries()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
            .enumerate()
        {
            let mut kept = Vec::new();
            if let Some((first, last, count)) = entry.steps {
                assert_eq!(first, stage["First"].as_u64().unwrap(), "{name} first row");
                assert_eq!(last, stage["Last"].as_u64().unwrap(), "{name} last row");
                assert_eq!(count, stage["Count"].as_u64().unwrap(), "{name} step count");
                counts.advance(&events, first, &cancelled).unwrap();
                for number in 0..count {
                    let row = first + number * entry.region.spacing;
                    if entry.keeps_row(row) {
                        kept.push(json!({"Row":row,"Lane":(number+index as u64)%4}));
                    }
                }
            } else {
                assert_eq!(stage["Count"], 0, "{name} empty region");
            }
            assert_eq!(
                json!(counts.lanes()),
                stage["LaneCounts"],
                "{name} stage {index} lanes"
            );
            assert_eq!(
                counts.total(),
                stage["TotalSteps"].as_i64().unwrap(),
                "{name} stage {index} cumulative total"
            );
            assert_eq!(
                counts.last_row().map(|row| row as i64).unwrap_or(-1),
                stage["LastCaptured"].as_i64().unwrap(),
                "{name} stage {index} captured row"
            );
            assert_eq!(
                kept,
                stage["Kept"].as_array().unwrap().clone(),
                "{name} stage {index} next-region trim"
            );
            assert_eq!(entry.retained_steps as usize, kept.len());
            for note in kept {
                events.push(ContextEvent {
                    row: note["Row"].as_u64().unwrap(),
                    lane: Some(note["Lane"].as_u64().unwrap() as u16),
                    is_step: true,
                });
            }
            events.sort_by_key(|event| event.row);
        }
    }
}
#[test]
fn invalid_schedules_and_cancelled_or_invalid_counter_updates_preserve_state() {
    let valid = region(&json!([48, 96, 12, true, true]));
    assert!(Schedule::new([valid], &AtomicBool::new(true)).is_err());
    let mut invalid = valid;
    invalid.spacing = 0;
    assert!(Schedule::new([invalid], &AtomicBool::new(false)).is_err());
    invalid = valid;
    invalid.span.end = 47;
    assert!(Schedule::new([invalid], &AtomicBool::new(false)).is_err());
    invalid = valid;
    invalid.span.end = i32::MAX as u64 + 1;
    assert!(Schedule::new([invalid], &AtomicBool::new(false)).is_err());
    let oversized = region(&json!([0, i32::MAX, 1, true, true]));
    assert!(Schedule::new([oversized], &AtomicBool::new(false)).is_err());
    assert!(LaneCounts::new(0).is_err());
    let mut counts = LaneCounts::new(4).unwrap();
    let cancelled = AtomicBool::new(false);
    counts
        .advance(
            &[ContextEvent {
                row: 0,
                lane: Some(2),
                is_step: true,
            }],
            48,
            &cancelled,
        )
        .unwrap();
    let before = counts.clone();
    for events in [
        vec![
            ContextEvent {
                row: 2,
                lane: Some(1),
                is_step: true,
            },
            ContextEvent {
                row: 1,
                lane: Some(1),
                is_step: true,
            },
        ],
        vec![ContextEvent {
            row: 1,
            lane: Some(4),
            is_step: true,
        }],
        vec![ContextEvent {
            row: 1,
            lane: None,
            is_step: true,
        }],
    ] {
        assert!(counts.advance(&events, 96, &cancelled).is_err());
        assert_eq!(counts, before);
    }
    assert!(
        counts
            .advance(&[], i32::MAX as u64 + 1, &cancelled)
            .is_err()
    );
    assert_eq!(counts, before);
    assert!(counts.advance(&[], 96, &AtomicBool::new(true)).is_err());
    assert_eq!(counts, before);
}
