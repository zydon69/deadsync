use deadsync_editor::minimap_background;
use deadsync_editor::minimap_projection;
use minimap_background::{build, fade};
#[test]
fn original_updatebegin_background_and_updateend_fade_match_96_cases() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-background.reference.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 96);
    for (index, case) in cases.iter().enumerate() {
        let input = &case["input"];
        let expected = &case["expected"];
        let projection = minimap_projection::Projection {
            full_start: -8.0,
            full_end: 40.0,
            mini_start: expected["miniStart"].as_f64().unwrap(),
            mini_unquantized: expected["miniUnquantized"].as_f64().unwrap(),
            mini_range: 16.0,
            editor_start: input["editorStart"].as_f64().unwrap(),
            editor_end: input["editorEnd"].as_f64().unwrap(),
            bounds_y: 0,
            visible_height: 16,
            grabbed_percentage: 0.25,
            reverse: false,
            quantize: input["quantize"].as_bool().unwrap(),
        };
        let mut background = build(
            &projection,
            16,
            (0.0, 24.0),
            3.0,
            input["state"] == "grabbed",
            input["state"] == "hover",
        )
        .unwrap();
        assert_eq!(
            background.drawable,
            expected["drawable"].as_bool().unwrap(),
            "drawcase{index}"
        );
        if !background.drawable {
            continue;
        }
        let before = expected["before"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        assert_eq!(background.pixels, before, "basecase{index}");
        assert_eq!(
            [
                background.content_start_y,
                background.content_end_y,
                background.cursor_y
            ],
            std::array::from_fn::<_, 3, _>(|i| expected["markers"][i].as_f64().unwrap()),
            "markerscase{index}"
        );
        assert!(
            fade(
                &mut background.pixels,
                (projection.editor_end - projection.editor_start) / projection.mini_range,
                0.9
            )
            .unwrap()
        );
        let after = expected["after"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        assert_eq!(background.pixels, after, "fadecase{index}");
    }
}
#[test]
fn malformed_background_geometry_and_fade_never_overflow() {
    let projection = minimap_projection::Projection {
        full_start: 0.0,
        full_end: 40.0,
        mini_start: 0.0,
        mini_unquantized: 0.0,
        mini_range: 16.0,
        editor_start: 0.0,
        editor_end: 2.0,
        bounds_y: 0,
        visible_height: 16,
        grabbed_percentage: 0.25,
        reverse: false,
        quantize: false,
    };
    assert!(build(&projection, 1, (0.0, 24.0), 3.0, false, false).is_err());
    assert!(build(&projection, 16, (f64::NAN, 24.0), 3.0, false, false).is_err());
    let mut pixels = vec![0xff112233];
    assert!(fade(&mut pixels, f64::NAN, 0.9).is_err());
    assert!(!fade(&mut pixels, 1.0, 0.9).unwrap());
    assert_eq!(pixels, [0xff112233]);
}
