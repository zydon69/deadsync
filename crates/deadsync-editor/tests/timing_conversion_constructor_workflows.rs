use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    timing_edit::{
        conversion::{Direction, Plan},
        region::{Kind, Regions},
    },
};
use std::collections::BTreeSet;
#[test]
fn unchanged_original_warp_constructor_clamps_rounded_zero_to_one_row() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/timing-conversion-constructor.expected.json"
    ))
    .unwrap();
    let case = reference["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Name"] == "stop-tiny-source-constructor")
        .unwrap();
    assert!(
        case["After"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["Kind"] == "Warp" && e["Row"] == 48 && e["Value"] == 1)
    );
    let mut s=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=60;#STOPS:1=-0.00001;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),Format::Ssc).unwrap());
    let before = s.serialize().unwrap();
    let plan = Plan::prepare(&s, 0, Direction::NegativeStopsToWarps, None).unwrap();
    plan.apply(&mut s, &BTreeSet::new()).unwrap();
    let after = s.serialize().unwrap();
    let reopened = Session::new(Document::parse(after.clone(), Format::Ssc).unwrap());
    let warps = Regions::parse(
        Kind::Warp,
        &reopened.document().text(None, "WARPS").unwrap(),
    )
    .unwrap();
    assert_eq!(warps.values().get(&48), Some(&1));
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn original_covering_all_generates_zero_stop_and_native_refuses_without_half_publication() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/timing-conversion-constructor.expected.json"
    ))
    .unwrap();
    let case = reference["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Name"] == "warp-covering-all")
        .unwrap();
    assert!(
        case["After"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["Kind"] == "Stop" && e["Row"] == 96 && e["Value"] == 0)
    );
    let s=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=60;#WARPS:1=3,2=1;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),Format::Ssc).unwrap());
    let before = s.serialize().unwrap();
    assert!(Plan::prepare(&s, 0, Direction::WarpsToNegativeStops, None).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.undo_label().is_none());
}
