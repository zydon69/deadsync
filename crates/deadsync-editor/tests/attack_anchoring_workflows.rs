use deadsync_editor::{
    attacks::placement::Timeline,
    document::{Document, Format},
    history::Session,
    timing_edit::{Scope, TempoDraft},
};
fn session(format: Format) -> Session {
    let notes = if format == Format::Ssc {
        "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;"
    } else {
        "#NOTES:dance-single::Hard:9::1000,0100;"
    };
    Session::new(Document::parse(format!("#VERSION:0.83;#OFFSET:0;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;#UNKNOWN:retain;{notes}").into_bytes(),format).unwrap())
}
fn rows(s: &Session, chart: usize) -> Vec<u64> {
    Timeline::capture(s, chart)
        .unwrap()
        .markers()
        .iter()
        .map(|m| m.row)
        .collect()
}
fn roundtrip(s: &Session, chart: usize, expected: &[u64]) {
    let saved = s.serialize().unwrap();
    let loaded = Session::new(Document::parse(saved, s.document().format).unwrap());
    assert_eq!(rows(&loaded, chart), expected);
    assert_eq!(
        Timeline::capture(&loaded, chart).unwrap().markers()[0].modifiers[0].length,
        2.0
    );
}
#[test]
fn tempo_and_song_offset_preserve_attack_rows_in_sm_and_ssc_as_single_undo_gestures() {
    for format in [Format::Sm, Format::Ssc] {
        let mut s = session(format);
        let original = s.serialize().unwrap();
        let expected = rows(&s, 0);
        assert_eq!(expected, [96]);
        let mut draft = TempoDraft::capture(&s, 0, Scope::Song).unwrap();
        draft.set(0, 240.0).unwrap();
        assert!(draft.apply(&mut s, 0).unwrap());
        assert_eq!(rows(&s, 0), expected);
        roundtrip(&s, 0, &expected);
        let tempo = s.serialize().unwrap();
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), original);
        s.redo().unwrap();
        assert_eq!(s.serialize().unwrap(), tempo);
        let mut draft = deadsync_editor::song_properties::Draft::capture(&s).unwrap();
        draft.set_music_offset(0.25).unwrap();
        assert!(draft.apply(&mut s).unwrap());
        assert_eq!(rows(&s, 0), expected);
        roundtrip(&s, 0, &expected);
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), tempo);
    }
}
#[test]
fn local_timing_detaches_attacks_without_moving_shared_or_other_local_charts() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180;#ATTACKS:TIME=2:LEN=2:MODS=100% Reverse;#NOTES:0010;";
    let mut s = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let original = s.serialize().unwrap();
    let expected: Vec<_> = (0..3).map(|c| rows(&s, c)).collect();
    let mut draft = TempoDraft::capture(&s, 0, Scope::Chart).unwrap();
    draft.set(0, 240.0).unwrap();
    draft.apply(&mut s, 0).unwrap();
    for c in 0..3 {
        assert_eq!(rows(&s, c), expected[c]);
        roundtrip(&s, c, &expected[c]);
    }
    assert!(s.document().raw_property(Some(0), "ATTACKS").is_some());
    assert!(s.document().raw_property(Some(1), "ATTACKS").is_none());
    assert_eq!(
        s.document().raw_property(None, "ATTACKS"),
        Some(b"TIME=1:LEN=2:MODS=100% Dizzy".as_slice())
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), original);
}
#[test]
fn chart_offset_metadata_preserves_attacks_and_undo_in_both_formats() {
    for format in [Format::Sm, Format::Ssc] {
        let mut s = session(format);
        let original = s.serialize().unwrap();
        let expected = rows(&s, 0);
        let mut draft = deadsync_editor::chart_properties::Draft::capture(&s, 0).unwrap();
        draft.set_music_offset(0.25).unwrap();
        draft.set_use_chart_offset(true);
        assert!(draft.apply(&mut s, 0).unwrap());
        assert_eq!(rows(&s, 0), expected);
        roundtrip(&s, 0, &expected);
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), original);
    }
}

#[test]
fn unrepresentable_warp_change_preserves_document_and_redo_branch() {
    use deadsync_editor::timing_edit::region::{Draft, Kind};
    let mut s = session(Format::Ssc);
    s.set_text(None, "TITLE", "Undo me").unwrap();
    s.undo().unwrap();
    let original = s.serialize().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let mut draft = Draft::capture(&s, 0, Scope::Song).unwrap();
    draft.set(Kind::Warp, 0, 192).unwrap();
    assert!(draft.apply(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), original);
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    s.redo().unwrap();
    assert_eq!(s.document().text(None, "TITLE").as_deref(), Some("Undo me"));
}
#[test]
fn existing_delays_stops_negative_stops_and_warps_preserve_reference_attack_anchors() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-placement.expected.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let format = if case["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        for chart in 0..case["Charts"].as_array().unwrap().len() {
            let mut s = Session::new(
                Document::parse(case["Content"].as_str().unwrap().as_bytes(), format).unwrap(),
            );
            let expected = rows(&s, chart);
            if expected.is_empty() {
                continue;
            }
            let before = s.serialize().unwrap();
            let mut draft = deadsync_editor::song_properties::Draft::capture(&s).unwrap();
            draft.set_music_offset(0.375).unwrap();
            draft.apply(&mut s).unwrap();
            assert_eq!(rows(&s, chart), expected);
            let saved = s.serialize().unwrap();
            let reloaded = Session::new(Document::parse(saved, format).unwrap());
            assert_eq!(rows(&reloaded, chart), expected);
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
        }
    }
}

#[test]
fn timing_transaction_uses_unsaved_hold_tails_and_preserves_note_history() {
    use deadsync_editor::notes::{Kind, Note, Position};
    let mut s = session(Format::Ssc);
    let pos = Position {
        row: 0,
        lane: 2,
        player: 0,
    };
    let note = Note {
        kind: Kind::Hold,
        end: Some(768),
        attributes: "".into(),
        tail_attributes: "".into(),
    };
    s.edit_notes(0, "Unsaved hold", [(pos, Some(note.clone()))])
        .unwrap();
    let before = s.serialize().unwrap();
    let expected = rows(&s, 0);
    let mut draft = TempoDraft::capture(&s, 0, Scope::Song).unwrap();
    draft.set(0, 240.0).unwrap();
    draft.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), expected);
    roundtrip(&s, 0, &expected);
    assert_eq!(s.grid(0).unwrap().get(pos), Some(&note));
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.undo().unwrap();
    assert!(s.grid(0).unwrap().get(pos).is_none());
}
