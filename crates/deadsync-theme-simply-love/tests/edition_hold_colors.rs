#[path = "../src/screens/edition/hold_colors.rs"]
mod colors;
#[test]
fn xna_fill_variants_match_unchanged_original_constructor_oracle() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/hold-colors.reference.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let rgb = std::array::from_fn(|i| case["rgb"][i].as_f64().unwrap() as f32);
        let expected: Vec<u8> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(
            colors::variant(
                rgb,
                case["held"].as_bool().unwrap(),
                case["selected"].as_bool().unwrap(),
                Default::default()
            )
            .unwrap()
            .as_slice(),
            expected
        );
    }
}
#[test]
fn held_start_and_body_lighting_remain_independent_of_cutoff_preference() {
    assert_eq!(
        colors::activity(true, false, true),
        colors::Activity {
            start_held: true,
            body_held: false,
            cutoff: true
        }
    );
    assert_eq!(
        colors::activity(true, true, false),
        colors::Activity {
            start_held: true,
            body_held: true,
            cutoff: false
        }
    );
    assert_eq!(
        colors::activity(false, true, true),
        colors::Activity {
            start_held: false,
            body_held: false,
            cutoff: false
        }
    );
}
#[test]
fn routine_owner_blend_uses_source_palette_before_selected_and_held_multipliers() {
    let p0 = colors::multiplayer(colors::HOLD, 0, 0.9).unwrap();
    let p1 = colors::multiplayer(colors::HOLD, 1, 0.9).unwrap();
    assert_ne!(p0, p1);
    assert_eq!(colors::multiplayer(colors::HOLD, 10, 0.9).unwrap(), p0);
    assert_eq!(
        colors::multiplayer(colors::ROLL, 2, 0.0).unwrap(),
        colors::ROLL
    );
    assert_eq!(
        colors::variant(colors::HOLD, false, false, Default::default()).unwrap(),
        [68, 68, 68, 255]
    );
    assert_eq!(
        colors::variant(colors::HOLD, true, false, Default::default()).unwrap(),
        [102, 102, 102, 255]
    );
    assert_ne!(
        colors::variant(p0, true, false, Default::default()),
        colors::variant(p1, true, false, Default::default())
    );
    assert!(colors::variant([f32::NAN, 0.0, 0.0], false, false, Default::default()).is_err());
    assert!(colors::multiplayer(colors::HOLD, 0, f32::NAN).is_err());
}
