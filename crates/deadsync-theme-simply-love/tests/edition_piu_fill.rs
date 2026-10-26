#[path = "../src/screens/edition/hold_colors.rs"]
mod hold_colors;
#[path = "../src/screens/edition/piu_fill.rs"]
mod piu_fill;
#[test]
fn source_hsv_and_blend_oracle_preserves_final_xna_color_bytes() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/piu-fill.reference.json")).unwrap();
    for c in fixture["cases"].as_array().unwrap() {
        let mut rgb: [f32; 3] = std::array::from_fn(|i| c["rgb"][i].as_f64().unwrap() as f32);
        if !c["player"].is_null() {
            let player: [f32; 3] = std::array::from_fn(|i| c["player"][i].as_f64().unwrap() as f32);
            let alpha = c["alpha"].as_f64().unwrap() as f32;
            rgb = std::array::from_fn(|i| {
                (rgb[i] * (1.0 - alpha) + player[i] * alpha).clamp(0.0, 1.0)
            });
        }
        let actual = piu_fill::desaturate(rgb, c["multiplier"].as_f64().unwrap() as f32).unwrap();
        let expected: [f32; 3] = std::array::from_fn(|i| c["expected"][i].as_f64().unwrap() as f32);
        for (a, e) in actual.into_iter().zip(expected) {
            assert!((a - e).abs() <= 1e-7, "{c}: {actual:?} != {expected:?}");
        }
        for held in [false, true] {
            for selected in [false, true] {
                assert_eq!(
                    hold_colors::variant(actual, held, selected, Default::default()),
                    hold_colors::variant(expected, held, selected, Default::default()),
                    "{c}"
                );
            }
        }
    }
}
#[test]
fn pump_body_desaturates_before_lighting_but_end_cap_keeps_note_color() {
    for i in 0..5 {
        let note = piu_fill::lane_color(i);
        let body = piu_fill::desaturate(note, 0.8).unwrap();
        assert_ne!(body, note);
        assert_eq!(piu_fill::lane_color(i), piu_fill::lane_color(i + 5));
        assert_eq!(hold_colors::activity(true, false, true).body_held, false);
    }
    assert_eq!(piu_fill::lane_color(2), piu_fill::MIDDLE);
    assert!(piu_fill::desaturate([f32::NAN, 0.0, 0.0], 0.8).is_err());
}
