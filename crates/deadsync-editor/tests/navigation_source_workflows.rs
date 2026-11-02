use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    navigation::{Command, Event, Index, Lookup},
};
#[test]
fn six_callbacks_match_768_original_editor_tree_queries() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/navigation-source6.expected.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/navigation-source6.expected.json"
    ))
    .unwrap();
    let commands = [
        ("OnMoveToPreviousRowWithSteps", Command::PreviousStep),
        ("OnMoveToNextRowWithSteps", Command::NextStep),
        ("OnMoveToPreviousRowWithEvents", Command::PreviousEvent),
        ("OnMoveToNextRowWithEvents", Command::NextEvent),
        ("OnMoveToStartOfStream", Command::StreamStart),
        ("OnMoveToEndOfStream", Command::StreamEnd),
    ];
    let mut count = 0;
    for (input, output) in request
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
    {
        assert_eq!(input["Name"], output["Name"]);
        let index = Lookup::new(
            input["Events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|event| Event {
                    position: event["Position"].as_f64().unwrap(),
                    step: event["Step"].as_bool().unwrap(),
                }),
        )
        .unwrap();
        for row in output["Rows"].as_array().unwrap() {
            let position = row["Position"].as_f64().unwrap();
            for (name, command) in commands {
                assert_eq!(
                    index.target(command, position, None).unwrap(),
                    row["Commands"][name].as_f64(),
                    "{} {position} {name}",
                    input["Name"]
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 768);
}
fn session() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#SAMPLESTART:0.13;#LASTSECONDHINT:0.33;#FAKES:1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:2000\n0000\n0100\n3000\n0000\n00L0\n000F\n000M;".to_vec(),Format::Ssc).unwrap())
}
#[test]
fn capture_uses_fractional_widgets_fake_steps_and_heads_without_tails() {
    let session = session();
    let index = Index::capture(&session, 0).unwrap();
    let target = |command, row| index.target(&session, 0, command, row, None).unwrap();
    // Eight rows per measure: heads at 0,48,120,144 and mine at168.
    // The 48-row tap lies in the fake region but remains an IsStep.
    assert_eq!(target(Command::NextStep, 1.0), Some(48.0));
    assert_eq!(target(Command::NextStep, 48.0), Some(120.0));
    assert_eq!(target(Command::PreviousStep, 150.0), Some(144.0));
    assert_eq!(target(Command::NextStep, 144.0), None);
    assert_eq!(target(Command::NextEvent, 144.0), Some(168.0));
    assert_eq!(target(Command::NextEvent, 48.0), Some(120.0)); // no tail at72
    assert!((target(Command::NextEvent, 1.0).unwrap() - 12.48).abs() < 1e-10); // preview0.13s
    assert!((target(Command::NextEvent, 13.0).unwrap() - 31.68).abs() < 1e-10); // end hint0.33s
    // Selection is queried dynamically and never mutates the cached index.
    assert_eq!(
        index
            .target(&session, 0, Command::NextEvent, 1.0, Some(7.25))
            .unwrap(),
        Some(7.25)
    );
    assert!((target(Command::NextEvent, 1.0).unwrap() - 12.48).abs() < 1e-10);
}
#[test]
fn lookup_refuses_nonfinite_and_nonintegral_step_positions() {
    assert!(
        Lookup::new([Event {
            position: f64::NAN,
            step: false
        }])
        .is_err()
    );
    assert!(
        Lookup::new([Event {
            position: 1.5,
            step: true
        }])
        .is_err()
    );
    let index = Lookup::new([]).unwrap();
    assert!(
        index
            .target(Command::NextEvent, f64::INFINITY, None)
            .is_err()
    );
    assert!(
        index
            .target(Command::NextStep, 0.0, Some(f64::NAN))
            .is_err()
    );
    assert_eq!(
        index.target(Command::PreviousEvent, 0.0, None).unwrap(),
        None
    );
}
#[test]
fn content_bound_cache_rejects_another_session() {
    let a = session();
    let b = session();
    let index = Index::capture(&a, 0).unwrap();
    assert!(!index.is_current(&b, 0));
    assert!(index.target(&b, 0, Command::NextStep, 0.0, None).is_err());
}

#[test]
fn attack_rows_from_original_reader_and_pattern_starts_are_real_navigation_events() {
    use deadsync_editor::generation::metadata::{
        ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS,
    };
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-placement.expected.json"
    ))
    .unwrap();
    let mut attacks = 0;
    for case in cases.as_array().unwrap() {
        let format = if case["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let session = Session::new(
            Document::parse(case["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
        );
        for (chart, reference) in case["Charts"].as_array().unwrap().iter().enumerate() {
            let index = Index::capture(&session, chart).unwrap();
            for attack in reference["EditorAttacks"].as_array().unwrap() {
                let row = attack["Row"].as_f64().unwrap();
                assert_eq!(
                    index
                        .target(&session, chart, Command::NextEvent, row - 1e-7, None)
                        .unwrap(),
                    Some(row)
                );
                attacks += 1;
            }
        }
    }
    assert!(attacks > 0);
    let mut session = session();
    ChartData {
        patterns: Some(
            [(
                17,
                Definition {
                    length: 24,
                    pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                    performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        ),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let index = Index::capture(&session, 0).unwrap();
    assert_eq!(
        index
            .target(&session, 0, Command::NextEvent, 16.0, None)
            .unwrap(),
        Some(17.0)
    );
    assert_eq!(
        index
            .target(&session, 0, Command::NextStep, 16.0, None)
            .unwrap(),
        Some(48.0)
    );
}
