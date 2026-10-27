#[path = "../src/screens/edition/minimap_mount_layout.rs"]
mod minimap_mount_layout;
use minimap_mount_layout::{Input, layout};
fn input() -> Input {
    Input {
        left: 100.0,
        lane_width: 48.0,
        columns: 4,
        size_zoom: 1.0,
        clip_right: 854.0,
        misc_width: 96.0,
        multiplayer: false,
    }
}
#[test]
fn real_native_misc_allocation_and_fixed_lane_centers_define_mount_edges() {
    let ordinary = layout(input()).unwrap();
    assert_eq!(ordinary.field_right, 292.0);
    assert_eq!((ordinary.misc_start, ordinary.misc_end), (304.0, 400.0));
    assert_eq!(
        (ordinary.without_scale, ordinary.with_scale),
        (400.0, 400.0)
    );
    let zoomed = layout(Input {
        size_zoom: 8.0,
        ..input()
    })
    .unwrap();
    assert_eq!(zoomed.without_scale, 400.0);
    assert_eq!(zoomed.with_scale, 460.0); // Last center268 + half scaled sprite192.
    let no_misc = layout(Input {
        misc_width: 0.0,
        size_zoom: 2.0,
        ..input()
    })
    .unwrap();
    assert_eq!((no_misc.without_scale, no_misc.with_scale), (292.0, 316.0));
}
#[test]
fn allocation_respects_available_panel_space_without_inventing_waveform_or_misc_regions() {
    let narrow = layout(Input {
        clip_right: 340.0,
        ..input()
    })
    .unwrap();
    assert_eq!(narrow.misc_end, 328.0);
    let below_text_minimum = layout(Input {
        clip_right: 330.0,
        ..input()
    })
    .unwrap();
    assert_eq!(below_text_minimum.misc_end, below_text_minimum.field_right);
    assert!(below_text_minimum.misc_end <= below_text_minimum.misc_start);
    let absent = layout(Input {
        clip_right: 280.0,
        ..input()
    })
    .unwrap();
    assert_eq!(absent.misc_end, 292.0);
    // Field/waveform extent stays real even when the view itself is partly off screen.
    assert_eq!(absent.with_scale, 292.0);
    for bad in [
        Input {
            size_zoom: f32::NAN,
            ..input()
        },
        Input {
            columns: 0,
            ..input()
        },
        Input {
            misc_width: -1.0,
            ..input()
        },
    ] {
        assert!(layout(bad).is_none());
    }
}

#[test]
fn only_rendered_multiplayer_indicator_adds_reserved_width() {
    let normal = layout(Input {
        misc_width: 0.0,
        ..input()
    })
    .unwrap();
    let multi = layout(Input {
        misc_width: 0.0,
        multiplayer: true,
        size_zoom: 2.0,
        ..input()
    })
    .unwrap();
    assert_eq!(normal.without_scale, 292.0);
    assert_eq!(multi.without_scale, 307.0);
    assert_eq!(multi.with_scale, 322.0);
}
