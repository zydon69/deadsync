//! Requires the prepared palette declaration/HSV-visibility patch.
use deadsync_editor::{
    minimap_palette::ColorMethod, note_color_palette::Palette, note_color_preferences::Preferences,
    timing_edit::signature::Signatures,
};
use std::sync::Arc;
#[test]
fn immutable_palette_binds_exact_shared_arc_and_xna_does_not_use_ui_multipliers() {
    let mut p = Preferences::default();
    p.colors.hold = [0.1, 0.2, 0.3];
    p.colors.normal_multiplier = 8.0;
    p.colors.selected_multiplier = 8.0;
    p.held_multiplier = 2.0;
    p.selection_multiplier = 1.5;
    let source = Arc::new(p);
    let palette = Palette::capture(source.clone()).unwrap();
    assert!(palette.is_current(&source));
    assert!(!palette.is_current(&Arc::new((*source).clone())));
    let fill = palette
        .fills("dance-single", 0, 0, 0, true, false, true, true)
        .unwrap();
    assert_eq!(fill.body, [76, 153, 229, 255]);
    assert_eq!(fill.cap, fill.body);
    let unlit = palette
        .fills("dance-single", 0, 0, 0, true, false, true, false)
        .unwrap();
    assert_ne!(unlit.start, unlit.body);
    assert_eq!(unlit.start, fill.body);
}
#[test]
fn per_chart_signature_phase_includes_same_row_change_before_note_color_lookup() {
    let signatures = Signatures::parse("0=3=8,0.5=5=16").unwrap();
    for (method, at24, at30) in [
        (ColorMethod::Stepmania, 24, 30),
        (ColorMethod::Note, 0, 6),
        (ColorMethod::Beat, 0, 24),
    ] {
        let p = Arc::new(Preferences {
            step_color_method: method,
            ..Default::default()
        });
        let palette = Palette::capture(p).unwrap();
        assert_eq!(palette.color_row(24, Some(&signatures)).unwrap(), at24);
        assert_eq!(palette.color_row(30, Some(&signatures)).unwrap(), at30);
    }
}
#[test]
fn cached_variants_replay_original_xna_constructor_oracle() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../deadsync-theme-simply-love/tests/fixtures/hold-colors.reference.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut p = Preferences::default();
        p.colors.hold = std::array::from_fn(|i| case["rgb"][i].as_f64().unwrap() as f32);
        let palette = Palette::capture(Arc::new(p)).unwrap();
        let expected: Vec<u8> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let fill = palette
            .fills(
                "dance-single",
                0,
                0,
                0,
                case["selected"].as_bool().unwrap(),
                false,
                case["held"].as_bool().unwrap(),
                true,
            )
            .unwrap();
        assert_eq!(fill.body.as_slice(), expected);
    }
}
#[test]
fn cached_piu_palette_replays_source_hsv_and_owner_blend_oracle() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../deadsync-theme-simply-love/tests/fixtures/piu-fill.reference.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut p = Preferences::default();
        p.colors.piu_top = std::array::from_fn(|i| case["rgb"][i].as_f64().unwrap() as f32);
        p.colors.piu_hold_saturation = case["multiplier"].as_f64().unwrap() as f32;
        p.colors.routine_alpha = case["alpha"].as_f64().unwrap() as f32;
        let routine = !case["player"].is_null();
        if routine {
            p.colors.players[0] =
                std::array::from_fn(|i| case["player"][i].as_f64().unwrap() as f32);
        }
        let palette = Palette::capture(Arc::new(p)).unwrap();
        let fill = palette
            .fills(
                if routine {
                    "pump-routine"
                } else {
                    "pump-single"
                },
                0,
                1,
                0,
                false,
                false,
                false,
                true,
            )
            .unwrap();
        let mut expected = [0, 0, 0, 255];
        for i in 0..3 {
            expected[i] = ((case["expected"][i].as_f64().unwrap() as f32) * 255.0) as u8;
        }
        assert_eq!(fill.body, expected, "{case}");
    }
}
#[test]
fn source_family_dispatch_custom_owners_and_piu_row_mode_share_one_raw_dto() {
    let mut p = Preferences::default();
    p.colors.color_multiplayer_holds = false;
    let palette = Palette::capture(Arc::new(p.clone())).unwrap();
    let ordinary = palette
        .fills("dance-double", 0, 0, 0, false, false, false, true)
        .unwrap();
    assert_eq!(
        palette
            .fills("dance-couple", 0, 4, 1, false, false, false, true)
            .unwrap(),
        ordinary
    );
    let routine = palette
        .fills("dance-routine", 0, 0, 0, false, false, false, true)
        .unwrap();
    assert_eq!(routine.body, ordinary.body);
    assert_ne!(routine.head, ordinary.head);
    let piu0 = palette
        .fills("pump-routine", 0, 1, 0, false, false, false, true)
        .unwrap();
    let piu1 = palette
        .fills("pump-routine", 0, 1, 1, false, false, false, true)
        .unwrap();
    assert_ne!(piu0.body, piu1.body);
    assert_eq!(piu0.cap, piu0.head);
    p.piu_row_based = true;
    let palette = Palette::capture(Arc::new(p)).unwrap();
    let p0 = palette
        .fills("pump-routine", 0, 1, 0, false, false, false, true)
        .unwrap();
    let p1 = palette
        .fills("pump-routine", 0, 1, 1, false, false, false, true)
        .unwrap();
    assert_eq!(p0.body, p1.body);
    assert_ne!(p0.head, p1.head);
}

#[test]
fn tap_is_undesaturated_piu_head_and_mines_use_xna_owner_variants() {
    let mut p = Preferences::default();
    p.colors.mine = [0.2, 0.4, 0.6];
    p.colors.routine_alpha = 1.0;
    p.colors.players[0] = [0.1, 0.2, 0.3];
    p.colors.piu_top = [0.2, 0.6, 0.4];
    p.colors.piu_hold_saturation = 0.0;
    let cache = Palette::capture(Arc::new(p)).unwrap();
    assert_eq!(
        cache.tap("pump-single", 0, 1, 0, false).unwrap(),
        [51, 153, 102, 255]
    );
    assert_eq!(
        cache.mine("dance-couple", 0, false).unwrap(),
        [51, 102, 153, 255]
    );
    assert_eq!(
        cache.mine("dance-routine", 0, false).unwrap(),
        [25, 51, 76, 255]
    );
    assert_eq!(cache.mine("smx-team", 0, true).unwrap(), [43, 86, 130, 255]);
}
