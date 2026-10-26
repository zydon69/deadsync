//! Source factory palette dispatch is a chart family property, never Grid.players().
#[path = "../src/screens/edition/hold_render.rs"]
mod hold;
use deadlib_present::actors::Actor;
fn body_tint(layout: &str, lane: u16, player: usize) -> [f32; 4] {
    let out = hold::chart_render(hold::ChartRequest {
        layout,
        row: 0,
        lane,
        player,
        selected: false,
        roll: false,
        active: false,
        center_x: 200.0,
        head_center: 160.0,
        tail_center: 300.0,
        width: 48.0,
        reverse: false,
        viewport: [0.0, 105.0, 800.0, 395.0],
        cutoff: None,
        size_zoom: 1.0,
        body_budget: 100,
    })
    .unwrap();
    match &out.actors[0] {
        Actor::Sprite { tint, .. } => *tint,
        _ => panic!("body sprite"),
    }
}
#[test]
fn routine_p1_is_colored_but_couple_multiple_participants_uses_double_palette() {
    let ordinary = body_tint("dance-double", 0, 0);
    assert_eq!(body_tint("dance-couple", 0, 0), ordinary);
    assert_eq!(body_tint("dance-couple", 4, 1), ordinary);
    assert_ne!(body_tint("dance-routine", 0, 0), ordinary);
    assert_ne!(
        body_tint("dance-routine", 0, 0),
        body_tint("dance-routine", 4, 1)
    );
}
#[test]
fn source_pump_couple_and_smx_full_are_ordinary_while_routine_and_team_use_owner() {
    assert_eq!(
        body_tint("pump-couple", 1, 1),
        body_tint("pump-double", 1, 0)
    );
    assert_ne!(
        body_tint("pump-routine", 1, 0),
        body_tint("pump-double", 1, 0)
    );
    assert_ne!(
        body_tint("pump-routine", 1, 0),
        body_tint("pump-routine", 1, 1)
    );
    assert_eq!(body_tint("smx-full", 0, 0), body_tint("smx-full", 5, 1));
    assert_ne!(body_tint("smx-team", 0, 0), body_tint("smx-full", 0, 0));
}
