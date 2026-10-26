#[path = "../src/screens/edition/player_indicator.rs"]
mod player_indicator;
use deadlib_present::actors::{Actor, SizeSpec};
#[test]
fn source_selected_player_marker_is_right_of_focused_field_with_original_atlas_and_raw_fill() {
    let actors = player_indicator::actors(
        "dance-routine",
        true,
        600.0,
        160.0,
        0.375,
        [0.7109375, 0.09375, 0.7109375],
    )
    .unwrap();
    assert_eq!(actors.len(), 2);
    for actor in &actors {
        let Actor::Sprite {
            offset, size, z, ..
        } = actor
        else {
            panic!("atlas sprite")
        };
        assert_eq!(*offset, [607.5, 160.0]);
        assert!(matches!(size, [SizeSpec::Px(15.0), SizeSpec::Px(15.0)]));
        assert_eq!(*z, 8002);
    }
    let Actor::Sprite { tint, uv_rect, .. } = &actors[0] else {
        unreachable!()
    };
    assert_eq!(*tint, [181.0 / 255.0, 23.0 / 255.0, 181.0 / 255.0, 1.0]);
    assert_eq!(
        *uv_rect,
        Some([
            589.0 / 1280.0,
            1215.0 / 1280.0,
            629.0 / 1280.0,
            1255.0 / 1280.0
        ])
    );
    let Actor::Sprite { tint, uv_rect, .. } = &actors[1] else {
        unreachable!()
    };
    assert_eq!(*tint, [1.0; 4]);
    assert_eq!(
        *uv_rect,
        Some([
            783.0 / 1280.0,
            717.0 / 1280.0,
            823.0 / 1280.0,
            757.0 / 1280.0
        ])
    );
}
#[test]
fn source_chart_family_and_focus_gate_marker_without_inferring_stream_count() {
    for layout in [
        "dance-couple",
        "dance-routine",
        "pump-couple",
        "pump-routine",
        "smx-team",
    ] {
        assert_eq!(
            player_indicator::actors(layout, true, 600.0, 160.0, 0.375, [1.0, 0.0, 0.0])
                .unwrap()
                .len(),
            2
        );
        assert!(
            player_indicator::actors(layout, false, 600.0, 160.0, 0.375, [1.0, 0.0, 0.0])
                .unwrap()
                .is_empty()
        );
    }
    for layout in ["dance-single", "dance-double", "pump-double", "smx-full"] {
        assert!(
            player_indicator::actors(layout, true, 600.0, 160.0, 0.375, [1.0, 0.0, 0.0])
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        player_indicator::actors("dance-routine", true, 600.0, 160.0, f32::NAN, [1.0; 3]).is_err()
    );
}
