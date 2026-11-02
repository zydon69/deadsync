use deadsync_editor::region_selection::misc_layout::{InvalidGeometry, Kind, Layout};
use serde_json::Value;
fn kind(index: u64) -> Kind {
    [
        Kind::Signature,
        Kind::Stop,
        Kind::Delay,
        Kind::Warp,
        Kind::Tick,
        Kind::Combo,
        Kind::Label,
        Kind::Attack,
        Kind::Preview,
        Kind::EndHint,
        Kind::Pattern,
        Kind::Tempo,
        Kind::Scroll,
        Kind::Speed,
        Kind::Fake,
    ][index as usize]
}
#[test]
fn actual_source_layout_matches_every_insertion_and_replacement() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/misc-layout-source.json"
    ))
    .unwrap();
    for case in fixtures.as_array().unwrap() {
        let mut layout =
            Layout::new(-130.0, 130.0, case["width"].as_i64().unwrap() as i32, 2.0).unwrap();
        for (i, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            layout
                .position(
                    i as u64,
                    kind(case["order"][i].as_u64().unwrap()),
                    0.0,
                    200.0,
                    20.0 + (i % 3) as f64 * 17.0,
                    20.0 + (i % 2) as f64 * 2.0,
                )
                .unwrap();
            for expected in step.as_array().unwrap() {
                let actual = layout.rectangle(expected["id"].as_u64().unwrap()).unwrap();
                for (name, value) in [
                    ("x", actual.x),
                    ("y", actual.y),
                    ("w", actual.width),
                    ("h", actual.height),
                ] {
                    assert_eq!(
                        value,
                        expected[name].as_f64().unwrap(),
                        "{case} step{i} id{} {name}",
                        expected["id"]
                    );
                }
            }
        }
        assert_eq!(
            layout.maximum_single_row_height(20.0).unwrap(),
            case["maxY"].as_f64().unwrap()
        );
        layout.clear();
        assert_eq!(layout.rectangles().count(), 0);
        layout
            .position(999, Kind::Signature, 0.0, 200.0, 40.0, 20.0)
            .unwrap();
        let reset = layout.rectangle(999).unwrap();
        assert_eq!(reset.x, case["reset"]["x"].as_f64().unwrap());
        assert_eq!(reset.y, case["reset"]["y"].as_f64().unwrap());
    }
}
#[test]
fn opposite_sides_and_distinct_keys_do_not_displace_each_other() {
    let mut layout = Layout::new(-130.0, 130.0, 35, 2.0).unwrap();
    layout
        .position(1, Kind::Stop, -0.0, 200.0, 30.0, 20.0)
        .unwrap();
    let left = layout.rectangle(1).unwrap();
    layout
        .position(2, Kind::Tempo, 0.0, 200.0, 30.0, 20.0)
        .unwrap();
    assert_eq!(layout.rectangle(1), Some(left));
    layout
        .position(3, Kind::Signature, 1.0, 300.0, 30.0, 20.0)
        .unwrap();
    assert_eq!(layout.rectangle(1), Some(left));
    layout
        .position(4, Kind::Signature, 0.0, 200.0, 30.0, 20.0)
        .unwrap();
    assert_ne!(
        layout.rectangle(1),
        Some(left),
        "negative zero shares a source row with positive zero"
    );
}
#[test]
fn invalid_geometry_changes_neither_rectangles_nor_row_references() {
    let mut layout = Layout::new(-130.0, 130.0, 100, 2.0).unwrap();
    layout
        .position(1, Kind::Stop, 0.0, 200.0, 30.0, 20.0)
        .unwrap();
    let before: Vec<_> = layout.rectangles().collect();
    assert_eq!(
        layout.position(2, Kind::Signature, f64::NAN, 200.0, 30.0, 20.0),
        Err(InvalidGeometry::NonFinite)
    );
    assert_eq!(
        layout.position(2, Kind::Signature, 0.0, 200.0, 0.0, 20.0),
        Err(InvalidGeometry::NonPositiveSize)
    );
    assert_eq!(layout.rectangles().collect::<Vec<_>>(), before);
}
