use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope,
        combo::{Combos, Draft, Multipliers},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn check(map: &Combos, expected: &Value) {
    assert_eq!(map.values().len(), expected.as_array().unwrap().len());
    for ((row, value), node) in map.values().iter().zip(expected.as_array().unwrap()) {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(i64::from(value.hit), node["Hit"].as_i64().unwrap());
        assert_eq!(i64::from(value.miss), node["Miss"].as_i64().unwrap());
        assert_eq!(value.to_string(), node["Text"].as_str().unwrap());
    }
}
#[test]
fn real_fumen_property_parser_and_original_combo_setter_match_each_edit_stage() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("combo-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut map = Combos::parse(request["Source"].as_str().unwrap()).unwrap();
        check(&map, &expected["Initial"]);
        for (op, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => Multipliers::parse(op[2].as_str().unwrap())
                    .and_then(|value| map.set(row, value)),
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
            assert_eq!(Combos::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 4);
}
fn source() -> Vec<u8> {
    b"#VERSION:0.83;#TITLE:Combos;#UNKNOWN:retain;#BPMS:0=120;#OFFSET:0.125;#STOPS:2=0.25;#COMBOS:0=1,4=-2=3;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#COMBOS:0=3=4;#NOTES:0100;".to_vec()
}
#[test]
fn local_combo_edits_copy_inherited_timing_and_round_trip_through_rssp_and_undo() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    draft.set(0, Multipliers::parse("0x/2x").unwrap()).unwrap();
    draft.set(48, Multipliers::parse("3x/4x").unwrap()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(None, tag)
        );
    }
    assert_eq!(session.document().text(Some(1), "COMBOS").unwrap(), "0=3=4");
    assert_eq!(session.document().text(None, "UNKNOWN").unwrap(), "retain");
    let changed = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(changed.clone(), Format::Ssc).unwrap());
    let summary = analysis::chart(&reopened, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(summary.chart_combos.as_deref(), Some("0=0=2,1=3=4,4=-2=3"));
    let parsed = Draft::capture(&reopened, 0, Scope::Chart).unwrap();
    assert_eq!(
        parsed.combos().values().get(&192),
        Some(&Multipliers { hit: -2, miss: 3 })
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
}
#[test]
fn scope_guards_noop_history_and_sm_source_eligibility_preserve_original_bytes() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(48, Multipliers::default()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = Session::new(
        Document::parse(
            b"#BPMS:0=120;#COMBOS:0=2=3;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let bytes = sm.serialize().unwrap();
    assert!(
        Draft::capture(&sm, 0, Scope::Song)
            .unwrap()
            .combos()
            .values()
            .is_empty(),
        "original unsupported SM tag stays ignored"
    );
    assert_eq!(sm.serialize().unwrap(), bytes);
}
#[test]
fn input_signed_source_movement_and_limits_fail_without_partial_mutation() {
    let mut map = Combos::parse("0=-2=3,4=5").unwrap();
    let before = map.clone();
    assert!(map.set(0, Multipliers { hit: -1, miss: 0 }).is_err());
    assert!(map.set(u64::MAX, Multipliers::default()).is_err());
    assert!(map.move_node(0, 192).is_err());
    assert_eq!(map, before);
    map.move_node(0, 48).unwrap();
    assert_eq!(
        map.values().get(&48),
        Some(&Multipliers { hit: -2, miss: 3 })
    );
    map.remove(48).unwrap();
    assert!(!map.remove(48).unwrap());
    assert_eq!(
        Multipliers::parse("2147483647x/2147483647x").unwrap(),
        Multipliers {
            hit: i32::MAX,
            miss: i32::MAX
        }
    );
    assert!(Multipliers::parse("2147483648x/1x").is_err());
    assert_eq!(
        Combos::parse("0=2,-0=3=4").unwrap().values().get(&0),
        Some(&Multipliers { hit: 3, miss: 4 })
    );
    assert!(Combos::parse("NaN=2=3").is_err());
}
