use deadsync_editor::minimap_projection;
#[path = "../src/minimap_position_preferences.rs"]
mod minimap_position_preferences;
#[path = "../src/minimap_select.rs"]
mod minimap_select;
use deadsync_editor::Error;
use minimap_position_preferences::SelectMode;
use minimap_projection::Projection;
use minimap_select::Capture;
fn projection() -> Projection {
    Projection {
        full_start: -10.0,
        full_end: 990.0,
        mini_start: -3.25,
        mini_unquantized: -3.25,
        mini_range: 400.0,
        editor_start: 23.75,
        editor_end: 103.75,
        bounds_y: 20,
        visible_height: 200,
        grabbed_percentage: 0.25,
        reverse: false,
        quantize: false,
    }
    .initialized()
    .unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn compare(projection: &Projection, capture: &Capture, expected: &serde_json::Value) {
    assert_eq!(capture.grabbed(), expected["grabbed"].as_bool().unwrap());
    for (key, value) in [
        ("miniStart", projection.mini_start),
        ("miniUnquantized", projection.mini_unquantized),
        ("editorStart", projection.editor_start),
        ("editorEnd", projection.editor_end),
    ] {
        close(value, expected[key].as_f64().unwrap());
    }
}
#[test]
fn original_mouse_selection_and_followup_motion_match_384_pinned_cases() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-select.reference.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 384);
    for case in fixture["cases"].as_array().unwrap() {
        let input = &case["input"];
        let expected = &case["expected"];
        let mut projection = projection();
        projection.full_end = input["full"].as_f64().unwrap() - 10.0;
        projection.mini_range = input["mini"].as_f64().unwrap();
        projection.editor_end = projection.editor_start + input["editor"].as_f64().unwrap();
        projection.reverse = input["reverse"].as_bool().unwrap();
        projection.quantize = input["quantize"].as_bool().unwrap();
        projection.grabbed_percentage = input["cursor"].as_f64().unwrap();
        projection = projection.initialized().unwrap();
        let cursor = projection.editor_start
            + (projection.editor_end - projection.editor_start) * projection.grabbed_percentage;
        let mode = if input["mode"] == "MoveToCursor" {
            SelectMode::MoveToCursor
        } else {
            SelectMode::MoveToSelectedArea
        };
        let pixel = input["pixel"].as_i64().unwrap() as i32;
        let mut capture = Capture::default();
        assert_eq!(
            capture.begin(&mut projection, pixel, cursor, mode),
            Some(expected["accepted"].as_bool().unwrap())
        );
        compare(&projection, &capture, &expected["begin"]);
        capture.move_to(&mut projection, pixel + 15).unwrap();
        compare(&projection, &capture, &expected["moved"]);
        assert_eq!(
            projection.grabbed_percentage,
            input["cursor"].as_f64().unwrap(),
            "cursor fraction must not become pointer grab fraction"
        );
    }
}
#[test]
fn outside_area_modes_have_distinct_destinations_and_capture_clear_ends_drag() {
    let mut cursor = projection();
    let mut selected = cursor;
    let position =
        cursor.editor_start + (cursor.editor_end - cursor.editor_start) * cursor.grabbed_percentage;
    let mut first = Capture::default();
    let mut second = Capture::default();
    first
        .begin(&mut cursor, 170, position, SelectMode::MoveToCursor)
        .unwrap();
    second
        .begin(&mut selected, 170, position, SelectMode::MoveToSelectedArea)
        .unwrap();
    assert!((cursor.editor_start - selected.editor_start).abs() > 100.0);
    second.clear();
    let saved = selected.editor_start;
    second.move_to(&mut selected, 200).unwrap();
    assert_eq!(selected.editor_start, saved);
}
#[test]
fn rejected_or_malformed_begin_cannot_mutate_projection_or_capture() {
    let mut projection = projection();
    let original = projection.editor_start;
    let mut capture = Capture::default();
    assert_eq!(
        capture.begin(&mut projection, 19, 40.0, SelectMode::MoveToSelectedArea),
        Some(false)
    );
    assert_eq!(projection.editor_start, original);
    assert!(!capture.grabbed());
    assert_eq!(
        capture.begin(
            &mut projection,
            100,
            f64::NAN,
            SelectMode::MoveToSelectedArea
        ),
        None
    );
    assert_eq!(projection.editor_start, original);
    assert!(!capture.grabbed());
}
