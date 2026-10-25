use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope,
        tick::{Draft, TickCounts},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn check(map: &TickCounts, expected: &Value) {
    assert_eq!(map.values().len(), expected.as_array().unwrap().len());
    for ((row, value), node) in map.values().iter().zip(expected.as_array().unwrap()) {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(i64::from(*value), node["Ticks"].as_i64().unwrap());
    }
}
#[test]
fn source_import_setter_and_mutable_initial_nodes_match_original_tick_events() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("tick-edit-") || !name.ends_with(".request.json") {
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
        let mut map = TickCounts::parse(&text).unwrap();
        check(&map, &expected["Initial"]);
        for (op, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => map.set(row, op[2].as_i64().unwrap() as i32),
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
                assert!(result.is_err(), "{name} {op}");
            }
            check(&map, &stage["Values"]);
            assert_eq!(TickCounts::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 4);
}
fn source() -> Vec<u8> {
    b"#VERSION:0.83;#TITLE:Ticks;#UNKNOWN:retain;#BPMS:0=120;#OFFSET:0.125;#STOPS:2=0.25;#TICKCOUNTS:0=4,4=-1;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#TICKCOUNTS:0=16;#NOTES:0100;".to_vec()
}
#[test]
fn inherited_tick_edit_detaches_all_timing_and_preserves_signed_source_through_rssp_and_undo() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    draft.set(0, 0).unwrap();
    draft.set(48, 8).unwrap();
    draft.apply(&mut session, 0).unwrap();
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(None, tag)
        );
    }
    assert_eq!(
        session.document().text(Some(1), "TICKCOUNTS").unwrap(),
        "0=16"
    );
    let changed = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(changed.clone(), Format::Ssc).unwrap());
    assert_eq!(
        Draft::capture(&reopened, 0, Scope::Chart)
            .unwrap()
            .counts()
            .values()
            .get(&192),
        Some(&-1)
    );
    let summary = analysis::chart(&reopened, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(summary.chart_tickcounts.as_deref(), Some("0=0,1=8,4=-1"));
    assert_eq!(reopened.document().text(None, "UNKNOWN").unwrap(), "retain");
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
}
#[test]
fn shared_sm_ticks_and_stale_foreign_noop_history_guards_work_without_format_conversion() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(48, 16).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    let mut session = Session::new(Document::parse(sm.as_slice(), Format::Sm).unwrap());
    assert!(Draft::capture(&session, 0, Scope::Chart).is_err());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    draft.set(0, 0).unwrap();
    draft.set(48, 12).unwrap();
    draft.apply(&mut session, 0).unwrap();
    let reopened = Document::parse(session.serialize().unwrap(), Format::Sm).unwrap();
    assert_eq!(reopened.text(None, "TICKCOUNTS").unwrap(), "0=0,1=12");
}
#[test]
fn integer_input_limits_legacy_negative_moves_and_source_lexical_filtering_are_explicit() {
    let mut map = TickCounts::parse("0=-3,1=2.5,2=2147483648,3=+8,// keep\n4=0").unwrap();
    assert_eq!(map.values().len(), 3);
    assert_eq!(map.values().get(&144), Some(&8));
    let before = map.clone();
    assert!(map.set(0, -1).is_err());
    assert!(map.set(u64::MAX, 4).is_err());
    assert!(map.move_node(0, 144).is_err());
    assert_eq!(map, before);
    map.move_node(0, 48).unwrap();
    assert_eq!(map.values().get(&48), Some(&-3));
    map.remove(48).unwrap();
    assert!(!map.remove(48).unwrap());
    assert!(map.set(0, i32::MAX).unwrap());
    assert!(TickCounts::parse("NaN=4").is_err());
    assert_eq!(
        TickCounts::parse("0=3,-0=5").unwrap().values().get(&0),
        Some(&5)
    );
}
