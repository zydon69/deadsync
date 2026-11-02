#[path = "../src/screens/edition/hold_render.rs"]
mod render;
use deadlib_present::actors::Actor;
use render::{Colors, Request, geometry::Input};
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
        cutoff: Some(200.0),
    }
}
fn colors() -> Colors {
    Colors {
        head: [200, 30, 40, 255],
        start: [90, 90, 90, 255],
        body: [102, 102, 102, 255],
        cap: [102, 102, 102, 255],
    }
}
fn details(actor: &Actor) -> ([f32; 2], [f32; 4], [f32; 4], f32) {
    match actor {
        Actor::Sprite {
            offset,
            tint,
            uv_rect,
            rot_z_deg,
            ..
        } => (*offset, *tint, uv_rect.unwrap(), *rot_z_deg),
        _ => panic!("expected original atlas sprite"),
    }
}
#[test]
fn renderer_emits_body_start_cap_head_in_source_order_with_independent_fill_and_rim() {
    let g = render::graphics::graphic("dance-single", 0, true, false).unwrap();
    let out = render::render(Request {
        graphic: &g,
        geometry: input(),
        center_x: 100.0,
        width: 128.0,
        colors: colors(),
        alpha: 0.5,
        body_budget: 100,
    })
    .unwrap();
    assert!(!out.partial);
    assert_eq!(out.actors.len(), 8);
    let (body, fill, _, _) = details(&out.actors[0]);
    let (_, rim, _, _) = details(&out.actors[1]);
    assert_eq!(body, [100.0, 271.0]);
    assert_eq!(fill, [0.4, 0.4, 0.4, 127.0 / 255.0]);
    assert_eq!(rim, [1.0, 1.0, 1.0, 0.5]);
    let (cap, _, _, _) = details(&out.actors[4]);
    assert_eq!(cap, [100.0, 367.5]);
    let (head, _, _, rotation) = details(&out.actors[6]);
    assert_eq!(head, [100.0, 200.0]);
    assert_eq!(rotation, -90.0);
    let (_, _, uv, _) = details(&out.actors[2]);
    assert!(((uv[3] - uv[1]) - 7.0 / 1280.0).abs() < 1e-7);
}
#[test]
fn pump_reverse_mirrors_body_uv_but_never_flips_arrow_end_cap() {
    let g = render::graphics::graphic("pump-single", 3, false, true).unwrap();
    let geometry = Input {
        reverse: true,
        head_top: 300.0,
        height: -300.0,
        cap_height: 128,
        cap_centered: true,
        cutoff: None,
        ..input()
    };
    let out = render::render(Request {
        graphic: &g,
        geometry,
        center_x: 200.0,
        width: 128.0,
        colors: colors(),
        alpha: 1.0,
        body_budget: 100,
    })
    .unwrap();
    let (_, _, body, _) = details(&out.actors[0]);
    assert!(body[0] > body[2]);
    assert!(body[1] > body[3]);
    let (_, _, cap, rotation) = details(&out.actors[out.actors.len() - 4]);
    assert!(cap[0] < cap[2]);
    assert!(cap[1] < cap[3]);
    assert_eq!(rotation, -90.0);
}
#[test]
fn body_budget_is_observable_and_dimensions_cannot_claim_a_different_atlas_layout() {
    let g = render::graphics::graphic("dance-single", 0, false, false).unwrap();
    let out = render::render(Request {
        graphic: &g,
        geometry: input(),
        center_x: 100.0,
        width: 128.0,
        colors: colors(),
        alpha: 1.0,
        body_budget: 1,
    })
    .unwrap();
    assert!(out.partial);
    assert_eq!(out.actors.len(), 6);
    assert!(
        render::render(Request {
            graphic: &g,
            geometry: Input {
                cap_height: 128,
                ..input()
            },
            center_x: 100.0,
            width: 128.0,
            colors: colors(),
            alpha: 1.0,
            body_budget: 100
        })
        .is_err()
    );
}
