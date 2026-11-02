use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
    timing_edit::{
        Scope,
        signature::{Draft, Signature, Signatures},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn check(map: &Signatures, expected: &Value) {
    assert_eq!(
        map.values().len(),
        expected["Nodes"].as_array().unwrap().len()
    );
    for ((row, signature), node) in map
        .values()
        .iter()
        .zip(expected["Nodes"].as_array().unwrap())
    {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(
            u64::from(signature.numerator),
            node["Numerator"].as_u64().unwrap()
        );
        assert_eq!(
            u64::from(signature.denominator),
            node["Denominator"].as_u64().unwrap()
        );
        assert_eq!(
            map.measure_position(*row).unwrap().measure,
            node["Measure"].as_u64().unwrap()
        );
    }
    for node in expected["Positions"].as_array().unwrap() {
        let position = map.measure_position(node["Row"].as_u64().unwrap()).unwrap();
        assert_eq!(position.measure, node["Measure"].as_u64().unwrap());
        assert_eq!(position.row_in_measure, node["Relative"].as_u64().unwrap());
        assert_eq!(position.rows_per_measure, node["Length"].as_u64().unwrap());
        let row = node["Row"].as_u64().unwrap();
        assert_eq!(
            map.measure_destination(row, true).unwrap() as f64,
            node["Previous"].as_f64().unwrap()
        );
        assert_eq!(
            map.measure_destination(row, false).unwrap() as f64,
            node["Next"].as_f64().unwrap()
        );
    }
}
#[test]
fn import_setter_movement_and_measure_positions_match_original_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("signature-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let source = request["Segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| format!("{}={}={}", v[0], v[1], v[2]))
            .collect::<Vec<_>>()
            .join(",");
        let mut map = Signatures::parse(&source).unwrap();
        check(&map, &expected["Initial"]);
        for (operation, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = operation[1].as_u64().unwrap();
            let result = match operation[0].as_str().unwrap() {
                "set" => {
                    Signature::parse(operation[2].as_str().unwrap()).and_then(|s| map.set(row, s))
                }
                "remove" => map.remove(row),
                "move" => map.move_node(row, operation[2].as_u64().unwrap()),
                _ => unreachable!(),
            };
            if stage["Error"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    stage["Changed"].as_bool().unwrap(),
                    "{name} {operation}"
                );
            } else {
                assert!(result.is_err(), "{name} {operation}");
            }
            check(&map, &stage["Values"]);
            assert_eq!(Signatures::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 5);
}
fn ssc() -> Vec<u8> {
    b"#VERSION:0.83;#UNKNOWN:retain;#BPMS:0=120,8=180;#OFFSET:0.125;#STOPS:1=0.25;#TIMESIGNATURES:0=4=4,4=3=4;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0000\n0000\n0000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=80;#TIMESIGNATURES:0=5=8;#NOTES:0100\n0000\n0000\n0000;".to_vec()
}
#[test]
fn inherited_chart_signature_edit_detaches_complete_timing_save_reopens_and_undoes_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, ssc()).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    file.session
        .edit_notes(
            0,
            "Pending note",
            [(
                Position {
                    row: 12,
                    lane: 2,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
    let before = file.session.serialize().unwrap();
    let mut draft = Draft::capture(&file.session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    draft.set(48, Signature::parse("7/8").unwrap()).unwrap();
    assert!(draft.apply(&mut file.session, 0).unwrap());
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            file.session.document().text(Some(0), tag),
            file.session.document().text(None, tag)
        );
    }
    assert_eq!(
        file.session
            .document()
            .text(Some(1), "TIMESIGNATURES")
            .unwrap(),
        "0=5=8"
    );
    let changed = file.session.serialize().unwrap();
    file.save().unwrap();
    let reopened = FileSession::open(&path).unwrap();
    let persisted = Draft::capture(&reopened.session, 0, Scope::Chart).unwrap();
    assert_eq!(
        persisted.signatures().at(48),
        Signature::parse("7/8").unwrap()
    );
    assert_eq!(
        reopened.session.document().text(None, "UNKNOWN").unwrap(),
        "retain"
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), before);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), changed);
}
#[test]
fn song_scope_sm_support_noop_and_foreign_stale_guards_preserve_history() {
    let mut session = Session::new(Document::parse(ssc(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(ssc(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(0, Signature::parse("2/2").unwrap()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000\n0000\n0000\n0000;";
    let mut session = Session::new(Document::parse(sm.as_slice(), Format::Sm).unwrap());
    assert!(Draft::capture(&session, 0, Scope::Chart).unwrap().inherited());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    draft.set(0, Signature::parse("3/4").unwrap()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    let reopened = Document::parse(session.serialize().unwrap(), Format::Sm).unwrap();
    assert!(reopened.raw_property(None,"TIMESIGNATURES").is_none());
    let reopened=Session::new(reopened);
    assert_eq!(Draft::capture(&reopened,0,Scope::Song).unwrap().signatures().serialize(),"0=3=4");
}
#[test]
fn invalid_signatures_collisions_and_unsafe_arithmetic_leave_maps_unchanged() {
    let mut map = Signatures::parse("0=4=4,4=5=6").unwrap();
    let before = map.clone();
    for text in [
        "4",
        "4/4/4",
        "0/4",
        "-1/4",
        "3/3",
        "3/48",
        "3/64",
        "2147483648/4",
    ] {
        assert!(Signature::parse(text).is_err());
    }
    assert!(
        map.set(48, Signature::parse("2147483647/1").unwrap())
            .is_err()
    );
    assert!(
        map.set(i32::MAX as u64 + 1, Signature::parse("3/4").unwrap())
            .is_err()
    );
    assert!(map.measure_position(u64::MAX).is_err());
    assert!(map.remove(0).is_err());
    assert!(map.move_node(0, 48).is_err());
    assert!(map.move_node(192, 0).is_err());
    assert_eq!(map, before);
    assert_eq!(
        Signatures::parse("0=4=4,// note\n4=3=4").unwrap().at(192),
        Signature::parse("3/4").unwrap()
    );
    assert!(Signatures::parse("NaN=3=4").is_err());
    assert_eq!(
        Signatures::parse("0=3=4,-0=5=8").unwrap().at(0),
        Signature::parse("5/8").unwrap()
    );
    assert_eq!(
        Signatures::parse("-0=5=8,0=3=4").unwrap().at(0),
        Signature::parse("3/4").unwrap()
    );
}

#[test]
fn visible_measure_boundaries_include_partial_measures_and_are_lazy_at_large_scale() {
    let map = Signatures::parse("0=3=4,3=7=8,6.5=2=2").unwrap();
    assert_eq!(
        map.measure_starts(0, 768).unwrap().collect::<Vec<_>>(),
        vec![(0, 0), (144, 1), (312, 2), (504, 3), (696, 4)]
    );
    assert_eq!(
        map.measure_starts(145, 504).unwrap().collect::<Vec<_>>(),
        vec![(312, 2), (504, 3)]
    );
    let map = Signatures::parse("0=4=4,1=3=4").unwrap();
    assert_eq!(
        map.measure_starts(0, 200).unwrap().collect::<Vec<_>>(),
        vec![(0, 0), (48, 1), (192, 2)]
    );
    assert!(map.measure_starts(10, 9).is_err());
    let dense = Signatures::parse("0=1=192").unwrap();
    assert_eq!(
        dense
            .measure_starts(0, i32::MAX as u64)
            .unwrap()
            .take(4)
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 1), (2, 2), (3, 3)]
    );
}
