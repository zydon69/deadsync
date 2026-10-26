use deadsync_editor::{
    attacks::{Modifier, inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    timing_edit::Scope,
};
use serde_json::Value;
#[test]
fn resident_inspection_and_edits_preserve_reference_rows_through_save_reopen_and_exact_undo() {
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
        for chart in 0..case["Charts"].as_array().unwrap().len() {
            let mut session = Session::new(
                Document::parse(case["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
            );
            let scope = if format == Format::Sm {
                Scope::Song
            } else {
                Scope::Chart
            };
            let mut draft = Draft::capture(&session, chart, scope).unwrap();
            let before = session.serialize().unwrap();
            assert!(!draft.has_changes());
            assert!(!draft.apply(&mut session, chart).unwrap());
            assert_eq!(session.serialize().unwrap(), before);
            let expected = case["Charts"][chart]["EditorAttacks"].as_array().unwrap();
            assert_eq!(draft.attacks().events().len(), expected.len());
            for (event, reference) in draft.attacks().events().iter().zip(expected) {
                assert_eq!(event.row, reference["Row"].as_u64().unwrap());
                assert_eq!(event.chart_time, reference["Time"].as_f64().unwrap());
            }
            if draft.attacks().events().is_empty() {
                continue;
            }
            let event = &draft.attacks().events()[0];
            let time = event.time;
            let row = event.row;
            let mut value = event.modifiers[0].clone();
            value.name = if value.name == "Reverse" {
                "Dizzy"
            } else {
                "Reverse"
            }
            .into();
            draft.set(0, 0, time, value).unwrap();
            assert_eq!(draft.start_row(0).unwrap(), row);
            assert!(
                draft.apply(&mut session, chart).unwrap(),
                "case {case_index} chart {chart}"
            );
            let after = session.serialize().unwrap();
            let reopened = Session::new(Document::parse(after.clone(), format).unwrap());
            let actual = Timeline::capture_scope(&reopened, chart, scope).unwrap();
            assert_eq!(actual.markers().len(), draft.attacks().events().len());
            for (marker, event) in actual.markers().iter().zip(draft.attacks().events()) {
                assert_eq!(marker.row, event.row, "case {case_index}");
                assert_eq!(marker.modifiers, event.modifiers);
            }
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), before);
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), after);
        }
    }
}
#[test]
fn transport_uses_mapped_start_and_rejects_collisions_or_unrepresentable_warp_rows_atomically() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#DELAYS:4=1;#STOPS:8=1;#ATTACKS:TIME=5.25:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;";
    let session = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert_eq!(draft.start_row(0).unwrap(), 312);
    assert_eq!(draft.attacks().events()[0].chart_time, 4.25);
    assert_eq!(draft.length_to_cursor(0, 312).unwrap(), 0.0);
    assert_eq!(draft.end_row(0, 0).unwrap(), 384);
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#WARPS:4=4;#ATTACKS:TIME=0:LEN=1:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut session = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    let value = draft.attacks().events()[0].modifiers[0].clone();
    draft.set_at_row(0, 0, 240, value.clone()).unwrap();
    let before = session.serialize().unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    draft.set_at_row(0, 0, 384, value).unwrap();
    assert!(draft.apply(&mut session, 0).unwrap());
    assert_eq!(
        Timeline::capture(&session, 0).unwrap().markers()[0].row,
        384
    );
}
#[test]
fn inspector_preserves_empty_modifiers_and_rejects_foreign_stale_invalid_or_colliding_drafts() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=0:LEN=0:MODS=:TIME=1:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut session = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(draft.attacks().events()[0].modifiers.is_empty());
    let event = &draft.attacks().events()[1];
    let time = event.time;
    let value = Modifier {
        speed: 2.0,
        ..event.modifiers[0].clone()
    };
    assert!(draft.set_at_row(1, 0, 0, value.clone()).is_err());
    assert!(!draft.has_changes());
    draft.set(1, 0, time, value).unwrap();
    draft.apply(&mut session, 0).unwrap();
    let reopened = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(reopened.attacks().events().len(), 2);
    assert!(reopened.attacks().events()[0].modifiers.is_empty());
    assert!(draft.apply(&mut session, 0).is_err());
    let mut foreign = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    assert!(reopened.apply(&mut foreign, 0).is_err());
    assert!(reopened.apply(&mut session, 1).is_err());
}

#[test]
fn attack_endpoints_and_cursor_lengths_match_unchanged_grooveauthor_ui_methods() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-placement.expected.json"
    ))
    .unwrap();
    let mut comparisons = 0;
    for (case_index, case) in cases.as_array().unwrap().iter().enumerate() {
        let format = if case["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let session = Session::new(
            Document::parse(case["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
        );
        for (chart_index, chart) in case["Charts"].as_array().unwrap().iter().enumerate() {
            let scope = if format == Format::Sm {
                Scope::Song
            } else {
                Scope::Chart
            };
            let draft = Draft::capture(&session, chart_index, scope).unwrap();
            for (event_index, event) in chart["EditorAttacks"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                for (modifier, transport) in
                    event["Transport"].as_array().unwrap().iter().enumerate()
                {
                    if let Some(row) = transport["EndRow"].as_f64() {
                        assert_eq!(
                            draft.end_row(event_index, modifier).unwrap(),
                            row.max(0.0).round() as u64,
                            "case {case_index} chart {chart_index} event {event_index} end"
                        );
                        comparisons += 1;
                    }
                    for cursor in transport["Cursors"].as_array().unwrap() {
                        let actual = draft
                            .length_to_cursor(event_index, cursor["Row"].as_u64().unwrap())
                            .unwrap();
                        let expected = cursor["Length"].as_f64().unwrap();
                        assert!(
                            (actual - expected).abs() <= 1e-12,
                            "case {case_index} chart {chart_index} event {event_index}: {actual} != {expected}"
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 168);
}
