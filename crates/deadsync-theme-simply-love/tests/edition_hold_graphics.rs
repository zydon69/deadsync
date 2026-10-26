#[path = "../src/screens/edition/hold_atlas.rs"]
mod atlas;
#[path = "../src/screens/edition/hold_graphics.rs"]
mod graphics;
#[test]
fn every_supported_lane_resolves_original_fill_and_selected_rim_rectangles() {
    for layout in deadsync_editor::chart_order::TYPE_ORDER {
        let n = deadsync_editor::notes::layout_columns(layout).unwrap();
        for lane in 0..n {
            for selected in [false, true] {
                for roll in [false, true] {
                    let g = graphics::graphic(layout, lane, selected, roll).unwrap();
                    for id in [
                        Some(&g.head_fill),
                        Some(&g.head_rim),
                        g.start_fill.as_ref(),
                        g.start_rim.as_ref(),
                        Some(&g.body_fill),
                        Some(&g.body_rim),
                        Some(&g.cap_fill),
                        Some(&g.cap_rim),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        let [x, y, w, h] =
                            atlas::rect(id).unwrap_or_else(|| panic!("{layout}/{lane}: {id}"));
                        assert!(w > 0 && h > 0 && x + w <= 1280 && y + h <= 1280);
                    }
                }
            }
        }
        assert!(graphics::graphic(layout, n, false, false).is_err());
    }
}
#[test]
fn solo_uses_narrow_body_on_all_lanes_and_start_only_for_diagonal_heads() {
    for lane in 0..6 {
        let g = graphics::graphic("dance-solo", lane, true, true).unwrap();
        assert_eq!(g.body_fill, "itg-solo-roll-fill");
        assert_eq!(g.cap_fill, "itg-solo-roll-end-fill");
        assert_eq!(g.start_fill.is_some(), lane == 1 || lane == 4);
    }
    let smx = graphics::graphic("smx-single", 0, false, false).unwrap();
    assert_eq!(smx.head_fill, "itg-down-fill");
    assert_eq!(smx.body_fill, "itg-center-hold-fill");
}
#[test]
fn pump_caps_remain_arrow_shaped_centered_and_unflipped_in_reverse() {
    for lane in 0..5 {
        let g = graphics::graphic("pump-single", lane, false, true).unwrap();
        assert_eq!(g.cap_fill, g.head_fill);
        assert_eq!(g.cap_rim, g.head_rim);
        assert_eq!(g.cap_rotation, g.head_rotation);
        assert!(g.cap_centered);
        assert!(!g.flip_cap_in_reverse);
        assert_eq!(g.body_mirrored, lane >= 3);
    }
    assert_eq!(
        graphics::graphic("pump-halfdouble", 0, false, false)
            .unwrap()
            .head_fill,
        "piu-center-fill"
    );
    assert!(graphics::graphic("unknown", 0, false, false).is_err());
}
