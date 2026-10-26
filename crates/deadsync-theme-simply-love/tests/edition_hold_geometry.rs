#[path = "../src/screens/edition/hold_atlas.rs"]
mod atlas;
#[path = "../src/screens/edition/hold_geometry.rs"]
mod geometry;
use geometry::Input;
fn input() -> Input {
    Input {
        head_top: 100.0,
        height: 300.0,
        scale: 1.0,
        arrow_height: 128,
        body_height: 128,
        cap_height: 65,
        start_height: 0,
        cap_centered: false,
        reverse: false,
        screen_height: 480.0,
        cutoff: None,
    }
}
#[test]
fn body_tiles_align_to_tail_and_partial_tile_uses_bottom_of_source() {
    let p = input().plan(100).unwrap();
    assert_eq!(p.head_top, 100.0);
    assert_eq!(p.cap.unwrap().top, 335.0);
    assert_eq!(p.body.len(), 2);
    assert_eq!((p.body[0].top, p.body[0].height), (207.0, 128.0));
    assert_eq!((p.body[1].top, p.body[1].height), (164.0, 43.0));
    assert_eq!((p.body[1].source_top, p.body[1].source_height), (85, 43));
    assert!(!p.partial);
}
#[test]
fn cutoff_moves_head_and_body_but_keeps_tail_and_texture_alignment() {
    let base = input().plan(100).unwrap();
    let p = Input {
        cutoff: Some(200.0),
        ..input()
    }
    .plan(100)
    .unwrap();
    assert_eq!(p.head_top, 136.0);
    assert_eq!(p.cap, base.cap);
    assert_eq!(p.body[0], base.body[0]);
    assert_eq!((p.body[1].top, p.body[1].height), (200.0, 7.0));
    assert_eq!((p.body[1].source_top, p.body[1].source_height), (121, 7));
}
#[test]
fn reverse_solo_omits_start_and_reflects_each_body_strip_about_head_center() {
    let p = Input {
        reverse: true,
        start_height: 28,
        height: -300.0,
        ..input()
    }
    .plan(100)
    .unwrap();
    assert!(p.start.is_none());
    assert_eq!(p.head_top, 100.0);
    assert!(p.body.iter().all(|s| s.height > 0.0));
    assert!(!p.partial);
}
#[test]
fn output_budget_is_explicit_and_large_offscreen_holds_do_not_traverse_from_tail() {
    let base = input().plan(100).unwrap();
    let p = input().plan(1).unwrap();
    assert_eq!(p.body, base.body[..1]);
    assert!(p.partial);
    let huge = Input {
        head_top: -1e9,
        height: 2e9,
        ..input()
    }
    .plan(10)
    .unwrap();
    assert!(huge.body.len() <= 6);
    assert!(!huge.partial);
}
#[test]
fn invalid_dimensions_fail_before_projection_and_original_caps_are_distinct() {
    assert!(
        Input {
            scale: 0.0,
            ..input()
        }
        .plan(100)
        .is_err()
    );
    assert!(
        Input {
            cutoff: Some(f64::NAN),
            ..input()
        }
        .plan(100)
        .is_err()
    );
    assert_eq!(atlas::rect("itg-down-hold-end-fill").unwrap()[3], 65);
    assert_eq!(atlas::rect("itg-solo-hold-end-fill").unwrap()[3], 53);
    assert_eq!(atlas::rect("itg-center-hold-end-fill").unwrap()[3], 51);
    assert_ne!(
        atlas::rect("itg-down-roll-fill"),
        atlas::rect("itg-down-hold-fill")
    );
    assert!(atlas::rect("not-a-texture").is_none());
}
#[test]
fn original_csharp_draw_and_tile_oracle_matches_reverse_cutoff_centered_and_solo_cases() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/hold-geometry.reference.json")).unwrap();
    fn strip(expected: &serde_json::Value, actual: geometry::Strip) {
        assert!((actual.top - expected["top"].as_f64().unwrap()).abs() < 1e-4);
        assert!((actual.height - expected["height"].as_f64().unwrap()).abs() < 1e-4);
        assert_eq!(
            actual.source_top,
            u32::try_from(expected["source_top"].as_u64().unwrap()).unwrap()
        );
        assert_eq!(
            actual.source_height,
            u32::try_from(expected["source_height"].as_u64().unwrap()).unwrap()
        );
    }
    for case in fixture["cases"].as_array().unwrap() {
        let i = &case["input"];
        let e = &case["expected"];
        let d = |k: &str| i[k].as_f64().unwrap();
        let n = |k: &str| i[k].as_u64().unwrap() as u32;
        let b = |k: &str| i[k].as_bool().unwrap();
        let p = Input {
            head_top: d("head_top"),
            height: d("height"),
            scale: d("scale"),
            arrow_height: n("arrow_height"),
            body_height: n("body_height"),
            cap_height: n("cap_height"),
            start_height: n("start_height"),
            cap_centered: b("cap_centered"),
            reverse: b("reverse"),
            screen_height: d("screen_height"),
            cutoff: i["cutoff"].as_f64(),
        }
        .plan(100)
        .unwrap();
        assert!(!p.partial);
        assert!((p.head_top - e["head_top"].as_f64().unwrap()).abs() < 1e-4);
        let body = e["body"].as_array().unwrap();
        assert_eq!(p.body.len(), body.len());
        for (actual, expected) in p.body.into_iter().zip(body) {
            strip(expected, actual);
        }
        for (expected, actual) in [(&e["start"], p.start), (&e["cap"], p.cap)] {
            assert_eq!(actual.is_some(), !expected.is_null());
            if let Some(actual) = actual {
                strip(expected, actual);
            }
        }
    }
}
