use deadsync_editor::{
    document::{Document, Format},
    generation::{
        metadata::{ChartData, DYNAMIC, Definition, Guid, PATTERN_BALANCED, SIXTEENTHS},
        profile_references::Reset,
        profiles::Kind,
    },
    history::Session,
};
use std::{collections::BTreeMap, sync::Arc};
fn source() -> Guid {
    "12345678-9abc-4def-8123-456789abcdef".parse().unwrap()
}
fn session(format: Format) -> Session {
    let mut bytes = b"#TITLE:Preserve;#VENDOR:keep;#BPMS:0=120;".to_vec();
    for _ in 0..3 {
        bytes.extend_from_slice(match format {
            Format::Sm => b"#NOTES:dance-single:source:Hard:9:0,0,0,0,0:1000;",
            Format::Ssc => b"#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        });
    }
    Session::new(Document::parse(Arc::<[u8]>::from(bytes), format).unwrap())
}
fn data() -> ChartData {
    ChartData {
        expressed_chart_config: source(),
        patterns: Some(BTreeMap::from([
            (
                0,
                Definition {
                    pattern_config_guid: source(),
                    performed_chart_config_guid: source(),
                    length: 48,
                    start_position_inclusive: true,
                    end_position_inclusive: true,
                    extra: BTreeMap::from([("VendorMarker".into(), serde_json::json!("keep"))]),
                    ..Default::default()
                },
            ),
            (
                96,
                Definition {
                    pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                    performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                    length: 48,
                    start_position_inclusive: true,
                    end_position_inclusive: true,
                    ..Default::default()
                },
            ),
        ])),
        extra: BTreeMap::from([("VendorChart".into(), serde_json::json!({"keep":true}))]),
        ..Default::default()
    }
}
#[test]
fn all_profile_families_reset_every_chart_in_one_exactly_undoable_action() {
    for format in [Format::Sm, Format::Ssc] {
        for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
            let mut s = session(format);
            data().save(&mut s, 0).unwrap();
            data().save(&mut s, 1).unwrap();
            let before = s.serialize().unwrap();
            let revision = s.revision();
            let plan = Reset::capture(&s, kind, &source()).unwrap();
            assert_eq!(plan.affected_charts(), 2);
            assert_eq!(plan.affected_references(), 2);
            assert!(plan.apply(&mut s).unwrap());
            assert!(s.revision() > revision);
            let after = s.serialize().unwrap();
            for chart in 0..2 {
                let got = ChartData::read(s.document(), chart).unwrap().unwrap();
                let mut expected = data();
                match kind {
                    Kind::Pattern => {
                        expected
                            .patterns
                            .as_mut()
                            .unwrap()
                            .get_mut(&0)
                            .unwrap()
                            .pattern_config_guid = SIXTEENTHS.parse().unwrap()
                    }
                    Kind::Performed => {
                        expected
                            .patterns
                            .as_mut()
                            .unwrap()
                            .get_mut(&0)
                            .unwrap()
                            .performed_chart_config_guid =
                            "6276c906-ea8f-43b3-9500-0ddeac7bdc22".parse().unwrap()
                    }
                    Kind::Expressed => expected.expressed_chart_config = DYNAMIC.parse().unwrap(),
                }
                assert_eq!(
                    serde_json::to_value(got).unwrap(),
                    serde_json::to_value(expected).unwrap()
                );
            }
            assert!(ChartData::read(s.document(), 2).unwrap().is_none());
            assert!(s.undo().unwrap());
            assert_eq!(s.serialize().unwrap(), before);
            assert!(s.redo().unwrap());
            assert_eq!(s.serialize().unwrap(), after);
            let empty = Reset::capture(&s, kind, &source()).unwrap();
            assert_eq!(empty.affected_references(), 0);
            let revision = s.revision();
            assert!(!empty.apply(&mut s).unwrap());
            assert_eq!(s.revision(), revision);
        }
    }
}
#[test]
fn stale_plans_and_builtins_are_rejected_without_touching_history() {
    let mut s = session(Format::Ssc);
    data().save(&mut s, 0).unwrap();
    let plan = Reset::capture(&s, Kind::Pattern, &source()).unwrap();
    data().save(&mut s, 1).unwrap();
    let before = s.serialize().unwrap();
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    let other_plan = Reset::capture(&s, Kind::Pattern, &source()).unwrap();
    let mut other =
        Session::new(Document::parse(Arc::<[u8]>::from(before.clone()), Format::Ssc).unwrap());
    assert!(other_plan.apply(&mut other).is_err());
    assert_eq!(other.serialize().unwrap(), before);
    for (kind, id) in [
        (Kind::Pattern, SIXTEENTHS),
        (Kind::Performed, PATTERN_BALANCED),
        (Kind::Expressed, DYNAMIC),
    ] {
        assert!(Reset::capture(&s, kind, &id.parse().unwrap()).is_err());
    }
    assert!(s.undo().unwrap());
    assert!(ChartData::read(s.document(), 1).unwrap().is_none());
}
#[test]
fn malformed_later_chart_prevents_any_reference_rewrite() {
    let mut s = session(Format::Ssc);
    data().save(&mut s, 0).unwrap();
    let mut bytes = s.serialize().unwrap();
    let needle = b"#NOTEDATA:";
    let second = bytes
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .nth(1)
        .unwrap()
        .0;
    bytes.splice(second..second, b"#VENDOR:unrelated;".iter().copied());
    // The third chart has incomplete metadata; even valid earlier repairs must
    // remain staged until the whole song has been inspected.
    bytes.extend_from_slice(b"#GAChartDataVersion:1;");
    let s = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    let before = s.serialize().unwrap();
    assert!(Reset::capture(&s, Kind::Pattern, &source()).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.undo_label().is_none());
}
