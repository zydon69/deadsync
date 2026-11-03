//! Run after applying the prepared ChartRequest bridge.
#[path = "../src/screens/edition/hold_render.rs"]
mod hold;
use deadlib_present::actors::{Actor, SizeSpec};
fn request(layout: &str, lane: u16, reverse: bool) -> hold::ChartRequest<'_> {
    hold::ChartRequest {
        layout,
        row: 0,
        lane,
        player: 0,
        selected: false,
        roll: false,
        active: false,
        center_x: 102.0,
        head_center: 100.0,
        tail_center: 180.0,
        width: 48.0,
        reverse,
        viewport: [100.0, 105.0, 200.0, 395.0],
        cutoff: None,
        size_zoom: 1.0,
        body_budget: 100,
    }
}
fn assert_clipped(actor: &Actor, viewport: [f32; 4]) {
    let Actor::Sprite {
        offset,
        size,
        rot_z_deg,
        cropleft: cl,
        cropright: cr,
        croptop: ct,
        cropbottom: cb,
        uv_rect: Some(uv),
        visible,
        ..
    } = actor
    else {
        panic!("expected sprite")
    };
    assert!(*visible);
    assert!(uv.iter().all(|n| n.is_finite() && (0.0..=1.0).contains(n)));
    let [SizeSpec::Px(width), SizeSpec::Px(height)] = *size else {
        panic!("explicit sprite sizes")
    };
    // The compositor offsets the cropped quad in world axes before rotation.
    // Measure that visible quad, including the renderer's crop compensation.
    let rotation = rot_z_deg.rem_euclid(360.0);
    let center = [
        offset[0] + (*cl - *cr) * width * 0.5,
        offset[1] + (*ct - *cb) * height * 0.5,
    ];
    let visible = [width * (1.0 - *cl - *cr), height * (1.0 - *ct - *cb)];
    let [w, h] = if rotation == 90.0 || rotation == 270.0 {
        [visible[1], visible[0]]
    } else {
        assert!(rotation == 0.0 || rotation == 180.0, "cardinal rotation");
        visible
    };
    let bounds = [
        center[0] - w * 0.5,
        center[1] - h * 0.5,
        center[0] + w * 0.5,
        center[1] + h * 0.5,
    ];
    assert!(
        bounds[0] >= viewport[0] - 1e-4
            && bounds[1] >= viewport[1] - 1e-4
            && bounds[2] <= viewport[2] + 1e-4
            && bounds[3] <= viewport[3] + 1e-4,
        "{bounds:?} vs {viewport:?}"
    );
}
#[test]
fn top_left_clipping_keeps_source_uv_phase_and_all_sprite_edges_inside_field() {
    let r = request("dance-single", 1, false);
    let viewport = r.viewport;
    let out = hold::chart_render(r).unwrap();
    assert!(!out.actors.is_empty());
    for a in &out.actors {
        assert_clipped(a, viewport);
    }
    assert!(
        out.actors
            .iter()
            .any(|a| matches!(a,Actor::Sprite{croptop,..}if *croptop>0.0))
    );
    assert!(
        out.actors
            .iter()
            .any(|a| matches!(a,Actor::Sprite{cropleft,..}if *cropleft>0.0))
    );
}
#[test]
fn pump_rotated_arrow_caps_and_heads_crop_in_their_local_axes() {
    let mut r = request("pump-single", 3, true);
    r.center_x = 195.0;
    r.head_center = 390.0;
    r.tail_center = 200.0;
    let viewport = r.viewport;
    let out = hold::chart_render(r).unwrap();
    for a in &out.actors {
        assert_clipped(a, viewport);
    }
    assert!(out.actors.iter().any(
        |a| matches!(a,Actor::Sprite{rot_z_deg,cropright,..}if *rot_z_deg== -90.0&& *cropright>0.0)
    ));
    assert!(out.actors.iter().any(
        |a| matches!(a,Actor::Sprite{rot_z_deg,croptop,..}if *rot_z_deg== -90.0&& *croptop>0.0)
    ));
}
#[test]
fn solo_reverse_and_far_offscreen_sustains_remain_clipped_and_bounded() {
    let mut r = request("dance-solo", 1, true);
    r.head_center = 400.0;
    r.tail_center = 110.0;
    let viewport = r.viewport;
    let out = hold::chart_render(r).unwrap();
    assert!(!out.actors.is_empty());
    for a in &out.actors {
        assert_clipped(a, viewport);
    }
    let mut r = request("dance-single", 0, false);
    r.head_center = -1e9;
    r.tail_center = 1e9;
    r.body_budget = 3;
    let viewport = r.viewport;
    let out = hold::chart_render(r).unwrap();
    assert!(out.partial);
    assert!(out.body_strips <= 3);
    assert!(out.actors.len() <= 12);
    for a in &out.actors {
        assert_clipped(a, viewport);
    }
}
