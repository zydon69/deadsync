use deadsync_editor::Error;
use deadsync_editor::minimap_mount;
use deadsync_editor::minimap_position_preferences;
use minimap_mount::{Bounds, FocusEdges, Input, Padding, Rect};
use minimap_position_preferences::{Mount, Preferences, SelectMode};
#[test]
fn unchanged_original_mount_bounds_match_576_csharp_cases() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-mount.reference.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 576);
    for (index, case) in cases.iter().enumerate() {
        let i = &case["input"];
        let e = &case["expected"];
        let mut p = Preferences::read(
            serde_json::json!({"MiniMapPosition":i["mount"]})
                .as_object()
                .unwrap(),
        )
        .unwrap();
        p.set_offset(p.mount, i["offset"].as_i64().unwrap() as i32);
        let scale = i["scale"].as_f64().unwrap() as f32;
        let input = Input {
            chart_area: Rect {
                x: i["area"][0].as_i64().unwrap() as i32,
                y: i["area"][1].as_i64().unwrap() as i32,
                width: i["area"][2].as_i64().unwrap() as i32,
                height: i["area"][3].as_i64().unwrap() as i32,
            },
            viewport_height: i["viewport"].as_i64().unwrap() as i32,
            focus: i["focused"].as_bool().unwrap().then(|| FocusEdges {
                without_scale: i["edge"].as_i64().unwrap() as i32,
                with_scale: i["scaledEdge"].as_i64().unwrap() as i32,
            }),
            padding: Padding::original(scale).unwrap(),
            dpi_scale: scale,
        };
        let actual = minimap_mount::bounds(p, i["width"].as_u64().unwrap() as u32, input).unwrap();
        let expected = Bounds {
            x: e["x"].as_i64().unwrap() as i32,
            y: e["y"].as_i64().unwrap() as i32,
            width: e["width"].as_i64().unwrap() as i32,
            texture_height: e["textureHeight"].as_i64().unwrap() as i32,
            visible_height: e["visibleHeight"].as_u64().unwrap() as u32,
        };
        assert_eq!(actual, expected, "case{index}");
    }
}
#[test]
fn original_per_mount_defaults_partial_offsets_and_unknown_values_roundtrip() {
    let p = Preferences::default();
    assert_eq!(p.mount, Mount::LeftSideOfWindow);
    assert_eq!(p.select_mode, SelectMode::MoveToCursor);
    assert_eq!(p.offset(Mount::RightSideOfWindow), 110);
    assert_eq!(p.offset(Mount::LeftSideOfWindow), 110);
    assert_eq!(p.offset(Mount::FocusedChartWithoutScaling), 10);
    assert_eq!(p.offset(Mount::FocusedChartWithScaling), 10);
    let mut fields=serde_json::json!({"MiniMapPosition":"FocusedChartWithScaling","MiniMapSelectMode":"MoveToSelectedArea","PositionOffsets":{"FocusedChartWithScaling":-25,"FutureMount":99},"Unknown":42}).as_object().unwrap().clone();
    let p = Preferences::read(&fields).unwrap();
    assert_eq!(p.offset(p.mount), -25);
    assert_eq!(p.offset(Mount::LeftSideOfWindow), 110);
    assert_eq!(p.select_mode, SelectMode::MoveToSelectedArea);
    p.write(&mut fields);
    assert_eq!(fields["PositionOffsets"]["FutureMount"], 99);
    assert_eq!(fields["Unknown"], 42);
    assert!(fields["PositionOffsets"]["FocusedChartWithScaling"].is_i64());
    assert_eq!(Preferences::read(&fields).unwrap(), p);
}
#[test]
fn invalid_enums_restore_defaults_and_malformed_offset_data_is_rejected() {
    let p = Preferences::read(
        serde_json::json!({"MiniMapPosition":"Future","MiniMapSelectMode":"Future"})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(p, Preferences::default());
    for invalid in [
        serde_json::json!({"PositionOffsets":[]}),
        serde_json::json!({"PositionOffsets":{"LeftSideOfWindow":1.5}}),
        serde_json::json!({"PositionOffsets":{"LeftSideOfWindow":2147483648_i64}}),
    ] {
        assert!(Preferences::read(invalid.as_object().unwrap()).is_err());
    }
}
#[test]
fn ui_scaling_truncates_signed_offsets_and_resource_guards_do_not_clamp_geometry() {
    assert_eq!(minimap_mount::ui_scaled(-25, 1.25).unwrap(), -31);
    assert_eq!(minimap_mount::ui_scaled(51, 1.25).unwrap(), 63);
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(minimap_mount::ui_scaled(90, scale).is_err());
    }
    assert!(minimap_mount::ui_scaled(i32::MAX, 2.0).is_err());
    let mut p = Preferences::default();
    p.set_offset(Mount::LeftSideOfWindow, -25);
    let input = Input {
        chart_area: Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 60,
        },
        viewport_height: 100,
        focus: None,
        padding: Padding::original(1.0).unwrap(),
        dpi_scale: 1.0,
    };
    let b = minimap_mount::bounds(p, 90, input).unwrap();
    assert_eq!(
        b.x, -15,
        "source permits widget extending outside chart area"
    );
    assert_eq!(b.texture_height, 39);
    assert_eq!(b.visible_height, 19);
    assert!(minimap_mount::bounds(p, 1025, input).is_err());
}
