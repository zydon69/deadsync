#[path = "../src/screens/edition/hold_palette.rs"]
mod palette;
use palette::{Options, palette};
#[test]
fn source_piu_body_lighting_never_changes_head_or_arrow_endcap_color() {
    let idle = palette(
        "pump-single",
        0,
        0,
        0,
        false,
        false,
        false,
        Options::default(),
    )
    .unwrap();
    let active = palette(
        "pump-single",
        0,
        0,
        0,
        false,
        false,
        true,
        Options::default(),
    )
    .unwrap();
    assert_eq!(idle.head, active.head);
    assert_eq!(idle.cap, active.cap);
    assert_eq!(active.cap, active.head);
    assert_ne!(idle.body, active.body);
    assert_ne!(active.body, active.cap);
    let half = palette(
        "pump-halfdouble",
        0,
        0,
        0,
        false,
        false,
        false,
        Options::default(),
    )
    .unwrap();
    let center = palette(
        "pump-single",
        0,
        2,
        0,
        false,
        false,
        false,
        Options::default(),
    )
    .unwrap();
    assert_eq!(half, center);
}
#[test]
fn source_multiplayer_switch_differs_for_piu_lane_and_row_based_palettes() {
    let options = Options {
        multiplayer: true,
        color_multiplayer_holds: false,
        ..Default::default()
    };
    let p0 = palette("pump-routine", 0, 0, 0, false, false, true, options).unwrap();
    let p1 = palette("pump-routine", 0, 0, 1, false, false, true, options).unwrap();
    assert_ne!(p0.body, p1.body);
    let row = Options {
        row_based_piu: true,
        ..options
    };
    let p0 = palette("pump-routine", 0, 0, 0, false, false, true, row).unwrap();
    let p1 = palette("pump-routine", 0, 0, 1, false, false, true, row).unwrap();
    assert_eq!(p0.body, p1.body);
    assert_ne!(p0.head, p1.head);
    let row = Options {
        color_multiplayer_holds: true,
        ..row
    };
    assert_ne!(
        palette("pump-routine", 0, 0, 0, false, false, true, row)
            .unwrap()
            .body,
        palette("pump-routine", 0, 0, 1, false, false, true, row)
            .unwrap()
            .body
    );
}
#[test]
fn source_subdivisions_and_hold_roll_colors_preserve_independent_lighting_flags() {
    assert_eq!(palette::row_color(0), palette::row_color(48));
    assert_ne!(palette::row_color(0), palette::row_color(24));
    let options = Options {
        light_holds: false,
        hide_arrows: true,
        ..Default::default()
    };
    let hold = palette("dance-solo", 24, 1, 0, false, false, true, options).unwrap();
    assert_ne!(hold.start, hold.body);
    assert_eq!(hold.cap, hold.body);
    assert!(hold.activity.cutoff);
    assert_ne!(
        hold.body,
        palette("dance-solo", 24, 1, 0, false, true, true, options)
            .unwrap()
            .body
    );
    assert!(palette("unknown", 0, 0, 0, false, false, false, options).is_err());
    assert!(palette("dance-single", 0, 4, 0, false, false, false, options).is_err());
}
