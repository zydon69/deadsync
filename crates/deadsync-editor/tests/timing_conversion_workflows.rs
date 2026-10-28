use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    timing_edit::{
        conversion::{Direction, Plan},
        pause::{Kind as PauseKind, Pauses},
        region::{Kind as RegionKind, Regions},
    },
};
use std::collections::{BTreeMap, BTreeSet};
fn session(case: &serde_json::Value) -> Session {
    let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for event in case["Events"].as_array().unwrap() {
        let row = event["Row"].as_u64().unwrap() as f64 / 48.0;
        let value = event["Value"].as_f64().unwrap();
        let (tag, value) = match event["Kind"].as_str().unwrap() {
            "Signature" => ("TIMESIGNATURES", format!("{row}=4=4")),
            "Tempo" => ("BPMS", format!("{row}={value}")),
            "Stop" => ("STOPS", format!("{row}={value}")),
            "Delay" => ("DELAYS", format!("{row}={value}")),
            "Warp" => ("WARPS", format!("{row}={}", value / 48.0)),
            _ => panic!("kind"),
        };
        groups.entry(tag).or_default().push(value);
    }
    let mut source = String::from(
        "#VERSION:0.83;#TITLE:Timing;#VENDOR:preserve;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};",
    );
    for (tag, values) in groups {
        source += &format!("#{tag}:{};", values.join(","));
    }
    source += "#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Keep;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#DESCRIPTION:Peer;#NOTES:00000001;";
    Session::new(Document::parse(source.into_bytes(), Format::Ssc).unwrap())
}
fn assert_events(s: &Session, expected: &serde_json::Value) {
    let stops = Pauses::parse(
        PauseKind::Stop,
        &s.document().text(None, "STOPS").unwrap_or_default(),
    )
    .unwrap();
    let warps = Regions::parse(
        RegionKind::Warp,
        &s.document().text(None, "WARPS").unwrap_or_default(),
    )
    .unwrap();
    let mut expected_stops = BTreeMap::new();
    let mut expected_warps = BTreeMap::new();
    for event in expected.as_array().unwrap() {
        let row = event["Row"].as_u64().unwrap();
        match event["Kind"].as_str().unwrap() {
            "Stop" => {
                expected_stops.insert(row, event["Value"].as_f64().unwrap());
            }
            "Warp" => {
                expected_warps.insert(row, event["Value"].as_u64().unwrap());
            }
            _ => {}
        }
    }
    assert_eq!(stops.values(), &expected_stops);
    assert_eq!(warps.values(), &expected_warps);
}
#[test]
fn original_23_actions_match_except_atomic_refusal_of_unrepresentable_zero_stop() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/timing-conversion.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/timing-conversion.expected.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut refused = 0;
    for (case, oracle) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        let mut s = session(case);
        let before = s.serialize().unwrap();
        let revision = s.revision();
        let direction = if case["Direction"] == "WarpToStop" {
            Direction::WarpsToNegativeStops
        } else {
            Direction::NegativeStopsToWarps
        };
        let selected: Vec<_> = case["Events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["Selected"].as_bool().unwrap_or(true))
            .map(|event| event["Row"].as_u64().unwrap())
            .collect();
        let p = Plan::prepare(
            &s,
            0,
            direction,
            if case["All"] == true {
                None
            } else {
                Some(&selected)
            },
        );
        if oracle["After"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["Kind"] == "Stop" && e["Value"].as_f64() == Some(0.0))
        {
            let message = p.err().expect("zero stop refused").to_string();
            assert!(message.contains("zero-length Stop"));
            assert_eq!(s.serialize().unwrap(), before);
            assert_eq!(s.revision(), revision);
            refused += 1;
            continue;
        }
        let p = p.unwrap_or_else(|e| panic!("{}: {e}", case["Name"]));
        let collisions = p.report().deleted_on_collision.clone();
        if !collisions.is_empty() {
            assert!(p.apply(&mut s, &BTreeSet::new()).is_err());
            assert_eq!(s.serialize().unwrap(), before);
        }
        let changed = p.apply(&mut s, &collisions).unwrap();
        assert_events(&s, &oracle["After"]);
        let after = s.serialize().unwrap();
        let reopened = Session::new(Document::parse(after.clone(), Format::Ssc).unwrap());
        assert_events(&reopened, &oracle["After"]);
        assert!(String::from_utf8_lossy(&after).contains("#VENDOR:preserve;"));
        if changed {
            s.undo().unwrap();
            assert_eq!(s.serialize().unwrap(), before);
            s.redo().unwrap();
            assert_eq!(s.serialize().unwrap(), after);
        } else {
            assert_eq!(before, after);
            assert_eq!(revision, s.revision());
        }
        matched += 1;
    }
    assert_eq!((matched, refused), (22, 1));
}
#[test]
fn stale_foreign_incomplete_review_and_zero_stop_preserve_existing_redo() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/timing-conversion.request.json"
    ))
    .unwrap();
    let case = cases["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Name"] == "warp-stop-collision-deletes-original")
        .unwrap();
    let mut s = session(case);
    let p = Plan::prepare(&s, 0, Direction::WarpsToNegativeStops, None).unwrap();
    let mut foreign = session(case);
    assert!(
        p.apply(&mut foreign, &p.report().deleted_on_collision)
            .is_err()
    );
    s.set_text(None, "TITLE", "redo").unwrap();
    let redo = s.serialize().unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    assert!(p.apply(&mut s, &BTreeSet::new()).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), redo);
    assert!(p.apply(&mut s, &p.report().deleted_on_collision).is_err());
    assert_eq!(s.serialize().unwrap(), redo);
    let zero = cases["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Name"] == "warp-selected-inside-unselected")
        .unwrap();
    let mut s = session(zero);
    s.set_text(None, "TITLE", "redo").unwrap();
    let redo = s.serialize().unwrap();
    s.undo().unwrap();
    let before = s.serialize().unwrap();
    // Source All first emits Stop48=-3, then Stop96=0. Both all and
    // selected-inner operations are therefore atomically refused.
    assert!(Plan::prepare(&s, 0, Direction::WarpsToNegativeStops, None).is_err());
    let selected = [96];
    assert!(Plan::prepare(&s, 0, Direction::WarpsToNegativeStops, Some(&selected)).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), redo);
}
#[test]
fn chart_local_timing_does_not_change_song_or_peer_timing() {
    let source = "#VERSION:0.83;#TITLE:Scoped;#BPMS:0=120;#WARPS:1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=60;#WARPS:2=1;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#NOTES:00000001;";
    let mut s = Session::new(Document::parse(source.as_bytes(), Format::Ssc).unwrap());
    let before = s.serialize().unwrap();
    let p = Plan::prepare(&s, 0, Direction::WarpsToNegativeStops, None).unwrap();
    assert!(p.apply(&mut s, &BTreeSet::new()).unwrap());
    assert_eq!(s.document().text(None, "WARPS").as_deref(), Some("1=1"));
    assert_eq!(s.document().text(Some(0), "STOPS").as_deref(), Some("2=-1"));
    assert_eq!(s.document().text(Some(1), "STOPS"), None);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
