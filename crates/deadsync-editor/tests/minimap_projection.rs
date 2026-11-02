use deadsync_editor::minimap_projection as projection;
use projection::Projection;

#[test]
fn projection_reflection_drag_and_overscroll_match_pinned_csharp_methods() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-projection.reference.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 60);
    for (index, case) in cases.iter().enumerate() {
        let i = &case["input"];
        let expected = &case["expected"];
        let number = |key: &str| i[key].as_f64().unwrap();
        let mut geometry = Projection {
            full_start: number("FullStart"),
            full_end: number("FullEnd"),
            mini_start: number("MiniStart"),
            mini_unquantized: 0.0,
            mini_range: number("MiniRange"),
            editor_start: number("EditorStart"),
            editor_end: number("EditorEnd"),
            bounds_y: i["BoundsY"].as_i64().unwrap() as i32,
            visible_height: i["VisibleHeight"].as_i64().unwrap() as i32,
            grabbed_percentage: number("GrabbedPercentage"),
            reverse: i["Reverse"].as_bool().unwrap(),
            quantize: i["Quantize"].as_bool().unwrap(),
        }
        .initialized()
        .unwrap();
        let screen_y = i["ScreenY"].as_i64().unwrap() as i32;
        let reflected = geometry.reflect_y(screen_y).unwrap();
        assert_eq!(
            i64::from(reflected),
            expected["reflected"].as_i64().unwrap()
        );
        let projected = geometry.project_editor(geometry.editor_start);
        let position = geometry.position_at_pixel(f64::from(reflected));
        geometry.drag(screen_y).unwrap();
        for (key, actual) in [
            ("position", position),
            ("projected", projected),
            ("miniStart", geometry.mini_start),
            ("miniUnquantized", geometry.mini_unquantized),
            ("editorStart", geometry.editor_start),
            ("editorEnd", geometry.editor_end),
        ] {
            let wanted = expected[key].as_f64().unwrap();
            assert!(
                (actual - wanted).abs() < 1e-10,
                "case{index} {key}: {actual} != {wanted}"
            );
        }
    }
}
