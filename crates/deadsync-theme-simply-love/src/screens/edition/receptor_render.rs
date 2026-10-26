//! Source receptor atlas projection, ready for per-resident animation state.
//! This module intentionally owns no Session, audio transport or lane input.
#[path = "receptor_atlas.rs"]
mod atlas;
use super::{view_layout::Geometry, view_render::Viewport};
use crate::act;
use deadlib_present::actors::Actor;
use deadsync_editor::{Error, receptors::Visual};
const TEXTURE: &str = "editor-grooveauthor/atlas.png";
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Graphic {
    pub base: &'static str,
    pub held: &'static str,
    pub glow: &'static str,
    pub rotation: f32,
}
pub(super) fn graphic(layout: &str, lane: u16) -> Result<Graphic, Error> {
    let columns = deadsync_editor::notes::layout_columns(layout)
        .ok_or_else(|| Error::Unsupported("Unsupported receptor layout".into()))?;
    if lane >= columns {
        return Err(Error::InvalidEdit("Receptor lane is outside layout".into()));
    }
    let i = usize::from(lane);
    let (shape, rotation) = match layout {
        "dance-single" | "dance-double" | "dance-couple" | "dance-routine" => {
            ("down", [90.0, 0.0, 180.0, 270.0][i % 4])
        }
        "dance-solo" => (
            if i == 1 || i == 4 { "solo" } else { "down" },
            [90.0, 0.0, 0.0, 180.0, 90.0, 270.0][i],
        ),
        "dance-threepanel" => (
            if i == 0 || i == 2 { "solo" } else { "down" },
            [0.0, 0.0, 90.0][i],
        ),
        "smx-beginner" => (
            if i == 1 { "center" } else { "down" },
            [90.0, 0.0, 270.0][i],
        ),
        "smx-dual" => (
            if i % 3 == 1 { "center" } else { "down" },
            [90.0, 0.0, 270.0][i % 3],
        ),
        "smx-single" | "smx-full" | "smx-team" => (
            if i % 5 == 2 { "center" } else { "down" },
            [90.0, 0.0, 0.0, 180.0, 270.0][i % 5],
        ),
        _ => {
            let i = (i + if layout == "pump-halfdouble" { 2 } else { 0 }) % 5;
            (
                if i == 2 { "piu-center" } else { "piu-diagonal" },
                [270.0, 0.0, 0.0, 90.0, 180.0][i],
            )
        }
    };
    let names = match shape {
        "down" => (
            "itg-down-receptor",
            "itg-down-receptor-held",
            "itg-down-receptor-glow",
        ),
        "solo" => (
            "itg-solo-receptor",
            "itg-solo-receptor-held",
            "itg-solo-receptor-glow",
        ),
        "center" => (
            "itg-center-receptor",
            "itg-center-receptor-held",
            "itg-center-receptor-glow",
        ),
        "piu-center" => (
            "piu-center-receptor",
            "piu-center-receptor-held",
            "piu-center-receptor-glow",
        ),
        _ => (
            "piu-diagonal-receptor",
            "piu-diagonal-receptor-held",
            "piu-diagonal-receptor-glow",
        ),
    };
    Ok(Graphic {
        base: names.0,
        held: names.1,
        glow: names.2,
        rotation,
    })
}
fn sprite(
    actors: &mut Vec<Actor>,
    name: &str,
    x: f32,
    y: f32,
    size: f32,
    rotation: f32,
    color: [f32; 4],
    z: i16,
) {
    let [a, b, w, h] = atlas::rect(name).expect("verified source receptor texture");
    actors.push(act!(sprite(TEXTURE): xy(x,y): setsize(size,size): customtexturerect(a as f32/1280.0,b as f32/1280.0,(a+w) as f32/1280.0,(b+h) as f32/1280.0): rotationz(rotation): diffuse(color[0],color[1],color[2],color[3]): z(z)));
}
pub(super) fn actors(
    actors: &mut Vec<Actor>,
    layout: &str,
    geometry: Geometry,
    viewport: Viewport,
    size_zoom: f64,
    visuals: &[Visual],
) -> Result<(), Error> {
    let columns = deadsync_editor::notes::layout_columns(layout)
        .ok_or_else(|| Error::Unsupported("Unsupported receptor layout".into()))?;
    if visuals.len() != usize::from(columns) {
        return Err(Error::InvalidEdit(
            "Receptor state count differs from layout".into(),
        ));
    }
    let alpha = deadsync_editor::receptors::alpha(size_zoom)?;
    if alpha <= 0.0 {
        return Ok(());
    }
    let size = geometry.lane_width * size_zoom as f32;
    for (lane, visual) in visuals.iter().enumerate() {
        let x = geometry.left + (lane as f32 + 0.5) * geometry.lane_width;
        if !geometry.visible_x(x) {
            continue;
        }
        let graphic = graphic(layout, lane as u16)?;
        sprite(
            actors,
            graphic.base,
            x,
            viewport.cursor,
            size * visual.tap_scale,
            graphic.rotation,
            [
                visual.brightness,
                visual.brightness,
                visual.brightness,
                alpha,
            ],
            8008,
        );
        if visual.rim_alpha > 0.0 {
            sprite(
                actors,
                graphic.held,
                x,
                viewport.cursor,
                size * visual.rim_scale,
                graphic.rotation,
                [1.0, 1.0, 1.0, visual.rim_alpha * alpha],
                8009,
            );
        }
        if visual.glow_alpha > 0.0 {
            sprite(
                actors,
                graphic.glow,
                x,
                viewport.cursor,
                size,
                graphic.rotation,
                [1.0, 1.0, 1.0, visual.glow_alpha * alpha],
                8010,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_supported_layout_receptor_shapes_resolve_inside_matching_original_atlas() {
        for layout in [
            "dance-single",
            "dance-double",
            "dance-couple",
            "dance-routine",
            "dance-solo",
            "dance-threepanel",
            "pump-single",
            "pump-double",
            "pump-couple",
            "pump-halfdouble",
            "pump-routine",
            "smx-beginner",
            "smx-single",
            "smx-dual",
            "smx-full",
            "smx-team",
        ] {
            let columns = deadsync_editor::notes::layout_columns(layout).unwrap();
            for lane in 0..columns {
                let g = graphic(layout, lane).unwrap();
                for name in [g.base, g.held, g.glow] {
                    let [x, y, w, h] = atlas::rect(name).unwrap();
                    assert!(x + w <= 1280 && y + h <= 1280, "{layout}/{lane}/{name}");
                }
            }
            assert!(graphic(layout, columns).is_err());
        }
        assert_eq!(
            graphic("pump-halfdouble", 0).unwrap().base,
            "piu-center-receptor"
        );
        assert_eq!(graphic("dance-solo", 1).unwrap().base, "itg-solo-receptor");
        assert_eq!(graphic("smx-team", 7).unwrap().base, "itg-center-receptor");
        assert!(graphic("kb7-single", 0).is_err());
    }
    #[test]
    fn receptor_actors_keep_lane_rotations_and_separate_foreground_effect_layers() {
        let geometry = Geometry {
            source: 0,
            order: 0,
            focused: true,
            left: 331.0,
            field_width: 192.0,
            lane_width: 48.0,
            clip_left: 0.0,
            clip_right: 854.0,
        };
        let viewport = Viewport {
            top: 105.0,
            bottom: 395.0,
            cursor: 160.0,
        };
        let visual = Visual {
            brightness: 0.8,
            tap_scale: 0.9,
            rim_scale: 1.1,
            rim_alpha: 0.5,
            glow_alpha: 0.25,
        };
        let mut out = Vec::new();
        actors(
            &mut out,
            "dance-single",
            geometry,
            viewport,
            1.0,
            &[visual; 4],
        )
        .unwrap();
        assert_eq!(out.len(), 12);
        for (lane, items) in out.chunks_exact(3).enumerate() {
            for (layer, actor) in items.iter().enumerate() {
                let Actor::Sprite {
                    offset,
                    rot_z_deg,
                    z,
                    uv_rect,
                    ..
                } = actor
                else {
                    panic!("expected receptor sprite")
                };
                assert_eq!(*offset, [355.0 + 48.0 * lane as f32, 160.0]);
                assert_eq!(*rot_z_deg, [90.0, 0.0, 180.0, 270.0][lane]);
                assert_eq!(*z, 8008 + layer as i16);
                assert!(
                    uv_rect
                        .unwrap()
                        .into_iter()
                        .all(|v| (0.0..=1.0).contains(&v))
                );
            }
        }
        out.clear();
        actors(
            &mut out,
            "dance-single",
            geometry,
            viewport,
            0.01,
            &[visual; 4],
        )
        .unwrap();
        assert!(out.is_empty());
    }
}
