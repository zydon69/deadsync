use deadsync_editor::generation::timing::{Event, EventKind, GenerationTiming};
use serde_json::Value;
use std::{fs, path::PathBuf};

// C# emits whole double values as JSON integers; compare their numeric f64
// representation without changing their value or allowing a tolerance.
fn event_times(mut events: Value) -> Value {
    for event in events.as_array_mut().unwrap() {
        event["Time"] = serde_json::json!(event["Time"].as_f64().unwrap());
    }
    events
}

#[test]
fn note_and_interpretation_group_clocks_match_original_fumen() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in ["delay", "tempo-delay", "mixed", "negative-stops", "overlap"] {
        let input: Value = serde_json::from_slice(
            &fs::read(root.join(format!("clock-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("clock-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let mut events = Vec::new();
        for field in ["Bpms", "Delays", "Stops", "Warps"] {
            if let Some(values) = input[field].as_array() {
                for value in values {
                    events.push(Event {
                        row: value[0].as_i64().unwrap(),
                        kind: match field {
                            "Bpms" => EventKind::Tempo(value[1].as_f64().unwrap()),
                            "Delays" => EventKind::Delay(value[1].as_f64().unwrap()),
                            "Stops" => EventKind::Stop(value[1].as_f64().unwrap()),
                            _ => EventKind::Warp(value[1].as_i64().unwrap()),
                        },
                    });
                }
            }
        }
        let timing = GenerationTiming::new(events).unwrap();
        let rows = input["Rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_u64().unwrap())
            .collect::<Vec<_>>();
        for (field, times) in [
            ("Notes", timing.times(rows.iter().copied()).unwrap()),
            (
                "Groups",
                timing.grouped_times(rows.iter().copied()).unwrap(),
            ),
        ] {
            let expected = expected[field].as_array().unwrap();
            assert_eq!(times.len(), expected.len());
            for (index, (actual, expected)) in times.into_iter().zip(expected).enumerate() {
                assert_eq!(
                    actual,
                    expected.as_f64().unwrap(),
                    "{name} {field} probe {index}"
                );
            }
        }
    }
}

#[test]
fn grouped_clock_rejects_unordered_input_and_checks_cancellation() {
    let timing = GenerationTiming::new(vec![Event {
        row: 0,
        kind: EventKind::Tempo(120.0),
    }])
    .unwrap();
    assert!(timing.grouped_times([48, 0]).is_err());
    assert!(timing.grouped_times([i32::MAX as u64 + 1]).is_err());
    let mut calls = 0;
    assert!(
        timing
            .grouped_times_checked([0, 48, 96], || {
                calls += 1;
                if calls > 2 {
                    Err(deadsync_editor::Error::InvalidEdit("Cancelled".into()))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
}

#[test]
fn real_sm_ssc_preparation_matches_original_reader_timestamps_and_event_span() {
    use deadsync_editor::{document::Format, generation::source};
    use std::sync::{Arc, atomic::AtomicBool};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    for name in [
        "sm",
        "inherited",
        "own",
        "offset-only",
        "metadata",
        "ending-delay",
        "ending-scroll-delay",
    ] {
        let input: Value = serde_json::from_slice(
            &fs::read(root.join(format!("file-clock-{name}.request.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(format!("file-clock-{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let format = if input["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let bytes: Arc<[u8]> = input["Content"].as_str().unwrap().as_bytes().into();
        let prepared = source::prepare(bytes, format, 0, &AtomicBool::new(false)).unwrap();
        assert_eq!(prepared.layout, "dance-single");
        assert_eq!(prepared.difficulty, 9.0);
        assert_eq!(
            prepared.event_span,
            expected["Span"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
                .as_slice(),
            "{name} event span"
        );
        use deadsync_editor::generation::{
            config::{BracketDetermination, ExpressedConfig},
            expressed,
            graph::StepGraph,
        };
        let graph = StepGraph::builtin(&prepared.layout).unwrap();
        let config = ExpressedConfig {
            bracket_parsing_determination: BracketDetermination::UseDefaultMethod,
            ..ExpressedConfig::default()
        };
        let interpretation = expressed::build_chart(
            &graph,
            deadsync_editor::pad::Pad::builtin(&prepared.layout).unwrap(),
            &prepared.request(),
            &config,
            &prepared.mines,
            &AtomicBool::new(false),
            |_, _, _| {},
        );
        assert_eq!(
            interpretation.is_ok(),
            expected["Success"].as_bool().unwrap(),
            "{name} interpretation"
        );
        let interpretation = interpretation.unwrap();
        let steps = interpretation
            .events
            .iter()
            .map(|event| {
                let link = (0..2)
                    .flat_map(|foot| {
                        (0..2).flat_map(move |portion| {
                            let action = event.instance.link.feet[foot][portion];
                            [
                                u64::from(action.valid),
                                action.step as u64,
                                action.action as u64,
                                event.instance.types[foot][portion] as u64,
                            ]
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({"Row":event.row, "Time":event.time, "Link":link})
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::json!(steps),
            event_times(expected["Steps"].clone()),
            "{name} interpreted movements"
        );
        let mines = interpretation
            .mines
            .iter()
            .map(|mine| {
                serde_json::json!({
                    "Row":mine.source.row, "Time":mine.source.time, "Lane":mine.source.lane,
                    "Kind":mine.kind as u64, "Nth":mine.nth.map(|value| value as i64).unwrap_or(-1),
                    "Foot":mine.foot.map(|value| value as i64).unwrap_or(-1)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::json!(mines),
            event_times(expected["Mines"].clone()),
            "{name} interpreted mines"
        );
        for (frame, time) in prepared.frames.iter().zip(&prepared.times) {
            let original = expected["Notes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|note| note["Row"].as_u64().unwrap() == frame.row)
                .unwrap();
            assert_eq!(
                *time,
                original["GroupTime"].as_f64().unwrap(),
                "{name} frame row {}",
                frame.row
            );
        }
        for mine in &prepared.mines {
            let original = expected["Notes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|note| {
                    note["Row"].as_u64().unwrap() == mine.row
                        && note["Lane"].as_u64().unwrap() == u64::from(mine.lane)
                        && note["Symbol"] == "M"
                })
                .unwrap();
            assert_eq!(mine.time, original["Time"].as_f64().unwrap(), "{name} mine");
        }
    }
}

#[test]
fn preparation_uses_one_unsaved_snapshot_and_rejects_cancelled_or_invalid_requests() {
    use deadsync_editor::{
        document::{Document, Format},
        generation::source,
        history::Session,
        notes::{Note, Position},
    };
    use std::sync::{Arc, atomic::AtomicBool};
    let bytes: Arc<[u8]> = b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000\n0000\n0000\n0000;".as_slice().into();
    let mut session = Session::new(Document::parse(bytes, Format::Ssc).unwrap());
    session.set_text(Some(0), "BPMS", "0=180").unwrap();
    session.set_text(Some(0), "METER", "12").unwrap();
    session
        .edit_notes(
            0,
            "Unsaved note",
            [(
                Position {
                    player: 0,
                    row: 48,
                    lane: 1,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let snapshot: Arc<[u8]> = session.serialize().unwrap().into();
    let prepared =
        source::prepare(snapshot.clone(), Format::Ssc, 0, &AtomicBool::new(false)).unwrap();
    assert_eq!(prepared.difficulty, 12.0);
    assert_eq!(prepared.frames.len(), 2);
    assert_eq!(prepared.frames[1].row, 48);
    assert_eq!(prepared.times[1], 48.0 * (60.0 / 180.0 / 48.0));
    assert!(source::prepare(snapshot.clone(), Format::Ssc, 0, &AtomicBool::new(true)).is_err());
    assert!(source::prepare(snapshot, Format::Ssc, 1, &AtomicBool::new(false)).is_err());
    session.set_text(Some(0), "METER", "NaN").unwrap();
    assert!(
        source::prepare(
            session.serialize().unwrap().into(),
            Format::Ssc,
            0,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    session.set_text(Some(0), "METER", "12").unwrap();
    session
        .set_text(Some(0), "ATTACKS", "TIME=1:LEN=1:MODS=drunk")
        .unwrap();
    assert!(
        source::prepare(
            session.serialize().unwrap().into(),
            Format::Ssc,
            0,
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
