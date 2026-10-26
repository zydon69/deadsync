//! Original hold/roll atlas rendering. The caller provides source palette colors
//! and the uncut source geometry; audio/cursor/selection ownership stays outside.
#[path = "hold_atlas.rs"]
mod atlas;
#[path = "hold_geometry.rs"]
pub mod geometry;
#[path = "hold_graphics.rs"]
pub mod graphics;
use deadlib_present::actors::Actor;
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
         customtexturerect(u0,v0,u1,v1): rotationz(rotation):
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
    })
}
