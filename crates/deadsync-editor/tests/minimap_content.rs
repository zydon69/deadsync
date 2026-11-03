use deadsync_editor::minimap_projection;
use deadsync_editor::minimap_content;
use minimap_content::{AddResult, Raster};
#[test]
fn pinned_csharp_visibility_lane_layout_and_raster_match_168_cases() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-content.reference.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 168);
    for (index, case) in cases.iter().enumerate() {
        let i = &case["input"];
        let expected = &case["expected"];
        let u = |key: &str| i[key].as_u64().unwrap() as u32;
        let quantize = i["quantize"].as_bool().unwrap();
        let mut raster = Raster::new(
            u("width"),
            u("height"),
            u("lanes"),
            u("noteWidth"),
            u("noteSpacing"),
            quantize,
            0xff102030,
        )
        .unwrap();
        let projection = minimap_projection::Projection {
            full_start: 0.0,
            full_end: 16.0,
            mini_start: 0.0,
            mini_unquantized: 0.0,
            mini_range: 16.0,
            editor_start: 0.0,
            editor_end: 1.0,
            bounds_y: 0,
            visible_height: 16,
            grabbed_percentage: 0.0,
            reverse: false,
            quantize,
        };
        let force = i["kind"] == "cursor";
        let start = Raster::project(&projection, i["start"].as_f64().unwrap(), force).unwrap();
        let end = Raster::project(&projection, i["end"].as_f64().unwrap(), false).unwrap();
        let result = match i["kind"].as_str().unwrap() {
            "short" => raster.short_note(u("lane"), start, 0xffef4477),
            "cursor" => raster.line(start, 1, u("width") - 2, 0xffef4477, true),
            "rect" => raster.rect(1, u("width") - 2, start, end, 0xffef4477),
            "hold" => raster.hold(u("lane"), start, end, 0xffef4477, 0xff3355dd),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(
            format!("{result:?}"),
            expected["result"].as_str().unwrap(),
            "case{index}"
        );
        assert_eq!(
            raster.lane_positions(),
            expected["lanes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<_>>(),
            "lanes case{index}"
        );
        assert_eq!(
            raster.pixels(),
            expected["pixels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<_>>(),
            "pixels case{index}"
        );
    }
}
#[test]
fn crossing_hold_roll_colors_zero_width_and_invalid_input_are_bounded() {
    let mut raster = Raster::new(16, 16, 4, 2, 3, false, 0xff102030).unwrap();
    assert_eq!((raster.width(), raster.height()), (16, 16));
    assert_eq!(
        raster
            .hold(0, -100.0, 100.0, 0xff00ff00, 0xff0000ff)
            .unwrap(),
        AddResult::InRange
    );
    let lane = raster.lane_positions()[0] as usize;
    assert_eq!(
        raster.pixels()[16 + lane],
        0xff0000ff,
        "crossing body has no synthetic head"
    );
    assert!(raster.short_note(4, 3.0, 1).is_err());
    assert!(raster.short_note(0, f64::NAN, 1).is_err());
    assert!(raster.rect(1, 16, 2.0, 4.0, 1).is_err());
    assert!(Raster::new(1, 16, 4, 2, 3, false, 1).is_err());
    assert!(Raster::new(1025, 16, 4, 2, 3, false, 1).is_err());
    let mut empty = Raster::new(16, 16, 4, 0, 3, true, 42).unwrap();
    assert_eq!(empty.hold(0, 2.0, 5.0, 1, 2).unwrap(), AddResult::InRange);
    assert!(empty.pixels().iter().all(|p| *p == 42));
}

#[test]
fn minimum_width_clips_independent_note_width_to_empty_interior() {
    let mut raster = Raster::new(2, 16, 4, 32, 3, false, 42).unwrap();
    assert_eq!(raster.lane_positions(), &[1, 1, 1, 1]);
    for lane in 0..4 {
        raster.short_note(lane, 3.0, 7).unwrap();
        raster.hold(lane, -10.0, 20.0, 8, 9).unwrap();
    }
    assert_eq!(raster.pixels().len(), 32);
    assert!(raster.pixels().iter().all(|pixel| *pixel == 42));
}
