use deadsync_editor::{
    analysis,
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope,
        label::{self, Draft, Labels},
    },
};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn check(map: &Labels, expected: &Value) {
    assert_eq!(map.values().len(), expected.as_array().unwrap().len());
    for ((row, text), node) in map.values().iter().zip(expected.as_array().unwrap()) {
        assert_eq!(*row, node["Row"].as_u64().unwrap());
        assert_eq!(text, node["Text"].as_str().unwrap());
    }
}
#[test]
fn actual_label_parser_original_sanitizer_and_setter_match_all_stages() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("label-edit-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        let mut map = Labels::parse(request["Source"].as_str().unwrap()).unwrap();
        check(&map, &expected["Initial"]);
        for value in expected["Sanitized"].as_array().unwrap() {
            assert_eq!(
                label::sanitize(value["Input"].as_str().unwrap()),
                value["Text"].as_str().unwrap()
            );
        }
        assert_eq!(
            request["Operations"].as_array().unwrap().len(),
            expected["Stages"].as_array().unwrap().len(),
            "{name}: every operation must have a reference stage"
        );
        for (op, stage) in request["Operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Stages"].as_array().unwrap())
        {
            let row = op[1].as_u64().unwrap();
            let result = match op[0].as_str().unwrap() {
                "set" => map.set(row, op[2].as_str().unwrap()),
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
            check(&map, &stage["Values"]);
            assert_eq!(Labels::parse(&map.serialize()).unwrap(), map);
        }
    }
    assert_eq!(count, 4);
}
fn source() -> Vec<u8> {
    b"#VERSION:0.83;#TITLE:Labels;#BPMS:0=120;#OFFSET:0.125;#STOPS:2=0.25;#LABELS:0=Legacy\\/\\/intro,4=End;#UNKNOWN:retain;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#LABELS:0=Local;#NOTES:0100;".to_vec()
}
#[test]
fn scoped_unicode_labels_keep_escaped_legacy_text_and_timing_through_rssp_and_undo() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    let mut draft = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(draft.inherited());
    assert_eq!(draft.affected_charts(), &[0]);
    assert_eq!(draft.labels().values().get(&0).unwrap(), "Legacy//intro");
    draft.set(48, "Pré-refrain 日本語 🎵").unwrap();
    draft.set(96, "a:b/#;\n,").unwrap();
    draft.apply(&mut session, 0).unwrap();
    for tag in ["BPMS", "OFFSET", "STOPS"] {
        assert_eq!(
            session.document().text(Some(0), tag),
            session.document().text(None, tag)
        );
    }
    assert_eq!(
        session.document().text(Some(1), "LABELS").unwrap(),
        "0=Local"
    );
    let changed = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(changed.clone(), Format::Ssc).unwrap());
    let labels = Labels::from_document(reopened.document(), 0).unwrap();
    assert_eq!(labels.values().get(&0).unwrap(), "Legacy//intro");
    assert_eq!(labels.values().get(&48).unwrap(), "Pré-refrain 日本語 🎵");
    assert_eq!(labels.values().get(&96).unwrap(), "a_b_____");
    assert!(labels.values().contains_key(&192));
    let summary = analysis::chart(&reopened, 0, &rssp::AnalysisOptions::default()).unwrap();
    assert!(
        summary
            .chart_labels
            .unwrap()
            .contains("Pré-refrain 日本語 🎵")
    );
    assert_eq!(reopened.document().text(None, "UNKNOWN").unwrap(), "retain");
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), changed);
}
#[test]
fn noops_stale_foreign_scope_and_sm_export_guards_preserve_data() {
    let mut session = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert_eq!(draft.affected_charts(), &[0]);
    let mut foreign = Session::new(Document::parse(source(), Format::Ssc).unwrap());
    assert!(draft.apply(&mut foreign, 0).is_err());
    assert!(draft.apply(&mut session, 1).is_err());
    draft.set(48, "").unwrap();
    draft.apply(&mut session, 0).unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    session.undo().unwrap();
    let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
    assert!(!unchanged.apply(&mut session, 0).unwrap());
    assert!(session.redo_label().is_some());
    let sm = Session::new(
        Document::parse(
            b"#BPMS:0=120;#LABELS:0=Legacy;#NOTES:dance-single::Hard:9::1000;".as_slice(),
            Format::Sm,
        )
        .unwrap(),
    );
    let bytes = sm.serialize().unwrap();
    assert!(Draft::capture(&sm, 0, Scope::Song).is_err());
    assert!(
        Labels::from_document(sm.document(), 0)
            .unwrap()
            .values()
            .is_empty()
    );
    assert_eq!(sm.serialize().unwrap(), bytes);
}
#[test]
fn unrepresentable_equals_and_invalid_coordinates_do_not_lose_labels() {
    let mut labels = Labels::parse("0=A,1=,2=bad=record,3= tail ").unwrap();
    assert_eq!(labels.values().get(&48).unwrap(), "");
    let before = labels.clone();
    assert!(labels.set(0, "A=B").is_err());
    assert!(labels.set(u64::MAX, "B").is_err());
    assert!(labels.move_node(0, 48).is_err());
    assert_eq!(labels, before);
    labels.move_node(0, 24).unwrap();
    labels.remove(24).unwrap();
    assert!(!labels.remove(24).unwrap());
    assert!(Labels::parse("NaN=A").is_err());
    assert_eq!(
        Labels::parse("0=A,-0=B").unwrap().values().get(&0).unwrap(),
        "B"
    );
}
