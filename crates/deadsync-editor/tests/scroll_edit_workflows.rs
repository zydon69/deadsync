use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope,
        scroll::{Draft, Scrolls},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn values(map: &Scrolls, expected: &Value) {
    assert_eq!(map.values().len(), expected.as_array().unwrap().len());
    for ((row, rate), node) in map.values().iter().zip(expected.as_array().unwrap()) {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(*rate, node["Rate"].as_f64().unwrap());
    }
}
#[test]
fn reader_setter_and_move_remove_sequences_match_original_scrolls() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("scroll-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let text = request["Segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| format!("{}={}", v[0], v[1]))
            .collect::<Vec<_>>()
            .join(",");
        let mut map = Scrolls::parse(&text).unwrap();
        values(&map, &expected["Initial"]);
        for (op, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => map.set(row, op[2].as_f64().unwrap()),
                "move" => map.move_node(row, op[2].as_u64().unwrap()),
                "remove" => map.remove(row),
                _ => unreachable!(),
            };
            if stage["Error"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    stage["Changed"].as_bool().unwrap(),
                    "{name} {op}"
                );
            } else {
                assert!(result.is_err());
            }
            values(&map, &stage["Values"]);
            assert_eq!(Scrolls::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 4);
}
fn source() -> Vec<u8> {
    b"#VERSION:0.83;#UNKNOWN:retain;#BPMS:0=120;#OFFSET:0.125;#STOPS:2=0.25;#SCROLLS:0=1,4=-1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#SCROLLS:0=2;#NOTES:0100;".to_vec()
}
#[test]
fn local_scroll_edit_copies_full_inherited_timing_once_and_round_trips_undo() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let original = session.serialize().unwrap();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    draft.set(0, 0.0).unwrap();
    draft.move_node(0, 48).unwrap();
    draft.set(0, -2.0).unwrap();
    draft.apply(&mut session, 0).unwrap();
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(None, tag)
        );
    }
    assert_eq!(session.document().text(Some(1), "SCROLLS").unwrap(), "0=2");
    assert_eq!(session.document().text(None, "UNKNOWN").unwrap(), "retain");
    let changed = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(changed.clone(), Format::Ssc).unwrap());
    let loaded = Draft::capture(&reopened, 0, Scope::Chart).unwrap();
    assert_eq!(loaded.scrolls().values().get(&48), Some(&0.0));
    let analyzed =
        deadsync_editor::analysis::chart(&reopened, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(
        analyzed.timing_segments.scrolls,
        vec![(0.0, -2.0), (1.0, 0.0), (4.0, -1.0)]
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
}
#[test]
fn scope_noop_history_stale_foreign_and_sm_guards_do_not_lose_data() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(48, -0.5).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = Session::new(
        Document::parse(
            b"#BPMS:0=120;#SCROLLS:0=2;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    assert!(
        Draft::capture(&sm, 0, Scope::Song)
            .unwrap()
            .scrolls()
            .values()
            .is_empty(),
        "original unsupported SM tag stays ignored"
    );
}
#[test]
fn zero_reverse_late_first_rates_and_invalid_edits_preserve_reference_semantics() {
    let mut map = Scrolls::parse("4=-0.5,8=0").unwrap();
    assert_eq!(map.at(0), -0.5);
    assert_eq!(map.at(192), -0.5);
    assert_eq!(map.at(384), 0.0);
    let original = map.clone();
    for rate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        assert!(map.set(0, rate).is_err());
    }
    assert!(map.set(u64::MAX, 1.0).is_err());
    assert!(map.move_node(192, 384).is_err());
    assert_eq!(map, original);
    map.remove(192).unwrap();
    map.remove(384).unwrap();
    assert_eq!(map.at(0), 1.0);
    assert!(map.values().is_empty());
    let mut map = Scrolls::parse("0=1").unwrap();
    assert!(!map.set(0, f64::from_bits(1.0f64.to_bits() + 1)).unwrap());
}
