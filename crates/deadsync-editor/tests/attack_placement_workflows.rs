use deadsync_editor::{
    attacks::placement::Timeline,
    document::{Document, Format},
    history::Session,
};
use serde_json::Value;
#[test]
fn attack_rows_snapped_times_and_coincident_groups_match_the_actual_fumen_file_readers() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-placement.expected.json"
    ))
    .unwrap();
    for (case_index, case) in cases.as_array().unwrap().iter().enumerate() {
        let format = if case["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let session = Session::new(
            Document::parse(case["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
        );
        let before = session.serialize().unwrap();
        for (chart, reference) in case["Charts"].as_array().unwrap().iter().enumerate() {
            let timeline = Timeline::capture(&session, chart).unwrap();
            let expected = reference["Attacks"].as_array().unwrap();
            assert_eq!(
                timeline.markers().len(),
                expected.len(),
                "case {case_index} chart {chart}"
            );
            let editor = reference["EditorAttacks"].as_array().unwrap();
            assert_eq!(timeline.markers().len(), editor.len());
            for (marker, event) in timeline.markers().iter().zip(editor) {
                assert_eq!(marker.row, event["Row"].as_u64().unwrap());
                assert_eq!(
                    marker.chart_time,
                    event["Time"].as_f64().unwrap(),
                    "editor phase case {case_index} row {}",
                    marker.row
                );
            }
            for (actual, expected) in timeline.markers().iter().zip(expected) {
                assert_eq!(
                    actual.row,
                    expected["Row"].as_u64().unwrap(),
                    "case {case_index} chart {chart}"
                );
                assert_eq!(
                    actual.loaded_time,
                    expected["Time"].as_f64().unwrap(),
                    "case {case_index} row {}",
                    actual.row
                );
                assert_eq!(
                    actual.modifiers.len(),
                    expected["Modifiers"].as_array().unwrap().len()
                );
                for (modifier, reference) in actual
                    .modifiers
                    .iter()
                    .zip(expected["Modifiers"].as_array().unwrap())
                {
                    assert_eq!(modifier.name, reference["Name"].as_str().unwrap());
                    assert_eq!(modifier.level, reference["Level"].as_f64().unwrap());
                    assert_eq!(modifier.speed, reference["Speed"].as_f64().unwrap());
                    assert_eq!(modifier.length, reference["Length"].as_f64().unwrap());
                }
            }
        }
        assert_eq!(session.serialize().unwrap(), before);
    }
}

#[test]
fn placement_index_finds_crossing_durations_wraps_navigation_and_never_mutates_source() {
    let content=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0:LEN=10:MODS=100% Dizzy:TIME=0.5:LEN=2:MODS=50% Drunk:TIME=0.501:LEN=3:MODS=100% Reverse;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let session = Session::new(Document::parse(content.as_slice(), Format::Ssc).unwrap());
    let timeline = Timeline::capture(&session, 0).unwrap();
    assert_eq!(timeline.markers().len(), 2);
    assert_eq!(timeline.at(48).unwrap().source_indices, vec![1, 2]);
    assert_eq!(timeline.at(48).unwrap().modifiers.len(), 2);
    assert!(timeline.last_row() >= 960);
    assert_eq!(timeline.visible(400, 500, 1).unwrap()[0].row, 0);
    assert_eq!(timeline.neighbor(0, true), Some(48));
    assert_eq!(timeline.neighbor(48, true), Some(0));
    assert_eq!(timeline.neighbor(0, false), Some(48));
    assert_eq!(timeline.neighbor(49, false), Some(48));
    assert!(timeline.visible(500, 400, 1).is_err());
    assert!(timeline.visible(0, 500, 0).is_err());
    assert_eq!(session.serialize().unwrap(), content);
}
#[test]
fn attack_phase_precedes_same_row_delays_and_supports_unsaved_note_tails_and_offsets() {
    use deadsync_editor::{
        notes::{Kind, Note, Position},
        playback::Clock,
    };
    let content=b"#VERSION:0.83;#BPMS:0=120;#DELAYS:4=1;#ATTACKS:TIME=2:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut session = Session::new(Document::parse(content.as_slice(), Format::Ssc).unwrap());
    let clock = Clock::capture(&session, 0).unwrap();
    assert_eq!(
        Timeline::capture(&session, 0)
            .unwrap()
            .at(192)
            .unwrap()
            .chart_time,
        2.0
    );
    assert_eq!(clock.note_time(192).unwrap(), 3.0);
    session
        .edit_notes(
            0,
            "Unsaved tail",
            [(
                Position {
                    row: 0,
                    lane: 1,
                    player: 0,
                },
                Some(Note {
                    kind: Kind::Hold,
                    end: Some(768),
                    attributes: "".into(),
                    tail_attributes: "".into(),
                }),
            )],
        )
        .unwrap();
    let before = session.serialize().unwrap();
    let working = Timeline::capture(&session, 0).unwrap();
    let reopened = Session::new(Document::parse(before.clone(), Format::Ssc).unwrap());
    let saved = Timeline::capture(&reopened, 0).unwrap();
    assert_eq!(working.markers().len(), saved.markers().len());
    for (a, b) in working.markers().iter().zip(saved.markers()) {
        assert_eq!(a.row, b.row);
        assert_eq!(a.chart_time, b.chart_time);
        assert_eq!(a.loaded_time, b.loaded_time);
    }
    assert_eq!(session.serialize().unwrap(), before);
}
