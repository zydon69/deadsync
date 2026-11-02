#[path = "../src/screens/edition/short_note_render.rs"]
mod short_note_render;
use deadlib_present::actors::Actor;
use short_note_render::{Kind, Request};
fn request(kind: Kind) -> Request<'static> {
    Request {
        layout: "dance-single",
        lane: 0,
        kind,
        selected: false,
        fake: false,
        center: [300.0, 200.0],
        width: 64.0,
        fill: [20, 40, 60, 255],
        alpha: 0.8,
    }
}
fn sprite(a: &Actor) -> ([f32; 4], f32) {
    match a {
        Actor::Sprite {
            uv_rect, rot_z_deg, ..
        } => (uv_rect.unwrap(), *rot_z_deg),
        _ => panic!("expected sprite"),
    }
}
#[test]
fn source_fill_rim_and_centered_unrotated_markers_keep_original_atlas_order() {
    let mut r = request(Kind::Lift);
    r.selected = true;
    let a = short_note_render::render(r).unwrap();
    assert_eq!(a.len(), 3);
    assert_eq!(sprite(&a[0]).1, -90.0);
    assert_eq!(sprite(&a[1]).1, -90.0);
    assert_eq!(sprite(&a[2]).1, 0.0);
    assert_eq!(
        sprite(&a[1]).0,
        [
            981.0 / 1280.0,
            1.0 / 1280.0,
            1109.0 / 1280.0,
            129.0 / 1280.0
        ]
    );
    assert_eq!(
        sprite(&a[2]).0,
        [
            589.0 / 1280.0,
            1173.0 / 1280.0,
            629.0 / 1280.0,
            1213.0 / 1280.0
        ]
    );
    let mut r = request(Kind::Lift);
    r.fake = true;
    let a = short_note_render::render(r).unwrap();
    assert_eq!(a.len(), 3);
    assert_eq!(
        sprite(&a[2]).0,
        [
            523.0 / 1280.0,
            1239.0 / 1280.0,
            563.0 / 1280.0,
            1279.0 / 1280.0
        ]
    );
}
#[test]
fn mine_never_rotates_and_fake_marker_overlays_its_selected_rim() {
    let mut r = request(Kind::Mine);
    r.selected = true;
    r.fake = true;
    let a = short_note_render::render(r).unwrap();
    assert_eq!(a.len(), 3);
    for actor in &a {
        assert_eq!(sprite(actor).1, 0.0);
    }
    assert_eq!(
        sprite(&a[0]).0,
        [
            849.0 / 1280.0,
            911.0 / 1280.0,
            977.0 / 1280.0,
            1039.0 / 1280.0
        ]
    );
    assert_eq!(
        sprite(&a[1]).0,
        [
            1109.0 / 1280.0,
            781.0 / 1280.0,
            1237.0 / 1280.0,
            909.0 / 1280.0
        ]
    );
}
#[test]
fn every_supported_family_resolves_and_invalid_or_hidden_notes_do_not_emit() {
    for layout in [
        "dance-single",
        "dance-double",
        "dance-couple",
        "dance-routine",
        "dance-solo",
        "dance-threepanel",
        "pump-single",
        "pump-halfdouble",
        "pump-double",
        "pump-couple",
        "pump-routine",
        "smx-single",
        "smx-beginner",
        "smx-dual",
        "smx-full",
        "smx-team",
    ] {
        for lane in 0..deadsync_editor::notes::layout_columns(layout).unwrap() {
            let mut r = request(Kind::Tap);
            r.layout = layout;
            r.lane = lane;
            assert_eq!(short_note_render::render(r).unwrap().len(), 2);
        }
    }
    let mut r = request(Kind::Tap);
    r.alpha = 0.0;
    assert!(short_note_render::render(r).unwrap().is_empty());
    let mut r = request(Kind::Tap);
    r.width = f32::NAN;
    assert!(short_note_render::render(r).is_err());
    let mut r = request(Kind::Tap);
    r.lane = 4;
    assert!(short_note_render::render(r).is_err());
}
