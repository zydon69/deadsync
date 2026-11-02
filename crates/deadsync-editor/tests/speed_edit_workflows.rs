use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope,
        speed::{Draft, Period, Speed, Speeds},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn check(map: &Speeds, expected: &Value) {
    let nodes = expected["Nodes"].as_array().unwrap();
    assert_eq!(map.values().len(), nodes.len());
    for ((row, value), node) in map.values().iter().zip(nodes) {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(value.rate, node["Rate"].as_f64().unwrap());
        match value.period {
            Period::Rows(rows) => {
                assert!(!node["Time"].as_bool().unwrap());
                assert_eq!(i64::from(rows), node["Rows"].as_i64().unwrap());
            }
            Period::Seconds(seconds) => {
                assert!(node["Time"].as_bool().unwrap());
                assert_eq!(seconds, node["Seconds"].as_f64().unwrap());
            }
        }
    }
    for query in expected["Queries"].as_array().unwrap() {
        let row = query["Row"].as_f64().unwrap();
        let time = query["Time"].as_f64().unwrap();
        assert_eq!(
            map.rate_at(row, time, |start| Ok(start as f64 / 96.0))
                .unwrap(),
            query["Rate"].as_f64().unwrap(),
            "row {row} time {time}"
        );
    }
}
#[test]
fn actual_speed_parser_original_setter_and_interpolation_match_all_stages() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("speed-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut map = Speeds::parse(request["Source"].as_str().unwrap()).unwrap();
        check(&map, &expected["Initial"]);
        for (op, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => Speed::parse(op[2].as_str().unwrap()).and_then(|v| map.set(row, v)),
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
            assert_eq!(Speeds::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 5);
}
fn source() -> Vec<u8> {
    b"#VERSION:0.83;#TITLE:Speeds;#BPMS:0=120;#OFFSET:0.125;#STOPS:2=0.25;#SPEEDS:0=1=0=0,4=-1=0.5=1;#UNKNOWN:retain;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#SPEEDS:0=2=0=0;#NOTES:0100;".to_vec()
}
#[test]
fn inherited_speed_changes_keep_both_units_signed_source_and_rssp_readback_through_undo() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    draft.set(0, Speed::parse("2x/48rows").unwrap()).unwrap();
    draft.set(48, Speed::parse("0x/.25s").unwrap()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(None, tag)
        );
    }
    assert_eq!(
        session.document().text(Some(1), "SPEEDS").unwrap(),
        "0=2=0=0"
    );
    let changed = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(changed.clone(), Format::Ssc).unwrap());
    let summary = analysis::chart(&reopened, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert_eq!(
        summary.chart_speeds.as_deref(),
        Some("0=2=1=0,1=0=0.25=1,4=-1=0.5=1")
    );
    assert_eq!(reopened.document().text(None, "UNKNOWN").unwrap(), "retain");
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
}
#[test]
fn scope_noop_foreign_stale_and_sm_source_eligibility_preserve_history_and_source() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(48, Speed::default()).unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = Session::new(
        Document::parse(
            b"#BPMS:0=120;#SPEEDS:0=2=1=0;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let before = sm.serialize().unwrap();
    assert!(
        Draft::capture(&sm, 0, Scope::Song)
            .unwrap()
            .speeds()
            .values()
            .is_empty(),
        "original unsupported SM tag stays ignored"
    );
    assert_eq!(sm.serialize().unwrap(), before);
}
#[test]
fn interpolation_units_instant_transitions_and_unsafe_queries_are_explicit() {
    let rows = Speed::parse("3x/48rows").unwrap();
    assert_eq!(rows.interpolate(1.0, 48, 99.0, 72.0, 99.0).unwrap(), 2.0);
    let seconds = Speed::parse("3x/2s").unwrap();
    assert_eq!(seconds.interpolate(1.0, 48, 10.0, 48.0, 11.0).unwrap(), 2.0);
    assert_eq!(
        Speed::parse("0x/0rows")
            .unwrap()
            .interpolate(1.0, 48, 0.5, 48.0, 0.5)
            .unwrap(),
        0.0
    );
    let map = Speeds::parse("0=1=0=0,1=3=4=0,2=0.5=2=0").unwrap();
    assert_eq!(
        map.rate_at(96.0, 1.0, |_| panic!("Rows need no clock lookup"))
            .unwrap(),
        3.0
    );
    let mut map = Speeds::parse("0=1=0=0").unwrap();
    let before = map.clone();
    assert!(map.set(i32::MAX as u64, rows).is_err());
    assert!(
        map.set(
            0,
            Speed {
                rate: f64::INFINITY,
                period: Period::Rows(0)
            }
        )
        .is_err()
    );
    assert!(map.move_node(0, u64::MAX).is_err());
    assert_eq!(map, before);
    assert!(map.rate_at(f64::NAN, 0.0, |_| Ok(0.0)).is_err());
    for text in [
        "-1x/1s",
        "2x/-1s",
        "2x/1.5rows",
        "2e0x/1s",
        "2x/2147483648rows",
    ] {
        assert!(Speed::parse(text).is_err());
    }
    let precise = Speed::parse("1.123456789012345x/0.123456789012345s").unwrap();
    assert_eq!(Speed::parse(&precise.to_string()).unwrap(), precise);
}
