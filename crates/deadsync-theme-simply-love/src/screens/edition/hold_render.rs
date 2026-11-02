//! Original hold/roll atlas rendering. The caller provides source palette colors
//! and the uncut source geometry; audio/cursor/selection ownership stays outside.
#[path = "hold_atlas.rs"]
mod atlas;
#[path = "hold_geometry.rs"]
pub mod geometry;
#[path = "hold_graphics.rs"]
pub mod graphics;
use deadlib_present::actors::{Actor, SizeSpec};
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub head: [u8; 4],
    pub start: [u8; 4],
    pub body: [u8; 4],
    pub cap: [u8; 4],
}
pub struct Request<'a> {
    pub graphic: &'a graphics::Graphic,
    pub geometry: geometry::Input,
    pub center_x: f32,
    pub width: f32,
    pub colors: Colors,
    pub alpha: f32,
    /// Body strips, each emitting one fill plus one rim actor.
    pub body_budget: usize,
}
pub struct Output {
    pub actors: Vec<Actor>,
    pub partial: bool,
    pub body_strips: usize,
}
#[derive(Clone, Copy)]
struct Destination {
    x: f32,
    top: f64,
    width: f32,
    height: f64,
}
fn fill(mut rgba: [u8; 4], alpha: f32) -> [f32; 4] {
    rgba[3] = (f32::from(rgba[3]) * alpha) as u8;
    rgba.map(|c| f32::from(c) / 255.0)
}
fn sprite(
    id: &str,
    d: Destination,
    crop: Option<(u32, u32)>,
    rotation: f32,
    flip_x: bool,
    flip_y: bool,
    color: [f32; 4],
) -> Result<Actor, &'static str> {
    let [x, y, w, h] = atlas::rect(id).ok_or("Unknown original hold texture")?;
    let (sy, sh) = crop.unwrap_or((0, h));
    if sy + sh > h {
        return Err("Hold source crop outside atlas rectangle");
    }
    let (mut u0, mut u1) = (x as f32 / 1280.0, (x + w) as f32 / 1280.0);
    let (mut v0, mut v1) = ((y + sy) as f32 / 1280.0, (y + sy + sh) as f32 / 1280.0);
    if flip_x {
        std::mem::swap(&mut u0, &mut u1);
    }
    if flip_y {
        std::mem::swap(&mut v0, &mut v1);
    }
    Ok(deadlib_present::__act_from_builder!(
        (xy(d.x,(d.top+d.height*0.5) as f32): setsize(d.width,d.height as f32):
         customtexturerect(u0,v0,u1,v1): rotationz(-rotation):
         diffuse(color[0],color[1],color[2],color[3]): z(8004))
        ::deadsync_assets::present_dsl::SpriteBuilder::static_texture("editor-grooveauthor/atlas.png")
    ))
}
pub fn render(request: Request<'_>) -> Result<Output, &'static str> {
    if ![request.center_x, request.width, request.alpha]
        .iter()
        .all(|n| n.is_finite())
        || request.width <= 0.0
        || !(0.0..=1.0).contains(&request.alpha)
    {
        return Err("Invalid hold actor projection");
    }
    if request.alpha == 0.0 {
        return Ok(Output {
            actors: Vec::new(),
            partial: false,
            body_strips: 0,
        });
    }
    let g = request.graphic;
    let height = |id: &str| {
        atlas::rect(id)
            .map(|r| r[3])
            .ok_or("Unknown original hold texture")
    };
    let start_height = match &g.start_rim {
        Some(id) => height(id)?,
        None => 0,
    };
    if request.geometry.arrow_height != height(&g.head_rim)?
        || request.geometry.body_height != height(&g.body_rim)?
        || request.geometry.cap_height != height(&g.cap_rim)?
        || request.geometry.start_height != start_height
        || request.geometry.cap_centered != g.cap_centered
    {
        return Err("Hold geometry differs from its atlas layout");
    }
    let p = request.geometry.plan(request.body_budget)?;
    let rim = [1.0, 1.0, 1.0, request.alpha];
    let reverse = request.geometry.reverse;
    let dest = |s: geometry::Strip| Destination {
        x: request.center_x,
        top: s.top,
        width: request.width,
        height: s.height,
    };
    let mut actors = Vec::new();
    for strip in &p.body {
        let crop = Some((strip.source_top, strip.source_height));
        actors.push(sprite(
            &g.body_fill,
            dest(*strip),
            crop,
            0.0,
            g.body_mirrored,
            reverse,
            fill(request.colors.body, request.alpha),
        )?);
        actors.push(sprite(
            &g.body_rim,
            dest(*strip),
            crop,
            0.0,
            g.body_mirrored,
            reverse,
            rim,
        )?);
    }
    if let Some(start) = p.start {
        if let (Some(fill_id), Some(rim_id)) = (&g.start_fill, &g.start_rim) {
            actors.push(sprite(
                fill_id,
                dest(start),
                None,
                0.0,
                false,
                reverse,
                fill(request.colors.start, request.alpha),
            )?);
            actors.push(sprite(rim_id, dest(start), None, 0.0, false, reverse, rim)?);
        }
    }
    if let Some(cap) = p.cap {
        let flip = reverse && g.flip_cap_in_reverse;
        actors.push(sprite(
            &g.cap_fill,
            dest(cap),
            None,
            g.cap_rotation,
            false,
            flip,
            fill(request.colors.cap, request.alpha),
        )?);
        actors.push(sprite(
            &g.cap_rim,
            dest(cap),
            None,
            g.cap_rotation,
            false,
            flip,
            rim,
        )?);
    }
    let head = Destination {
        x: request.center_x,
        top: p.head_top,
        width: request.width,
        height: f64::from(request.geometry.arrow_height) * request.geometry.scale,
    };
    actors.push(sprite(
        &g.head_fill,
        head,
        None,
        g.head_rotation,
        false,
        false,
        fill(request.colors.head, request.alpha),
    )?);
    actors.push(sprite(
        &g.head_rim,
        head,
        None,
        g.head_rotation,
        false,
        false,
        rim,
    )?);
    Ok(Output {
        actors,
        partial: p.partial,
        body_strips: p.body.len(),
    })
}

#[path = "hold_palette.rs"]
mod palette;
/// Native projection boundary: note centers become the original extended H.
/// Bound work by visible geometry, independently for each admitted hold.
/// Every atlas body is at least one source pixel high and scales by width/128.
/// Two edge tiles plus rounding allowance cover the planner's padded viewport.
/// Extreme subpixel zoom remains bounded and is reported as partial by plan().
pub(super) fn viewport_body_budget(width: f32, height: f32) -> usize {
    if !width.is_finite() || width <= 0.0 || !height.is_finite() || height < 0.0 {
        return 0;
    }
    ((f64::from(height) * 128.0 / f64::from(width)).ceil() as usize)
        .saturating_add(4).min(16_384)
}

pub struct ChartRequest<'a> {
    pub layout: &'a str,
    pub row: u64,
    pub lane: u16,
    pub player: usize,
    pub selected: bool,
    pub roll: bool,
    pub active: bool,
    pub center_x: f32,
    pub head_center: f64,
    pub tail_center: f64,
    pub width: f32,
    pub reverse: bool,
    pub viewport: [f32; 4],
    pub cutoff: Option<f64>,
    pub size_zoom: f64,
    pub body_budget: usize,
}
pub fn chart_render(r: ChartRequest<'_>) -> Result<Output, &'static str> {
    let fills = palette::palette(
        r.layout,
        r.row,
        r.lane,
        r.player,
        r.selected,
        r.roll,
        r.active,
        palette::Options {
            multiplayer: matches!(r.layout, "dance-routine" | "pump-routine" | "smx-team"),
            ..Default::default()
        },
    )?;
    chart_render_colors(
        r,
        Colors {
            head: fills.head,
            start: fills.start,
            body: fills.body,
            cap: fills.cap,
        },
        None,
    )
}
/// IsBeingEdited uses ActiveEditEventAlpha independently of ordinary zoom fading.
pub fn chart_render_draft(
    r: ChartRequest<'_>,
    palette: &deadsync_editor::note_color_palette::Palette,
    signatures: Option<&deadsync_editor::timing_edit::signature::Signatures>,
) -> Result<Output, &'static str> {
    chart_render_cached_alpha(r, palette, signatures, Some(0.8), true)
}
pub fn chart_render_cached_with_options(r: ChartRequest<'_>,palette:&deadsync_editor::note_color_palette::Palette,signatures:Option<&deadsync_editor::timing_edit::signature::Signatures>,light_holds:bool)->Result<Output,&'static str>{chart_render_cached_alpha(r,palette,signatures,None,light_holds)}
fn chart_render_cached_alpha(
    r: ChartRequest<'_>,
    palette: &deadsync_editor::note_color_palette::Palette,
    signatures: Option<&deadsync_editor::timing_edit::signature::Signatures>,
    alpha: Option<f32>,
    light_holds:bool,
) -> Result<Output, &'static str> {
    let row = palette
        .color_row(r.row, signatures)
        .map_err(|_| "Invalid chart note color phase")?;
    let fills = palette
        .fills(
            r.layout,
            row,
            r.lane,
            r.player as u32,
            r.selected,
            r.roll,
            r.active,
            light_holds,
        )
        .map_err(|_| "Invalid chart note color palette")?;
    chart_render_colors(
        r,
        Colors {
            head: fills.head,
            start: fills.start,
            body: fills.body,
            cap: fills.cap,
        },
        alpha,
    )
}
fn chart_render_colors(
    r: ChartRequest<'_>,
    colors: Colors,
    alpha: Option<f32>,
) -> Result<Output, &'static str> {
    let [left, top, right, bottom] = r.viewport;
    if !r.viewport.iter().all(|n| n.is_finite()) || right <= left || bottom <= top {
        return Err("Invalid hold viewport");
    }
    let top = f64::from(top);
    let graphic = graphics::graphic(r.layout, r.lane, r.selected, r.roll)?;
    let height = |id: &str| {
        atlas::rect(id)
            .map(|r| r[3])
            .ok_or("Unknown original hold texture")
    };
    let arrow_height = height(&graphic.head_rim)?;
    let body_height = height(&graphic.body_rim)?;
    let cap_height = height(&graphic.cap_rim)?;
    let start_height = match &graphic.start_rim {
        Some(id) => height(id)?,
        None => 0,
    };
    let scale = f64::from(r.width) / 128.0;
    let head_top = r.head_center - top - f64::from(arrow_height) * scale * 0.5;
    let cap_extent = f64::from(cap_height) * scale * if graphic.cap_centered { 0.5 } else { 1.0 };
    let height = r.tail_center - top + cap_extent * if r.reverse { -1.0 } else { 1.0 } - head_top;
    let mut output = render(Request {
        graphic: &graphic,
        geometry: geometry::Input {
            head_top,
            height,
            scale,
            arrow_height,
            body_height,
            cap_height,
            start_height,
            cap_centered: graphic.cap_centered,
            reverse: r.reverse,
            screen_height: f64::from(bottom) - top,
            cutoff: r.cutoff.map(|y| y - top),
        },
        center_x: r.center_x,
        width: r.width,
        colors,
        alpha: match alpha {
            Some(alpha) => alpha,
            None => deadsync_editor::receptors::alpha(r.size_zoom)
                .map_err(|_| "Invalid hold size alpha")?,
        },
        body_budget: r.body_budget,
    })?;
    for actor in &mut output.actors {
        clip_to_viewport(actor, r.viewport)?;
    }
    output
        .actors
        .retain(|a| matches!(a, Actor::Sprite { visible: true, .. }));
    Ok(output)
}
/// Adapt world-space short-note and marker actors to the hold viewport clipper.
pub(super) fn clip_world_sprite(
    actor: &mut Actor,
    viewport: [f32; 4],
) -> Result<bool, &'static str> {
    let Actor::Sprite { offset, .. } = actor else {
        return Err("Expected sprite for viewport clipping");
    };
    offset[1] -= viewport[1];
    clip_to_viewport(actor, viewport)?;
    Ok(matches!(actor, Actor::Sprite { visible: true, .. }))
}
pub(super) fn clip_to_viewport(
    actor: &mut Actor,
    [left, top, right, bottom]: [f32; 4],
) -> Result<(), &'static str> {
    let Actor::Sprite {
        offset,
        size,
        rot_z_deg,
        cropleft,
        cropright,
        croptop,
        cropbottom,
        visible,
        ..
    } = actor
    else {
        return Err("Unexpected non-sprite hold actor");
    };
    offset[1] += top;
    let width = match size[0] {
        SizeSpec::Px(n) => n,
        _ => return Err("Hold sprite width is not explicit"),
    };
    let height = match size[1] {
        SizeSpec::Px(n) => n,
        _ => return Err("Hold sprite height is not explicit"),
    };
    if width <= 0.0 || height <= 0.0 {
        *visible = false;
        return Ok(());
    }
    let rotation = rot_z_deg.rem_euclid(360.0);
    let (w, h) = if rotation == 90.0 || rotation == 270.0 {
        (height, width)
    } else {
        (width, height)
    };
    let x0 = offset[0] - w * 0.5;
    let x1 = offset[0] + w * 0.5;
    let y0 = offset[1] - h * 0.5;
    let y1 = offset[1] + h * 0.5;
    if x1 <= left || x0 >= right || y1 <= top || y0 >= bottom {
        *visible = false;
        return Ok(());
    }
    let l = ((left - x0) / w).clamp(0.0, 1.0);
    let r = ((x1 - right) / w).clamp(0.0, 1.0);
    let t = ((top - y0) / h).clamp(0.0, 1.0);
    let b = ((y1 - bottom) / h).clamp(0.0, 1.0);
    let [cl, cr, ct, cb] = match rotation {
        0.0 => [l, r, t, b],
        90.0 => [b, t, l, r],
        180.0 => [r, l, b, t],
        270.0 => [t, b, r, l],
        _ => return Err("Noncardinal hold sprite rotation"),
    };
    // The compositor applies the crop's center displacement in world axes,
    // then rotates the resized quad. Move the actor by the difference from
    // the rotated displacement so the visible original pixels stay anchored.
    let delta = [(cl - cr) * width * 0.5, (ct - cb) * height * 0.5];
    let (sin, cos) = rotation.to_radians().sin_cos();
    offset[0] += cos * delta[0] + sin * delta[1] - delta[0];
    offset[1] += -sin * delta[0] + cos * delta[1] - delta[1];
    *cropleft = cl;
    *cropright = cr;
    *croptop = ct;
    *cropbottom = cb;
    Ok(())
}

#[cfg(test)]
mod rotation_boundary_tests {
    use super::*;
    #[test]
    fn every_source_hold_head_and_pump_cap_uses_native_handedness() {
        for layout in deadsync_editor::chart_order::TYPE_ORDER {
            for lane in 0..deadsync_editor::notes::layout_columns(layout).unwrap() {
                let graphic=graphics::graphic(layout,lane,false,false).unwrap();
                for (id,source_rotation) in [(&graphic.head_fill,graphic.head_rotation),(&graphic.cap_fill,graphic.cap_rotation)] {
                    let actor=sprite(id,Destination{x:100.,top:140.,width:48.,height:48.},None,source_rotation,false,false,[1.;4]).unwrap();
                    let Actor::Sprite{rot_z_deg,..}=actor else {panic!("sprite");};
                    assert_eq!(rot_z_deg,-source_rotation,"{layout}/{lane}/{id}");
                }
            }
        }
    }
}

#[cfg(test)]
mod viewport_corner_tests {
    use super::*;

    #[test]
    fn cropped_cardinal_sprite_corners_stay_inside_world_viewport() {
        let graphic = graphics::graphic("dance-single", 0, false, false).unwrap();
        let viewport = [20.0, 40.0, 180.0, 220.0];
        for rotation in [0.0_f32, 90.0, 180.0, 270.0] {
            for center in [[24.0, 130.0], [176.0, 130.0], [100.0, 44.0], [100.0, 216.0]] {
                let mut actor = sprite(&graphic.head_fill, Destination {
                    x: center[0], top: f64::from(center[1] - 24.0),
                    width: 48.0, height: 48.0,
                }, None, -rotation, false, false, [1.0; 4]).unwrap();
                assert!(clip_world_sprite(&mut actor, viewport).unwrap());
                let Actor::Sprite { offset, rot_z_deg, cropleft, cropright,
                    croptop, cropbottom, .. } = actor else { panic!("sprite"); };
                let (sin, cos) = rot_z_deg.to_radians().sin_cos();
                // Match the actual non-skewed compositor: crop moves the
                // center in world axes, then rotates the resized rectangle.
                let center = [offset[0] + (cropleft - cropright) * 24.0,
                    offset[1] + (croptop - cropbottom) * 24.0];
                let half = [24.0 * (1.0 - cropleft - cropright),
                    24.0 * (1.0 - croptop - cropbottom)];
                for x in [-half[0], half[0]] {
                    for y in [-half[1], half[1]] {
                        let world = [center[0] + cos * x + sin * y,
                            center[1] - sin * x + cos * y];
                        assert!(world[0] >= viewport[0] - 0.001
                            && world[0] <= viewport[2] + 0.001
                            && world[1] >= viewport[1] - 0.001
                            && world[1] <= viewport[3] + 0.001,
                            "rotation={rotation}, center={center:?}, corner={world:?}");
                    }
                }
            }
        }
    }
}
